//! Reverse impact analysis.

use std::collections::{HashMap, HashSet};

use klaub_core::graph::{
    CallerLevel, EdgeOrigin, EntrypointReach, EntrypointScope, ImpactCoverage, ImpactReport,
    ImpactScope, PathStep, SymbolKey,
};
use petgraph::Direction;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::{ModuleMap, ProjectGraph};

/// Limits of an impact query.
#[derive(Debug, Clone, Copy)]
pub struct ImpactOptions {
    /// Deepest caller chain to follow.
    pub max_depth: u32,
    /// Most entrypoint paths to report.
    pub max_entrypoints: usize,
}

impl Default for ImpactOptions {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_entrypoints: 12,
        }
    }
}

impl ProjectGraph {
    /// Whether a chain of calls leads from `from` to `to` within `max_depth` hops.
    ///
    /// `None` when the graph does not know one of the symbols.
    pub fn reaches(&self, from: &SymbolKey, to: &SymbolKey, max_depth: u32) -> Option<bool> {
        let (start, goal) = (*self.keys.get(from)?, *self.keys.get(to)?);
        let mut seen: HashSet<NodeIndex> = HashSet::from([start]);
        let mut frontier = vec![start];
        for _ in 0..max_depth {
            let mut next = Vec::new();
            for node in frontier {
                for edge in self.graph.edges_directed(node, Direction::Outgoing) {
                    let target = edge.target();
                    if target == goal {
                        return Some(true);
                    }
                    if seen.insert(target) {
                        next.push(target);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        Some(false)
    }

    /// What the change at `key` can reach: callers by distance, entrypoints with their paths,
    /// callees, the structural scope and how complete the analysis is.
    ///
    /// `None` when the graph does not know the symbol.
    pub fn impact(
        &self,
        key: &SymbolKey,
        options: &ImpactOptions,
        modules: &ModuleMap,
    ) -> Option<ImpactReport> {
        let start = *self.keys.get(key)?;
        let mut depth_of: HashMap<NodeIndex, u32> = HashMap::from([(start, 0)]);
        // For each visited node: the node one step closer to `start`, and the origin of that edge.
        let mut toward_start: HashMap<NodeIndex, (NodeIndex, EdgeOrigin)> = HashMap::new();
        let mut levels: Vec<CallerLevel> = Vec::new();
        let mut coverage = ImpactCoverage::default();
        let mut frontier = vec![start];

        for depth in 1..=options.max_depth {
            let mut next = Vec::new();
            let mut callers = Vec::new();
            for &node in &frontier {
                for edge in self.graph.edges_directed(node, Direction::Incoming) {
                    match edge.weight().origin {
                        EdgeOrigin::Resolved => coverage.resolved_edges += 1,
                        EdgeOrigin::Heuristic => coverage.heuristic_edges += 1,
                        EdgeOrigin::Unresolved => {}
                    }
                    let caller = edge.source();
                    if depth_of.contains_key(&caller) {
                        continue;
                    }
                    depth_of.insert(caller, depth);
                    toward_start.insert(caller, (node, edge.weight().origin));
                    callers.push(PathStep {
                        symbol: self.key_of(caller),
                        origin: edge.weight().origin,
                    });
                    next.push(caller);
                }
            }
            if next.is_empty() {
                frontier.clear();
                break;
            }
            callers.sort_by(|a, b| a.symbol.cmp(&b.symbol));
            levels.push(CallerLevel { depth, callers });
            frontier = next;
        }
        coverage.truncated = frontier.iter().any(|n| {
            self.graph
                .edges_directed(*n, Direction::Incoming)
                .any(|e| !depth_of.contains_key(&e.source()))
        });
        coverage.unresolved_calls = depth_of
            .keys()
            .map(|n| self.unresolved.get(n).copied().unwrap_or(0))
            .sum();

        // Entrypoints among the visited nodes, with the path from the entrypoint down to `start`.
        let mut reaches: Vec<EntrypointReach> = Vec::new();
        for &node in depth_of.keys() {
            for entry in self.entrypoints.get(&node).into_iter().flatten() {
                let mut path = vec![PathStep {
                    symbol: self.key_of(node),
                    origin: entry.origin,
                }];
                let mut origin = entry.origin;
                let mut current = node;
                while let Some(&(next, edge_origin)) = toward_start.get(&current) {
                    origin = origin.weakest(edge_origin);
                    path.push(PathStep {
                        symbol: self.key_of(next),
                        origin: edge_origin,
                    });
                    current = next;
                }
                reaches.push(EntrypointReach {
                    entrypoint: entry.clone(),
                    path,
                    origin,
                });
            }
        }
        reaches.sort_by(|a, b| {
            let rank = |r: &EntrypointReach| {
                (
                    r.entrypoint.scope,
                    r.path.len(),
                    r.origin,
                    r.entrypoint.symbol.clone(),
                )
            };
            rank(a).cmp(&rank(b))
        });
        reaches.truncate(options.max_entrypoints);

        let callees = self
            .graph
            .edges_directed(start, Direction::Outgoing)
            .map(|e| PathStep {
                symbol: self.key_of(e.target()),
                origin: e.weight().origin,
            })
            .collect::<Vec<_>>();

        let scope = self.scope_of(start, &depth_of, &reaches, modules);
        Some(ImpactReport {
            symbol: key.clone(),
            scope,
            callers: levels,
            entrypoints: reaches,
            callees,
            coverage,
        })
    }

    fn scope_of(
        &self,
        start: NodeIndex,
        visited: &HashMap<NodeIndex, u32>,
        reaches: &[EntrypointReach],
        modules: &ModuleMap,
    ) -> ImpactScope {
        let own_file = self.file_of(start);
        let own_module = modules.module_of(own_file);
        let others: Vec<NodeIndex> = visited.keys().copied().filter(|n| *n != start).collect();
        if others.iter().all(|n| self.file_of(*n) == own_file) {
            return ImpactScope::Local;
        }
        let foreign_modules: HashSet<String> = others
            .iter()
            .map(|n| modules.module_of(self.file_of(*n)))
            .filter(|m| *m != own_module)
            .collect();
        if foreign_modules.is_empty() {
            return ImpactScope::Module;
        }
        let external = reaches
            .iter()
            .any(|r| r.entrypoint.scope == EntrypointScope::External);
        if foreign_modules.len() >= 2 || external {
            ImpactScope::Global
        } else {
            ImpactScope::CrossModule
        }
    }
}

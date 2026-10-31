//! # klaub-graph
//!
//! A pure domain service that turns parsed files into a call and dependency graph and answers
//! the reviewer's question *"what can this change reach?"*:
//!
//! - **Resolution.** Calls are resolved through scope, imports, `self`, classes and finally by
//!   name. Every edge says how it was found: `Resolved` (statically unambiguous), `Heuristic`
//!   (by name, with at most three candidates) or, when a name is too ambiguous to follow,
//!   the call is counted as unresolved instead of guessed.
//! - **Entrypoints.** Parser hints (routes, `main`, exports) plus package manifests
//!   (`pyproject.toml` scripts, `package.json` `bin`/`main`/`exports`).
//! - **Impact.** A reverse traversal from a symbol up to the entrypoints that can reach it, the
//!   structural scope of the change, and how complete the picture is. A missing path is never
//!   evidence of a missing impact.

mod impact;
mod manifest;
mod modules;
mod resolve;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use klaub_core::graph::{EdgeOrigin, Entrypoint, SymbolKey};
use klaub_core::path::RepoPath;
use klaub_core::symbol::{CallSite, EntrypointHint, ParsedFile, Receiver, Symbol, SymbolKind};
use petgraph::graph::{DiGraph, NodeIndex};

pub use impact::ImpactOptions;
pub use manifest::ManifestText;
pub use modules::ModuleMap;

use manifest::{Target, Use};
use resolve::ModuleIndex;

/// Candidates tolerated for a name-based (heuristic) edge before the call is called ambiguous.
const MAX_HEURISTIC_CANDIDATES: usize = 3;
/// How many re-export hops are followed when looking for the definition of an imported name.
const MAX_REEXPORT_HOPS: u32 = 3;

/// A parsed file of the project, at the "after" state.
#[derive(Debug, Clone)]
pub struct IndexedFile {
    /// Path relative to the worktree.
    pub path: RepoPath,
    /// What the parser found.
    pub parsed: Arc<ParsedFile>,
}

/// A symbol of the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NodeRef {
    file: usize,
    symbol: usize,
}

#[derive(Debug, Clone, Copy)]
struct CallEdge {
    origin: EdgeOrigin,
}

/// A dependency between two files through an import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEdge {
    /// The importing file.
    pub from: RepoPath,
    /// The imported file.
    pub to: RepoPath,
    /// How sure the resolution is.
    pub origin: EdgeOrigin,
}

#[derive(Debug, Clone)]
enum Bound {
    Symbol { files: Vec<usize>, name: String },
    Module { files: Vec<usize> },
}

#[derive(Debug, Default)]
struct FileBindings {
    by_local: HashMap<String, Vec<Bound>>,
    /// `import a.b.c` style imports: dotted specifier to files.
    dotted: Vec<(String, Vec<usize>)>,
    wildcards: Vec<usize>,
}

/// The call and dependency graph of a project.
pub struct ProjectGraph {
    files: Vec<IndexedFile>,
    graph: DiGraph<NodeRef, CallEdge>,
    keys: HashMap<SymbolKey, NodeIndex>,
    entrypoints: HashMap<NodeIndex, Vec<Entrypoint>>,
    unresolved: HashMap<NodeIndex, u32>,
    import_edges: Vec<ImportEdge>,
    index: ModuleIndex,
}

fn simple_name(symbol: &Symbol) -> &str {
    symbol.name.rsplit('.').next().unwrap_or(&symbol.name)
}

fn is_callable_or_type(kind: SymbolKind) -> bool {
    kind.is_callable()
}

fn container_of(qualified: &str) -> &str {
    qualified.rsplit_once('.').map_or("", |(parent, _)| parent)
}

struct Builder<'a> {
    files: &'a [IndexedFile],
    index: ModuleIndex,
    bindings: Vec<FileBindings>,
    by_simple_name: HashMap<&'a str, Vec<NodeRef>>,
    top_level: HashMap<(usize, &'a str), Vec<usize>>,
    by_qualified: HashMap<(usize, &'a str), usize>,
}

impl<'a> Builder<'a> {
    fn new(files: &'a [IndexedFile]) -> Self {
        let index = ModuleIndex::new(files);
        let mut by_simple_name: HashMap<&str, Vec<NodeRef>> = HashMap::new();
        let mut top_level: HashMap<(usize, &str), Vec<usize>> = HashMap::new();
        let mut by_qualified: HashMap<(usize, &str), usize> = HashMap::new();
        for (f, file) in files.iter().enumerate() {
            for (s, symbol) in file.parsed.symbols.iter().enumerate() {
                if symbol.kind == SymbolKind::Module {
                    continue;
                }
                by_qualified
                    .entry((f, symbol.qualified_name.as_str()))
                    .or_insert(s);
                if is_callable_or_type(symbol.kind) {
                    by_simple_name
                        .entry(simple_name(symbol))
                        .or_default()
                        .push(NodeRef { file: f, symbol: s });
                }
                if symbol.parent == Some(0) && is_callable_or_type(symbol.kind) {
                    top_level
                        .entry((f, symbol.name.as_str()))
                        .or_default()
                        .push(s);
                }
            }
        }
        let mut builder = Self {
            files,
            index,
            bindings: Vec::new(),
            by_simple_name,
            top_level,
            by_qualified,
        };
        builder.bindings = (0..files.len()).map(|f| builder.bind(f)).collect();
        builder
    }

    fn bind(&self, f: usize) -> FileBindings {
        let file = &self.files[f];
        let mut out = FileBindings::default();
        for import in &file.parsed.imports {
            let language = file.parsed.language;
            let module_files = self.index.resolve(self.files, f, import);
            if import.wildcard && import.alias.is_none() {
                out.wildcards.extend(module_files.iter().copied());
            }
            let last_segment = import
                .module
                .rsplit(['/', '.', ':'])
                .find(|s| !s.is_empty())
                .unwrap_or("")
                .to_owned();
            // Module bindings: `import x as y`, `import * as ns`, Go packages, `use a::b`.
            if let Some(alias) = &import.alias {
                if !module_files.is_empty() {
                    out.by_local
                        .entry(alias.clone())
                        .or_default()
                        .push(Bound::Module {
                            files: module_files.clone(),
                        });
                }
            } else if import.names.is_empty() && !import.wildcard {
                match language {
                    klaub_core::language::Language::Python => {
                        let dotted = import.module.clone();
                        if !module_files.is_empty() {
                            out.dotted.push((dotted.clone(), module_files.clone()));
                            if let Some(first) = dotted.split('.').next() {
                                // `import a.b` binds `a`; calls are matched by their dotted path.
                                if dotted == first {
                                    out.by_local.entry(first.to_owned()).or_default().push(
                                        Bound::Module {
                                            files: module_files.clone(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                    klaub_core::language::Language::Go
                    | klaub_core::language::Language::Rust
                    | klaub_core::language::Language::Java
                        if !module_files.is_empty() && !last_segment.is_empty() =>
                    {
                        out.by_local
                            .entry(last_segment.clone())
                            .or_default()
                            .push(Bound::Module {
                                files: module_files.clone(),
                            });
                    }
                    _ => {}
                }
            }
            // Named imports.
            for name in &import.names {
                let local = name.alias.clone().unwrap_or_else(|| name.name.clone());
                if !module_files.is_empty() {
                    out.by_local
                        .entry(local.clone())
                        .or_default()
                        .push(Bound::Symbol {
                            files: module_files.clone(),
                            name: name.name.clone(),
                        });
                }
                // `from pkg import mod` and `use crate::a::b` may name a submodule.
                if language == klaub_core::language::Language::Python {
                    let spec = if import.module.is_empty() {
                        name.name.clone()
                    } else {
                        format!("{}.{}", import.module, name.name)
                    };
                    let sub = self
                        .index
                        .python(file.path.as_str(), &spec, import.relative_level);
                    if !sub.is_empty() {
                        out.by_local
                            .entry(local)
                            .or_default()
                            .push(Bound::Module { files: sub });
                    }
                } else if language == klaub_core::language::Language::Rust {
                    let sub = self.index.rust(
                        file.path.as_str(),
                        &format!("{}::{}", import.module, name.name),
                    );
                    if !sub.is_empty() {
                        out.by_local
                            .entry(local)
                            .or_default()
                            .push(Bound::Module { files: sub });
                    }
                }
            }
        }
        out
    }

    fn top_level_symbols(&self, f: usize, name: &str) -> Vec<usize> {
        self.top_level.get(&(f, name)).cloned().unwrap_or_default()
    }

    /// Definitions of `name` exported by file `f`, following re-exports.
    fn exported(&self, f: usize, name: &str, hops: u32) -> Vec<(usize, usize)> {
        if name == "default" {
            let public: Vec<usize> = self.files[f]
                .parsed
                .symbols
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.parent == Some(0) && s.is_public() && is_callable_or_type(s.kind)
                })
                .map(|(i, _)| i)
                .collect();
            return if public.len() == 1 {
                vec![(f, public[0])]
            } else {
                Vec::new()
            };
        }
        let direct = self.top_level_symbols(f, name);
        if !direct.is_empty() {
            return direct.into_iter().map(|s| (f, s)).collect();
        }
        if hops >= MAX_REEXPORT_HOPS {
            return Vec::new();
        }
        let mut found = Vec::new();
        for import in self.files[f].parsed.imports.iter().filter(|i| i.reexport) {
            let listed = import.wildcard
                || import
                    .names
                    .iter()
                    .any(|n| n.alias.as_deref().unwrap_or(&n.name) == name);
            if !listed {
                continue;
            }
            let original = import
                .names
                .iter()
                .find(|n| n.alias.as_deref().unwrap_or(&n.name) == name)
                .map_or(name, |n| n.name.as_str());
            for target in self.index.resolve(self.files, f, import) {
                found.extend(self.exported(target, original, hops + 1));
            }
        }
        found
    }

    fn constructor_of(&self, f: usize, class: usize) -> Option<usize> {
        let qualified = &self.files[f].parsed.symbols[class].qualified_name;
        ["__init__", "constructor", "new", "New"]
            .iter()
            .find_map(|ctor| {
                self.by_qualified
                    .get(&(f, format!("{qualified}.{ctor}").as_str()))
                    .copied()
            })
    }

    fn class_named(&self, f: usize, name: &str) -> Option<(usize, usize)> {
        let local = self.top_level_symbols(f, name).into_iter().find(|s| {
            matches!(
                self.files[f].parsed.symbols[*s].kind,
                SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface
            )
        });
        if let Some(s) = local {
            return Some((f, s));
        }
        for bound in self.bindings[f].by_local.get(name)? {
            if let Bound::Symbol { files, name } = bound {
                for &target in files {
                    for (tf, ts) in self.exported(target, name, 0) {
                        if matches!(
                            self.files[tf].parsed.symbols[ts].kind,
                            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface
                        ) {
                            return Some((tf, ts));
                        }
                    }
                }
            }
        }
        None
    }

    fn bases_of(symbol: &Symbol) -> Vec<String> {
        let signature = symbol.signature.as_str();
        let names: Vec<&str> = if let Some((_, rest)) = signature.split_once(" extends ") {
            rest.split(" implements ")
                .next()
                .unwrap_or(rest)
                .split(',')
                .collect()
        } else if let Some((_, args)) = signature.split_once('(') {
            args.trim_end_matches([')', ':', ' ']).split(',').collect()
        } else {
            Vec::new()
        };
        names
            .into_iter()
            .map(|n| {
                n.trim()
                    .split(['<', '['])
                    .next()
                    .unwrap_or("")
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_owned()
            })
            .filter(|n| !n.is_empty() && !n.contains('=') && n != "object")
            .collect()
    }

    /// Looks for `name` as a nested definition of the caller, then of its ancestors.
    fn local_lookup(&self, f: usize, caller: usize, name: &str) -> Option<usize> {
        let symbols = &self.files[f].parsed.symbols;
        let mut scope = Some(caller);
        while let Some(current) = scope {
            if let Some(found) = symbols.iter().enumerate().find_map(|(i, s)| {
                (s.parent == Some(current)
                    && s.name == name
                    && is_callable_or_type(s.kind)
                    && s.kind != SymbolKind::Method)
                    .then_some(i)
            }) {
                return Some(found);
            }
            scope = symbols[current].parent;
        }
        None
    }

    fn by_name(
        &self,
        name: &str,
        caller: NodeRef,
        prefer_methods: bool,
    ) -> (Vec<(NodeRef, EdgeOrigin)>, bool) {
        let mut candidates: Vec<NodeRef> = self
            .by_simple_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|n| *n != caller)
            .collect();
        let kind_of = |n: &NodeRef| self.files[n.file].parsed.symbols[n.symbol].kind;
        let wanted: Vec<NodeRef> = candidates
            .iter()
            .copied()
            .filter(|n| (kind_of(n) == SymbolKind::Method) == prefer_methods)
            .collect();
        if !wanted.is_empty() {
            candidates = wanted;
        }
        match candidates.len() {
            0 => (Vec::new(), false),
            n if n <= MAX_HEURISTIC_CANDIDATES => (
                candidates
                    .into_iter()
                    .map(|c| (c, EdgeOrigin::Heuristic))
                    .collect(),
                false,
            ),
            _ => (Vec::new(), true),
        }
    }

    fn with_constructor(
        &self,
        f: usize,
        s: usize,
        origin: EdgeOrigin,
        out: &mut Vec<(NodeRef, EdgeOrigin)>,
    ) {
        out.push((NodeRef { file: f, symbol: s }, origin));
        if matches!(
            self.files[f].parsed.symbols[s].kind,
            SymbolKind::Class | SymbolKind::Struct
        ) && let Some(ctor) = self.constructor_of(f, s)
        {
            out.push((
                NodeRef {
                    file: f,
                    symbol: ctor,
                },
                origin,
            ));
        }
    }

    fn module_files(&self, f: usize, path: &[String]) -> Vec<usize> {
        let bindings = &self.bindings[f];
        let mut files = Vec::new();
        if path.len() == 1
            && let Some(bounds) = bindings.by_local.get(&path[0])
        {
            for bound in bounds {
                if let Bound::Module { files: module } = bound {
                    files.extend(module.iter().copied());
                }
            }
        }
        let dotted = path.join(".");
        for (spec, module) in &bindings.dotted {
            if *spec == dotted {
                files.extend(module.iter().copied());
            }
        }
        files.sort_unstable();
        files.dedup();
        files
    }

    /// Resolves one call site. The flag is true when the call is ambiguous and was not followed.
    fn resolve_call(&self, f: usize, call: &CallSite) -> (Vec<(NodeRef, EdgeOrigin)>, bool) {
        let parsed = &self.files[f].parsed;
        let caller = NodeRef {
            file: f,
            symbol: call.caller,
        };
        let caller_symbol = &parsed.symbols[call.caller];
        let name = call.name.as_str();
        let bindings = &self.bindings[f];
        let mut out: Vec<(NodeRef, EdgeOrigin)> = Vec::new();

        match &call.receiver {
            Receiver::None => {
                if let Some(local) = self.local_lookup(f, call.caller, name) {
                    self.with_constructor(f, local, EdgeOrigin::Resolved, &mut out);
                    return (out, false);
                }
                for bound in bindings.by_local.get(name).into_iter().flatten() {
                    if let Bound::Symbol {
                        files,
                        name: target,
                    } = bound
                    {
                        for &tf in files {
                            for (ef, es) in self.exported(tf, target, 0) {
                                let origin = if files.len() == 1 {
                                    EdgeOrigin::Resolved
                                } else {
                                    EdgeOrigin::Heuristic
                                };
                                self.with_constructor(ef, es, origin, &mut out);
                            }
                        }
                    }
                }
                if !out.is_empty() {
                    return (out, false);
                }
                for &wildcard in &bindings.wildcards {
                    for (ef, es) in self.exported(wildcard, name, 0) {
                        self.with_constructor(ef, es, EdgeOrigin::Heuristic, &mut out);
                    }
                }
                if !out.is_empty() {
                    return (out, false);
                }
                // A function of the same file or of the project that is only reachable by name.
                if let Some(same_file) = self.top_level_symbols(f, name).into_iter().next() {
                    self.with_constructor(f, same_file, EdgeOrigin::Resolved, &mut out);
                    return (out, false);
                }
                self.by_name(name, caller, false)
            }
            Receiver::SelfRef => {
                let container = container_of(&caller_symbol.qualified_name);
                if !container.is_empty() {
                    if let Some(&method) = self
                        .by_qualified
                        .get(&(f, format!("{container}.{name}").as_str()))
                    {
                        return (
                            vec![(
                                NodeRef {
                                    file: f,
                                    symbol: method,
                                },
                                EdgeOrigin::Resolved,
                            )],
                            false,
                        );
                    }
                    // Inherited method: look through the class's bases.
                    let class = self
                        .by_qualified
                        .get(&(f, container))
                        .map(|c| &parsed.symbols[*c]);
                    for base in class.map(Self::bases_of).unwrap_or_default() {
                        if let Some((bf, bs)) = self.class_named(f, &base) {
                            let qualified = format!(
                                "{}.{name}",
                                self.files[bf].parsed.symbols[bs].qualified_name
                            );
                            if let Some(&method) = self.by_qualified.get(&(bf, qualified.as_str()))
                            {
                                return (
                                    vec![(
                                        NodeRef {
                                            file: bf,
                                            symbol: method,
                                        },
                                        EdgeOrigin::Resolved,
                                    )],
                                    false,
                                );
                            }
                        }
                    }
                }
                self.by_name(name, caller, true)
            }
            Receiver::Path(path) => {
                for module in self.module_files(f, path) {
                    for (ef, es) in self.exported(module, name, 0) {
                        self.with_constructor(ef, es, EdgeOrigin::Resolved, &mut out);
                    }
                }
                if !out.is_empty() {
                    return (out, false);
                }
                if path.len() == 1
                    && let Some((cf, cs)) = self.class_named(f, &path[0])
                {
                    let qualified = format!(
                        "{}.{name}",
                        self.files[cf].parsed.symbols[cs].qualified_name
                    );
                    if let Some(&method) = self.by_qualified.get(&(cf, qualified.as_str())) {
                        return (
                            vec![(
                                NodeRef {
                                    file: cf,
                                    symbol: method,
                                },
                                EdgeOrigin::Resolved,
                            )],
                            false,
                        );
                    }
                }
                self.by_name(name, caller, true)
            }
            Receiver::Complex => self.by_name(name, caller, true),
        }
    }

    fn entry_symbol(&self, hint: &EntrypointHint, f: usize) -> Option<usize> {
        hint.symbol
            .filter(|s| *s < self.files[f].parsed.symbols.len())
    }
}

fn better(a: EdgeOrigin, b: EdgeOrigin) -> EdgeOrigin {
    a.min(b)
}

/// Builds the graph of a project.
///
/// `files` should contain every parsed source file of the project in its "after" state;
/// `manifests` any `pyproject.toml` / `package.json` texts, which add entrypoints.
pub fn build_graph(files: Vec<IndexedFile>, manifests: &[ManifestText]) -> ProjectGraph {
    let builder = Builder::new(&files);

    let mut graph: DiGraph<NodeRef, CallEdge> = DiGraph::new();
    let mut node_of: Vec<Vec<NodeIndex>> = Vec::with_capacity(files.len());
    let mut keys: HashMap<SymbolKey, NodeIndex> = HashMap::new();
    for (f, file) in files.iter().enumerate() {
        let mut nodes = Vec::with_capacity(file.parsed.symbols.len());
        for (s, symbol) in file.parsed.symbols.iter().enumerate() {
            let node = graph.add_node(NodeRef { file: f, symbol: s });
            keys.entry(SymbolKey::new(
                file.path.clone(),
                symbol.qualified_name.clone(),
            ))
            .or_insert(node);
            nodes.push(node);
        }
        node_of.push(nodes);
    }

    // Call edges.
    let mut unresolved: HashMap<NodeIndex, u32> = HashMap::new();
    let mut edges: HashMap<(NodeIndex, NodeIndex), EdgeOrigin> = HashMap::new();
    for (f, file) in files.iter().enumerate() {
        for call in &file.parsed.calls {
            let from = node_of[f][call.caller];
            let (targets, ambiguous) = builder.resolve_call(f, call);
            if ambiguous {
                *unresolved.entry(from).or_default() += 1;
            }
            for (target, origin) in targets {
                let to = node_of[target.file][target.symbol];
                if to == from {
                    continue;
                }
                edges
                    .entry((from, to))
                    .and_modify(|o| *o = better(*o, origin))
                    .or_insert(origin);
            }
        }
    }
    let mut ordered: Vec<_> = edges.into_iter().collect();
    ordered.sort_by_key(|((from, to), _)| (from.index(), to.index()));
    for ((from, to), origin) in ordered {
        graph.add_edge(from, to, CallEdge { origin });
    }

    // Entrypoints from the parser.
    let mut entrypoints: HashMap<NodeIndex, Vec<Entrypoint>> = HashMap::new();
    for (f, file) in files.iter().enumerate() {
        for hint in &file.parsed.entrypoints {
            let Some(symbol) = builder.entry_symbol(hint, f) else {
                continue;
            };
            entrypoints
                .entry(node_of[f][symbol])
                .or_default()
                .push(Entrypoint {
                    symbol: SymbolKey::new(
                        file.path.clone(),
                        file.parsed.symbols[symbol].qualified_name.clone(),
                    ),
                    scope: hint.scope,
                    kind: hint.kind.clone(),
                    origin: hint.origin,
                    evidence: hint.evidence.clone(),
                });
        }
    }

    // Entrypoints from manifests.
    for manifest in manifests {
        let dir = manifest.path.parent().unwrap_or("");
        for entry in manifest::entries(manifest) {
            let mut add = |f: usize, s: usize, kind: klaub_core::graph::EntrypointKind| {
                entrypoints
                    .entry(node_of[f][s])
                    .or_default()
                    .push(Entrypoint {
                        symbol: SymbolKey::new(
                            files[f].path.clone(),
                            files[f].parsed.symbols[s].qualified_name.clone(),
                        ),
                        scope: klaub_core::graph::EntrypointScope::External,
                        kind,
                        origin: EdgeOrigin::Resolved,
                        evidence: entry.evidence.clone(),
                    });
            };
            match &entry.target {
                Target::PythonCallable { module, function } => {
                    for file in builder.index.python(manifest.path.as_str(), module, 0) {
                        for symbol in builder.top_level_symbols(file, function) {
                            add(file, symbol, klaub_core::graph::EntrypointKind::Cli);
                        }
                    }
                }
                Target::File(relative) => {
                    let joined = if dir.is_empty() {
                        relative.clone()
                    } else {
                        format!("{dir}/{relative}")
                    };
                    let found = builder.index.lookup_path(&joined).or_else(|| {
                        builder
                            .index
                            .javascript(manifest.path.as_str(), &format!("./{relative}"))
                            .first()
                            .copied()
                    });
                    let Some(file) = found else { continue };
                    match entry.usage {
                        Use::Script => add(file, 0, klaub_core::graph::EntrypointKind::Cli),
                        Use::PackageApi => {
                            for (s, symbol) in files[file].parsed.symbols.iter().enumerate() {
                                if symbol.parent == Some(0)
                                    && symbol.is_public()
                                    && is_callable_or_type(symbol.kind)
                                {
                                    add(file, s, klaub_core::graph::EntrypointKind::PublicExport);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for list in entrypoints.values_mut() {
        list.sort_by(|a, b| (a.scope, &a.evidence).cmp(&(b.scope, &b.evidence)));
        list.dedup_by(|a, b| a.evidence == b.evidence && a.kind == b.kind);
    }

    // File-level import edges.
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut import_edges = Vec::new();
    for (f, file) in files.iter().enumerate() {
        for import in &file.parsed.imports {
            let targets = builder.index.resolve(&files, f, import);
            let origin = if targets.len() == 1 {
                EdgeOrigin::Resolved
            } else {
                EdgeOrigin::Heuristic
            };
            for t in targets {
                if t != f && seen.insert((f, t)) {
                    import_edges.push(ImportEdge {
                        from: file.path.clone(),
                        to: files[t].path.clone(),
                        origin,
                    });
                }
            }
        }
    }
    import_edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));

    let Builder { index, .. } = builder;
    ProjectGraph {
        files,
        graph,
        keys,
        entrypoints,
        unresolved,
        import_edges,
        index,
    }
}

impl ProjectGraph {
    /// Number of symbols in the graph (including the module symbol of every file).
    pub fn symbol_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of resolved or heuristic call edges.
    pub fn call_edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Whether the graph knows `key`.
    pub fn contains(&self, key: &SymbolKey) -> bool {
        self.keys.contains_key(key)
    }

    /// File-level dependencies created by imports.
    pub fn import_edges(&self) -> &[ImportEdge] {
        &self.import_edges
    }

    /// The entrypoints declared at `key`, if any.
    pub fn entrypoints_at(&self, key: &SymbolKey) -> &[Entrypoint] {
        self.keys
            .get(key)
            .and_then(|n| self.entrypoints.get(n))
            .map_or(&[], Vec::as_slice)
    }

    /// Number of distinct symbols that call `key`.
    pub fn fan_in(&self, key: &SymbolKey) -> usize {
        self.keys.get(key).map_or(0, |n| {
            self.graph
                .neighbors_directed(*n, petgraph::Direction::Incoming)
                .count()
        })
    }

    /// The project files that `parsed`, as it was at `path` (possibly an older version), imports.
    ///
    /// This lets callers compare the dependencies of a file before and after a change using
    /// the same resolver.
    pub fn imported_files(&self, path: &RepoPath, parsed: &ParsedFile) -> Vec<RepoPath> {
        let mut out: Vec<RepoPath> = Vec::new();
        for import in &parsed.imports {
            let targets: Vec<usize> = match parsed.language {
                klaub_core::language::Language::Python => {
                    self.index
                        .python(path.as_str(), &import.module, import.relative_level)
                }
                klaub_core::language::Language::JavaScript
                | klaub_core::language::Language::TypeScript
                | klaub_core::language::Language::Tsx => {
                    self.index.javascript(path.as_str(), &import.module)
                }
                klaub_core::language::Language::Rust => {
                    self.index.rust(path.as_str(), &import.module)
                }
                klaub_core::language::Language::Go => self.index.go(&import.module),
                klaub_core::language::Language::Java => {
                    self.index.java(&import.module, import.wildcard)
                }
            };
            out.extend(
                targets
                    .into_iter()
                    .map(|t| self.files[t].path.clone())
                    .filter(|p| p != path),
            );
        }
        out.sort();
        out.dedup();
        out
    }

    /// Total number of ambiguous calls that were not followed.
    pub fn unresolved_calls(&self) -> u32 {
        self.unresolved.values().sum()
    }

    fn key_of(&self, node: NodeIndex) -> SymbolKey {
        let n = self.graph[node];
        SymbolKey::new(
            self.files[n.file].path.clone(),
            self.files[n.file].parsed.symbols[n.symbol]
                .qualified_name
                .clone(),
        )
    }

    fn file_of(&self, node: NodeIndex) -> &RepoPath {
        &self.files[self.graph[node].file].path
    }

    /// Dependency cycles between modules, formed by file imports. Each cycle lists its modules sorted.
    pub fn module_cycles(&self, modules: &ModuleMap) -> Vec<Vec<String>> {
        let mut ids: HashMap<String, NodeIndex> = HashMap::new();
        let mut module_graph: DiGraph<String, ()> = DiGraph::new();
        let mut node = |name: String, graph: &mut DiGraph<String, ()>| {
            *ids.entry(name.clone())
                .or_insert_with(|| graph.add_node(name))
        };
        let mut seen = HashSet::new();
        for edge in &self.import_edges {
            let (from, to) = (modules.module_of(&edge.from), modules.module_of(&edge.to));
            if from != to && seen.insert((from.clone(), to.clone())) {
                let (a, b) = (node(from, &mut module_graph), node(to, &mut module_graph));
                module_graph.add_edge(a, b, ());
            }
        }
        let mut cycles: Vec<Vec<String>> = petgraph::algo::tarjan_scc(&module_graph)
            .into_iter()
            .filter(|component| component.len() > 1)
            .map(|component| {
                let mut names: Vec<String> = component
                    .into_iter()
                    .map(|n| module_graph[n].clone())
                    .collect();
                names.sort();
                names
            })
            .collect();
        cycles.sort();
        cycles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_are_read_from_signatures() {
        let symbol = |signature: &str| Symbol {
            name: "A".into(),
            qualified_name: "A".into(),
            kind: SymbolKind::Class,
            signature: signature.into(),
            visibility: klaub_core::symbol::Visibility::Public,
            span: klaub_core::symbol::Span {
                start_byte: 0,
                end_byte: 0,
                start_line: 1,
                end_line: 1,
            },
            parent: Some(0),
            decorators: vec![],
            fingerprint: klaub_core::symbol::Fingerprint::default(),
            features: klaub_core::symbol::SymbolFeatures::default(),
        };
        assert_eq!(
            Builder::bases_of(&symbol("class A(Base, mixins.Other[T], metaclass=Meta)")),
            vec!["Base", "Other"]
        );
        assert_eq!(
            Builder::bases_of(&symbol("export class A extends Base<T> implements I")),
            vec!["Base"]
        );
        assert_eq!(Builder::bases_of(&symbol("class A")), Vec::<String>::new());
        assert_eq!(
            Builder::bases_of(&symbol("class A(object)")),
            Vec::<String>::new()
        );
    }
}

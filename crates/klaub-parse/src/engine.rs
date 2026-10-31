//! The language-independent extraction engine.
//!
//! Extraction runs in two passes over a tree-sitter syntax tree:
//!
//! 1. **Symbols.** The tree is walked once; whenever the language profile recognises a
//!    definition, a [`Symbol`] is created and its children are searched for nested ones.
//! 2. **Content.** Each symbol's *own* content (everything except nested symbols) is scanned
//!    to compute fingerprints, complexity, calls, imports and risk markers.
//!
//! Language-specific knowledge lives behind the [`Profile`] trait, so adding a language
//! means describing its grammar, not rewriting the engine.

use std::collections::HashMap;

use klaub_core::graph::{EdgeOrigin, EntrypointScope};
use klaub_core::language::Language;
use klaub_core::symbol::{
    CallSite, EntrypointHint, Fingerprint, HighlightClass, HighlightSpan, ImportRef, Marker,
    MarkerKind, ParsedFile, Receiver, Span, Symbol, SymbolFeatures, SymbolKind, Visibility,
};
use klaub_core::{Error, Result};
use tree_sitter::{Node, Parser};

/// Version of the extraction logic. Bump it whenever output for the same input can change.
pub const PARSER_VERSION: u32 = 2;

/// Deepest syntax nesting that is analysed; deeper content is ignored (and reported as an error).
const MAX_DEPTH: u32 = 250;

/// A definition recognised by a language profile.
pub(crate) struct Def<'t> {
    pub(crate) kind: SymbolKind,
    pub(crate) name: String,
    pub(crate) signature: String,
    pub(crate) visibility: Visibility,
    pub(crate) decorators: Vec<String>,
    pub(crate) is_async: bool,
    pub(crate) is_test: bool,
    pub(crate) param_count: u32,
    /// The node that identifies the symbol and whose subtree is its content.
    pub(crate) node: Node<'t>,
    /// The (possibly larger) node whose range is the symbol's span: decorators, `export`, ...
    pub(crate) span_node: Node<'t>,
    /// Prefix for the qualified names of children, when different from the symbol's own.
    pub(crate) child_prefix: Option<String>,
}

/// A call expression broken into the parts the engine and the markers need.
pub(crate) struct CallParts<'t> {
    pub(crate) name: String,
    pub(crate) receiver: Receiver,
    pub(crate) args: Vec<Node<'t>>,
    pub(crate) kwargs: Vec<(String, Node<'t>)>,
}

/// What entrypoint detection can see.
pub(crate) struct EntryCtx<'a, 't> {
    pub(crate) root: Node<'t>,
    pub(crate) src: &'t [u8],
    pub(crate) symbols: &'a [Symbol],
    pub(crate) nodes: &'a [Node<'t>],
}

/// Everything the engine needs to know about a language.
pub(crate) trait Profile: Sync {
    fn lang(&self) -> Language;
    fn ts_language(&self) -> tree_sitter::Language;
    /// Describes `node` if it defines a symbol. `parent` is the kind of the enclosing symbol.
    fn definition<'t>(&self, node: Node<'t>, src: &'t [u8], parent: SymbolKind) -> Option<Def<'t>>;
    fn is_comment(&self, kind: &str) -> bool;
    fn is_loop(&self, kind: &str) -> bool;
    /// Number of independent paths this node adds (0 for most nodes).
    fn decision_points(&self, node: Node<'_>, src: &[u8]) -> u32;
    fn is_nesting(&self, kind: &str) -> bool;
    fn call<'t>(&self, node: Node<'t>, src: &'t [u8]) -> Option<CallParts<'t>>;
    fn imports(&self, node: Node<'_>, src: &[u8]) -> Vec<ImportRef>;
    /// Markers that are visible on a single node (not a call).
    fn node_markers(&self, node: Node<'_>, src: &[u8]) -> Vec<(MarkerKind, String)>;
    /// Markers that are visible on a call.
    fn call_markers(
        &self,
        call: &CallParts<'_>,
        in_loop: bool,
        src: &[u8],
    ) -> Vec<(MarkerKind, String)>;
    /// The exception a `raise`/`throw` statement names, when `node` is one.
    fn raised(&self, _node: Node<'_>, _src: &[u8]) -> Option<String> {
        None
    }
    fn entrypoints(&self, ctx: &EntryCtx<'_, '_>) -> Vec<EntrypointHint>;
    fn highlight(&self, node: Node<'_>, src: &[u8]) -> Option<HighlightClass>;
}

pub(crate) fn fnv32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[derive(Clone, Copy)]
pub(crate) struct Fnv64(u64);

impl Fnv64 {
    pub(crate) fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
        // A separator so ["ab","c"] and ["a","bc"] hash differently.
        self.0 ^= 0x1f;
        self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
    }

    pub(crate) fn finish(self) -> u64 {
        self.0
    }
}

pub(crate) fn text<'a>(node: Node<'_>, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

pub(crate) fn collapse(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn line_of(node: Node<'_>) -> u32 {
    u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX)
}

fn span_of(node: Node<'_>) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    let end_line = if end.column == 0 && end.row > start.row {
        end.row
    } else {
        end.row + 1
    };
    Span {
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: u32::try_from(start.row + 1).unwrap_or(u32::MAX),
        end_line: u32::try_from(end_line).unwrap_or(u32::MAX),
    }
}

/// Header text of a definition: everything before its body, whitespace collapsed.
pub(crate) fn header(node: Node<'_>, src: &[u8], max_chars: usize) -> String {
    let end = node
        .child_by_field_name("body")
        .map_or(node.end_byte(), |body| body.start_byte());
    let raw = src
        .get(node.start_byte()..end)
        .and_then(|b| std::str::from_utf8(b).ok())
        .unwrap_or("");
    let collapsed = collapse(raw);
    let trimmed = collapsed.trim_end_matches([':', '{', ' ']).to_owned();
    if trimmed.chars().count() > max_chars {
        trimmed.chars().take(max_chars).collect()
    } else {
        trimmed
    }
}

struct Extractor<'p, 't> {
    profile: &'p dyn Profile,
    src: &'t [u8],
    symbols: Vec<Symbol>,
    nodes: Vec<Node<'t>>,
    prefixes: Vec<String>,
    def_nodes: HashMap<usize, usize>,
    imports: Vec<ImportRef>,
    calls: Vec<CallSite>,
    too_deep: bool,
}

impl<'t> Extractor<'_, 't> {
    fn collect(&mut self, node: Node<'t>, parent: usize, depth: u32) {
        if depth > MAX_DEPTH {
            self.too_deep = true;
            return;
        }
        let mut parent_for_children = parent;
        if let Some(def) = self
            .profile
            .definition(node, self.src, self.symbols[parent].kind)
        {
            let prefix = &self.prefixes[parent];
            let qualified = if prefix.is_empty() {
                def.name.clone()
            } else {
                format!("{prefix}.{}", def.name)
            };
            let child_prefix = def
                .child_prefix
                .clone()
                .unwrap_or_else(|| qualified.clone());
            let index = self.symbols.len();
            self.symbols.push(Symbol {
                name: def.name,
                qualified_name: qualified,
                kind: def.kind,
                signature: def.signature,
                visibility: def.visibility,
                span: span_of(def.span_node),
                parent: Some(parent),
                decorators: def.decorators,
                fingerprint: Fingerprint::default(),
                features: SymbolFeatures {
                    param_count: def.param_count,
                    is_test: def.is_test,
                    is_async: def.is_async,
                    ..SymbolFeatures::default()
                },
            });
            self.nodes.push(def.node);
            self.prefixes.push(child_prefix);
            self.def_nodes.insert(def.node.id(), index);
            parent_for_children = index;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect(child, parent_for_children, depth + 1);
        }
    }

    fn scan(&mut self, index: usize) {
        let node = self.nodes[index];
        let mut scan = Scan {
            profile: self.profile,
            src: self.src,
            own_id: node.id(),
            symbol: index,
            def_nodes: &self.def_nodes,
            tokens: Vec::new(),
            token_hash: Fnv64::new(),
            shape_hash: Fnv64::new(),
            complexity: 1,
            max_nesting: 0,
            markers: Vec::new(),
            raises: Vec::new(),
            io_calls: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            too_deep: false,
        };
        scan.visit(node, 0, 0, 0);

        let mut text_hash = Fnv64::new();
        let own = self.symbols[index].span;
        let mut holes: Vec<(usize, usize)> = self
            .symbols
            .iter()
            .filter(|s| s.parent == Some(index))
            .map(|s| (s.span.start_byte, s.span.end_byte))
            .collect();
        holes.sort_unstable();
        let mut cursor = own.start_byte;
        for (start, end) in holes {
            if start > cursor {
                text_hash.write(self.src.get(cursor..start).unwrap_or(&[]));
            }
            cursor = cursor.max(end);
        }
        text_hash.write(
            self.src
                .get(cursor.min(own.end_byte)..own.end_byte)
                .unwrap_or(&[]),
        );

        let symbol = &mut self.symbols[index];
        symbol.fingerprint = Fingerprint {
            shape_hash: scan.shape_hash.finish(),
            token_hash: scan.token_hash.finish(),
            text_hash: text_hash.finish(),
            tokens: scan.tokens,
        };
        symbol.features.complexity = scan.complexity;
        symbol.features.max_nesting = scan.max_nesting;
        symbol.features.line_count = symbol.span.line_count();
        symbol.features.markers = scan.markers;
        symbol.features.raises = scan.raises;
        symbol.features.io_calls = scan.io_calls;
        self.imports.extend(scan.imports);
        self.calls.extend(scan.calls);
        self.too_deep |= scan.too_deep;
    }
}

struct Scan<'a, 'p, 't> {
    profile: &'p dyn Profile,
    src: &'t [u8],
    own_id: usize,
    symbol: usize,
    def_nodes: &'a HashMap<usize, usize>,
    tokens: Vec<u32>,
    token_hash: Fnv64,
    shape_hash: Fnv64,
    complexity: u32,
    max_nesting: u32,
    markers: Vec<Marker>,
    raises: Vec<String>,
    io_calls: Vec<String>,
    imports: Vec<ImportRef>,
    calls: Vec<CallSite>,
    too_deep: bool,
}

impl<'t> Scan<'_, '_, 't> {
    fn visit(&mut self, node: Node<'t>, depth: u32, nesting: u32, loops: u32) {
        if depth > MAX_DEPTH {
            self.too_deep = true;
            return;
        }
        if node.id() != self.own_id && self.def_nodes.contains_key(&node.id()) {
            // A nested symbol is a unit of its own: its content, and even its presence,
            // must not change the fingerprint of the symbol that contains it.
            return;
        }
        let kind = node.kind();
        if self.profile.is_comment(kind) {
            return;
        }
        if node.child_count() == 0 {
            let leaf = text(node, self.src).as_bytes();
            // The root of an empty file is a childless node with no text: not a token.
            if !leaf.is_empty() {
                self.tokens.push(fnv32(leaf));
                self.token_hash.write(leaf);
            }
        }
        self.shape_hash.write(kind.as_bytes());

        self.complexity += self.profile.decision_points(node, self.src);
        let nesting = if self.profile.is_nesting(kind) {
            self.max_nesting = self.max_nesting.max(nesting + 1);
            nesting + 1
        } else {
            nesting
        };

        if let Some(call) = self.profile.call(node, self.src) {
            let line = line_of(node);
            for (marker, detail) in self.profile.call_markers(&call, loops > 0, self.src) {
                self.markers.push(Marker {
                    kind: marker,
                    line,
                    detail,
                });
            }
            if crate::markers::io_call(&call.receiver, &call.name) {
                let label = match &call.receiver {
                    Receiver::Path(path) if !path.is_empty() => {
                        format!("{}.{}", path.join("."), call.name)
                    }
                    _ => call.name.clone(),
                };
                if !self.io_calls.contains(&label) {
                    self.io_calls.push(label);
                }
            }
            self.calls.push(CallSite {
                caller: self.symbol,
                name: call.name,
                receiver: call.receiver,
                line,
                in_loop: loops > 0,
            });
        }
        if let Some(raised) = self.profile.raised(node, self.src)
            && !self.raises.contains(&raised)
        {
            self.raises.push(raised);
        }
        self.imports.extend(self.profile.imports(node, self.src));
        for (marker, detail) in self.profile.node_markers(node, self.src) {
            self.markers.push(Marker {
                kind: marker,
                line: line_of(node),
                detail,
            });
        }

        let loops = loops + u32::from(self.profile.is_loop(kind));
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child, depth + 1, nesting, loops);
        }
    }
}

/// Parses `source` with `profile` and extracts symbols, calls, imports and entrypoints.
pub(crate) fn extract(profile: &dyn Profile, source: &str) -> Result<ParsedFile> {
    let mut parser = Parser::new();
    parser.set_language(&profile.ts_language()).map_err(|e| {
        Error::Parse(format!(
            "cannot load the {} grammar: {e}",
            profile.lang().name()
        ))
    })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| Error::Parse(format!("the {} parser gave up", profile.lang().name())))?;
    let root = tree.root_node();
    let src = source.as_bytes();

    let module = Symbol {
        name: "<module>".to_owned(),
        qualified_name: "<module>".to_owned(),
        kind: SymbolKind::Module,
        signature: String::new(),
        visibility: Visibility::Public,
        span: Span {
            start_byte: 0,
            end_byte: src.len(),
            start_line: 1,
            end_line: u32::try_from(source.lines().count().max(1)).unwrap_or(u32::MAX),
        },
        parent: None,
        decorators: Vec::new(),
        fingerprint: Fingerprint::default(),
        features: SymbolFeatures::default(),
    };
    let mut extractor = Extractor {
        profile,
        src,
        symbols: vec![module],
        nodes: vec![root],
        prefixes: vec![String::new()],
        def_nodes: HashMap::new(),
        imports: Vec::new(),
        calls: Vec::new(),
        too_deep: false,
    };
    // Children of the root only: the root itself is the module symbol.
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        extractor.collect(child, 0, 1);
    }
    for index in 0..extractor.symbols.len() {
        extractor.scan(index);
    }

    let entrypoints = profile.entrypoints(&EntryCtx {
        root,
        src,
        symbols: &extractor.symbols,
        nodes: &extractor.nodes,
    });
    Ok(ParsedFile {
        language: profile.lang(),
        symbols: extractor.symbols,
        imports: extractor.imports,
        calls: extractor.calls,
        entrypoints,
        has_syntax_errors: root.has_error() || extractor.too_deep,
        parser_version: PARSER_VERSION,
    })
}

/// Syntax highlighting spans, in source order and non-overlapping.
pub(crate) fn highlight(profile: &dyn Profile, source: &str) -> Result<Vec<HighlightSpan>> {
    let mut parser = Parser::new();
    parser.set_language(&profile.ts_language()).map_err(|e| {
        Error::Parse(format!(
            "cannot load the {} grammar: {e}",
            profile.lang().name()
        ))
    })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| Error::Parse("the parser gave up".to_owned()))?;
    let mut spans = Vec::new();
    let src = source.as_bytes();
    let mut stack = vec![(tree.root_node(), 0_u32)];
    while let Some((node, depth)) = stack.pop() {
        if let Some(class) = profile.highlight(node, src) {
            spans.push(HighlightSpan {
                start: node.start_byte(),
                end: node.end_byte(),
                class,
            });
            continue; // the whole node has one colour; do not descend
        }
        if depth < MAX_DEPTH {
            let mut cursor = node.walk();
            let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
            stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
        }
    }
    spans.sort_by_key(|span| span.start);
    Ok(spans)
}

/// Helpers shared by profiles.
pub(crate) fn string_value(node: Node<'_>, src: &[u8]) -> Option<String> {
    let raw = text(node, src);
    let trimmed = raw.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let quote = trimmed.chars().next()?;
    if !matches!(quote, '"' | '\'' | '`') {
        return None;
    }
    let inner = trimmed.trim_matches(quote);
    let inner = inner.trim_start_matches([quote, quote]);
    Some(inner.to_owned())
}

pub(crate) fn external(
    origin: EdgeOrigin,
    symbol: Option<usize>,
    kind: klaub_core::graph::EntrypointKind,
    evidence: String,
    line: u32,
) -> EntrypointHint {
    EntrypointHint {
        symbol,
        scope: EntrypointScope::External,
        kind,
        origin,
        evidence,
        line,
    }
}

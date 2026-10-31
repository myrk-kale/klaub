//! What a parser reports about one source file.
//!
//! These types are language-agnostic. Each parser adapter maps its grammar onto
//! them, so matching, graph building and rules never see a syntax tree.

use serde::{Deserialize, Serialize};

use crate::graph::{EdgeOrigin, EntrypointKind, EntrypointScope};
use crate::language::Language;

/// A region of a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start_byte: usize,
    /// End byte offset (exclusive).
    pub end_byte: usize,
    /// First line, 1-based.
    pub start_line: u32,
    /// Last line, 1-based and inclusive.
    pub end_line: u32,
}

impl Span {
    /// Number of lines covered.
    pub fn line_count(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Whether `line` (1-based) lies inside the span.
    pub fn contains_line(&self, line: u32) -> bool {
        (self.start_line..=self.end_line).contains(&line)
    }

    /// Whether the two spans share at least one line.
    pub fn overlaps_lines(&self, other: &Span) -> bool {
        self.start_line <= other.end_line && other.start_line <= self.end_line
    }
}

/// The kind of a named code unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    /// The file itself (top-level code).
    Module,
    /// A class.
    Class,
    /// An interface (TypeScript, Go).
    Interface,
    /// A struct (Rust, Go).
    Struct,
    /// An enum.
    Enum,
    /// A trait (Rust).
    Trait,
    /// A type alias.
    TypeAlias,
    /// A free function.
    Function,
    /// A function that belongs to a class, struct or impl.
    Method,
    /// A named constant.
    Constant,
    /// An `impl` block (Rust).
    Impl,
}

impl SymbolKind {
    /// A short lower-case label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::TypeAlias => "type",
            Self::Function => "fn",
            Self::Method => "method",
            Self::Constant => "const",
            Self::Impl => "impl",
        }
    }

    /// Whether calls can target this kind.
    pub fn is_callable(self) -> bool {
        matches!(
            self,
            Self::Function | Self::Method | Self::Class | Self::Struct
        )
    }

    /// Whether this kind groups other symbols.
    pub fn is_container(self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::Class
                | Self::Interface
                | Self::Struct
                | Self::Enum
                | Self::Trait
                | Self::Impl
        )
    }
}

/// Who can use a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Exported or public.
    Public,
    /// Visible to subclasses.
    Protected,
    /// Private to its module or type.
    Private,
    /// Visible inside the package or crate only.
    Internal,
    /// The language has no visibility concept or it could not be determined.
    Unknown,
}

/// Hashes that identify a symbol's content at different levels of strictness.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Hash of the syntax shape with identifiers and literals abstracted away.
    pub shape_hash: u64,
    /// Hash of the tokens, ignoring whitespace and comments.
    pub token_hash: u64,
    /// Hash of the raw text of the symbol's own content.
    pub text_hash: u64,
    /// Token hashes in source order, comments excluded, nested symbols excluded.
    pub tokens: Vec<u32>,
}

/// Kinds of structural evidence a parser can report for a rule to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerKind {
    /// An error handler that discards the error (`except: pass`, empty `catch`, `let _ =`).
    SwallowedError,
    /// A query string assembled by concatenation or interpolation and passed to a database call.
    RawSqlConcat,
    /// A string literal assigned to a name that looks like a secret.
    SecretLiteral,
    /// A call to an I/O-looking function inside a loop.
    CallInLoop,
    /// `eval`, `exec`, `new Function` and similar.
    DynamicExecution,
    /// A shell is invoked (`os.system`, `shell=True`, `child_process.exec`).
    ShellExecution,
    /// An `unsafe` block.
    UnsafeBlock,
    /// Certificate or TLS verification is turned off.
    VerificationDisabled,
}

/// One observation of a [`MarkerKind`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    /// What was observed.
    pub kind: MarkerKind,
    /// Line of the observation (1-based).
    pub line: u32,
    /// A short description (never the full source text).
    pub detail: String,
}

/// Measurements of a symbol used by rules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolFeatures {
    /// Cyclomatic complexity (1 + number of decision points).
    pub complexity: u32,
    /// Deepest nesting of control flow.
    pub max_nesting: u32,
    /// Number of lines.
    pub line_count: u32,
    /// Number of declared parameters.
    pub param_count: u32,
    /// The symbol is a test (test function, `#[test]`, name convention).
    pub is_test: bool,
    /// The symbol is asynchronous.
    pub is_async: bool,
    /// Structural evidence for rules.
    pub markers: Vec<Marker>,
    /// Names of the exceptions the body raises or throws explicitly (`ValueError`), in source
    /// order and without repeats. Only what is written in the body: nothing is inferred from
    /// the functions it calls.
    #[serde(default)]
    pub raises: Vec<String>,
    /// Calls that look like network, file or database I/O (`requests.get`, `cursor.execute`),
    /// in source order and without repeats. A judgement from the call's name and receiver, not
    /// from types.
    #[serde(default)]
    pub io_calls: Vec<String>,
}

/// A named code unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    /// The simple name (`validate`).
    pub name: String,
    /// The dotted path from the file (`SessionService.validate`); `<module>` for the file itself.
    pub qualified_name: String,
    /// What kind of unit it is.
    pub kind: SymbolKind,
    /// The normalised declaration without the body (`def validate(self, token: str) -> bool`).
    pub signature: String,
    /// Who can use it.
    pub visibility: Visibility,
    /// Where it is, including decorators and attributes.
    pub span: Span,
    /// Index of the enclosing symbol in [`ParsedFile::symbols`].
    pub parent: Option<usize>,
    /// Decorators or attributes as written, without the `@`.
    pub decorators: Vec<String>,
    /// Content hashes.
    pub fingerprint: Fingerprint,
    /// Measurements and markers.
    pub features: SymbolFeatures,
}

impl Symbol {
    /// Whether the symbol is exported or public.
    pub fn is_public(&self) -> bool {
        self.visibility == Visibility::Public
    }
}

/// An imported name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedName {
    /// The name in the imported module (`default` for a JavaScript default import).
    pub name: String,
    /// The local alias, if any.
    pub alias: Option<String>,
}

/// An import statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRef {
    /// The module as written (`os.path`, `./util`, `crate::a::b`, `net/http`).
    pub module: String,
    /// Number of leading dots of a relative Python import.
    pub relative_level: u32,
    /// Names pulled out of the module.
    pub names: Vec<ImportedName>,
    /// Alias of the module itself (`import numpy as np`, `import * as ns`).
    pub alias: Option<String>,
    /// A wildcard import (`from x import *`, `import * as ns`).
    pub wildcard: bool,
    /// The import is a re-export (`export { a } from './b'`).
    pub reexport: bool,
    /// The import is computed at run time (`require`, `import()`).
    pub dynamic: bool,
    /// Line of the statement (1-based).
    pub line: u32,
}

/// The expression a method is called on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Receiver {
    /// A plain call: `validate(x)`.
    None,
    /// `self`, `this`, `cls` or the Rust receiver.
    SelfRef,
    /// A dotted path of identifiers: `auth.tokens.validate(x)` has `["auth", "tokens"]`.
    Path(Vec<String>),
    /// Anything else (a call result, an index, ...).
    Complex,
}

/// A call expression inside a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSite {
    /// Index of the enclosing symbol in [`ParsedFile::symbols`].
    pub caller: usize,
    /// The called name (`validate`).
    pub name: String,
    /// What it is called on.
    pub receiver: Receiver,
    /// Line of the call (1-based).
    pub line: u32,
    /// The call is inside a loop body.
    pub in_loop: bool,
}

/// A symbol the parser recognised as an entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointHint {
    /// Index of the entrypoint symbol; `None` for the module-level `__main__` block.
    pub symbol: Option<usize>,
    /// Internal or external.
    pub scope: EntrypointScope,
    /// Its kind.
    pub kind: EntrypointKind,
    /// `Resolved` only when the recognition is unambiguous.
    pub origin: EdgeOrigin,
    /// Why it was recognised.
    pub evidence: String,
    /// Line of the evidence (1-based).
    pub line: u32,
}

/// The parser's view of one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFile {
    /// The language that was parsed.
    pub language: Language,
    /// All symbols in source order. Index 0 is the module symbol covering the whole file.
    pub symbols: Vec<Symbol>,
    /// Import statements.
    pub imports: Vec<ImportRef>,
    /// Call expressions.
    pub calls: Vec<CallSite>,
    /// Recognised entrypoints.
    pub entrypoints: Vec<EntrypointHint>,
    /// The grammar reported syntax errors; results may be partial.
    pub has_syntax_errors: bool,
    /// Version of the extraction logic, part of cache keys.
    pub parser_version: u32,
}

impl ParsedFile {
    /// The module-level symbol, when present.
    pub fn module_symbol(&self) -> Option<&Symbol> {
        self.symbols
            .first()
            .filter(|s| s.kind == SymbolKind::Module)
    }

    /// Indexes of the direct children of `parent`.
    pub fn children_of(&self, parent: usize) -> impl Iterator<Item = usize> + '_ {
        self.symbols
            .iter()
            .enumerate()
            .filter(move |(_, s)| s.parent == Some(parent))
            .map(|(i, _)| i)
    }

    /// Index of the symbol with the given qualified name.
    pub fn find(&self, qualified_name: &str) -> Option<usize> {
        self.symbols
            .iter()
            .position(|s| s.qualified_name == qualified_name)
    }

    /// The innermost non-module symbol containing `line`, falling back to the module symbol.
    pub fn enclosing(&self, line: u32) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (index, symbol) in self.symbols.iter().enumerate() {
            if !symbol.span.contains_line(line) {
                continue;
            }
            let better = match best {
                None => true,
                Some(current) => {
                    let current = &self.symbols[current].span;
                    symbol.span.line_count() <= current.line_count()
                }
            };
            if better {
                best = Some(index);
            }
        }
        best
    }
}

/// Categories of syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightClass {
    /// A language keyword.
    Keyword,
    /// A string literal.
    String,
    /// A comment.
    Comment,
    /// A number literal.
    Number,
    /// A type name.
    Type,
    /// A function or method name at its definition.
    Function,
}

/// A highlighted byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightSpan {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
    /// The category.
    pub class: HighlightClass,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(name: &str, kind: SymbolKind, start: u32, end: u32, parent: Option<usize>) -> Symbol {
        Symbol {
            name: name.into(),
            qualified_name: name.into(),
            kind,
            signature: String::new(),
            visibility: Visibility::Public,
            span: Span {
                start_byte: 0,
                end_byte: 0,
                start_line: start,
                end_line: end,
            },
            parent,
            decorators: vec![],
            fingerprint: Fingerprint::default(),
            features: SymbolFeatures::default(),
        }
    }

    fn file() -> ParsedFile {
        ParsedFile {
            language: Language::Python,
            symbols: vec![
                sym("<module>", SymbolKind::Module, 1, 30, None),
                sym("A", SymbolKind::Class, 2, 20, Some(0)),
                sym("m", SymbolKind::Method, 3, 10, Some(1)),
                sym("f", SymbolKind::Function, 22, 30, Some(0)),
            ],
            imports: vec![],
            calls: vec![],
            entrypoints: vec![],
            has_syntax_errors: false,
            parser_version: 1,
        }
    }

    #[test]
    fn enclosing_picks_the_innermost_symbol() {
        let file = file();
        assert_eq!(file.enclosing(5), Some(2));
        assert_eq!(file.enclosing(15), Some(1));
        assert_eq!(file.enclosing(21), Some(0));
        assert_eq!(file.enclosing(25), Some(3));
    }

    #[test]
    fn children_and_lookup() {
        let file = file();
        assert_eq!(file.children_of(0).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(file.find("m"), Some(2));
        assert_eq!(file.module_symbol().unwrap().name, "<module>");
    }

    #[test]
    fn span_helpers() {
        let a = Span {
            start_byte: 0,
            end_byte: 1,
            start_line: 3,
            end_line: 5,
        };
        let b = Span {
            start_byte: 0,
            end_byte: 1,
            start_line: 5,
            end_line: 9,
        };
        let c = Span {
            start_byte: 0,
            end_byte: 1,
            start_line: 6,
            end_line: 9,
        };
        assert_eq!(a.line_count(), 3);
        assert!(a.overlaps_lines(&b));
        assert!(!a.overlaps_lines(&c));
        assert!(a.contains_line(4));
    }
}

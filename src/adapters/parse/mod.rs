//! The tree-sitter adapter of Klaub. It implements the [`SourceParser`] port: source text in,
//! language-agnostic [`ParsedFile`] out (symbols with signatures, visibility, fingerprints and
//! structural markers; calls; imports; entrypoint hints; highlight spans).
//!
//! Supported languages: Python, JavaScript, TypeScript (and TSX), Rust, Go and Java. Parsing is
//! fault tolerant: a file with syntax errors still yields every symbol the grammar could
//! recover and sets `has_syntax_errors`.

mod engine;
mod go;
mod java;
mod javascript;
mod markers;
mod python;
mod rust_lang;

use crate::domain::Result;
use crate::domain::language::Language;
use crate::domain::ports::SourceParser;
use crate::domain::symbol::{HighlightSpan, ParsedFile};

pub use engine::PARSER_VERSION;

/// The tree-sitter implementation of [`SourceParser`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TreeSitterParser;

impl TreeSitterParser {
    /// Creates a parser. It holds no state; a tree-sitter parser is created per call.
    pub fn new() -> Self {
        Self
    }
}

fn profile(language: Language) -> &'static dyn engine::Profile {
    match language {
        Language::Python => &python::Python,
        Language::JavaScript => &javascript::JavaScript {
            language: Language::JavaScript,
        },
        Language::TypeScript => &javascript::JavaScript {
            language: Language::TypeScript,
        },
        Language::Tsx => &javascript::JavaScript {
            language: Language::Tsx,
        },
        Language::Rust => &rust_lang::RustLang,
        Language::Go => &go::Go,
        Language::Java => &java::Java,
    }
}

impl SourceParser for TreeSitterParser {
    fn supports(&self, _language: Language) -> bool {
        true
    }

    fn parser_version(&self) -> u32 {
        PARSER_VERSION
    }

    fn parse(&self, language: Language, source: &str) -> Result<ParsedFile> {
        engine::extract(profile(language), source)
    }

    fn highlight(&self, language: Language, source: &str) -> Result<Vec<HighlightSpan>> {
        engine::highlight(profile(language), source)
    }
}

//! Supported source languages.

use serde::{Deserialize, Serialize};

use crate::path::RepoPath;

/// A language Klaub can parse into symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Python (priority 0).
    Python,
    /// JavaScript, including JSX (priority 0).
    JavaScript,
    /// TypeScript (priority 0).
    TypeScript,
    /// TypeScript with JSX.
    Tsx,
    /// Rust (priority 1).
    Rust,
    /// Go (priority 1).
    Go,
    /// Java (priority 1).
    Java,
}

impl Language {
    /// Every supported language.
    pub const ALL: [Self; 7] = [
        Self::Python,
        Self::JavaScript,
        Self::TypeScript,
        Self::Tsx,
        Self::Rust,
        Self::Go,
        Self::Java,
    ];

    /// Detects the language from a file extension.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "py" | "pyi" => Some(Self::Python),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            _ => None,
        }
    }

    /// Detects the language of a repository path.
    pub fn from_path(path: &RepoPath) -> Option<Self> {
        path.extension().and_then(Self::from_extension)
    }

    /// The name used in configuration (`languages = ["python", ...]`) and diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Java => "java",
        }
    }

    /// Parses a configuration name. `"typescript"` also enables TSX.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|language| language.name() == name.to_ascii_lowercase())
    }

    /// Whether `self` is enabled by a configured language name.
    pub fn enabled_by(self, configured: &str) -> bool {
        let configured = configured.to_ascii_lowercase();
        self.name() == configured || (self == Self::Tsx && configured == "typescript")
    }

    /// Whether this is a JavaScript family language (JavaScript, TypeScript, TSX).
    pub fn is_js_family(self) -> bool {
        matches!(self, Self::JavaScript | Self::TypeScript | Self::Tsx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_languages_from_paths() {
        let lang = |p: &str| Language::from_path(&RepoPath::new(p).unwrap());
        assert_eq!(lang("a/b.py"), Some(Language::Python));
        assert_eq!(lang("a/b.PYI"), Some(Language::Python));
        assert_eq!(lang("web/app.tsx"), Some(Language::Tsx));
        assert_eq!(lang("web/app.mjs"), Some(Language::JavaScript));
        assert_eq!(lang("src/lib.rs"), Some(Language::Rust));
        assert_eq!(lang("main.go"), Some(Language::Go));
        assert_eq!(lang("src/main/java/a/B.java"), Some(Language::Java));
        assert_eq!(lang("README.md"), None);
        assert_eq!(lang("Makefile"), None);
    }

    #[test]
    fn typescript_configuration_enables_tsx() {
        assert!(Language::Tsx.enabled_by("typescript"));
        assert!(Language::TypeScript.enabled_by("TypeScript"));
        assert!(!Language::Python.enabled_by("typescript"));
    }
}

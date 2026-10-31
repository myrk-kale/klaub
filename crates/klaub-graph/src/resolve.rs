//! Resolving import specifiers to project files.
//!
//! Resolution is by path conventions only (no interpreter, no package manager), so it is
//! deliberately modest: when a specifier matches several files, the one closest to the importing
//! file wins and ties are returned together for the caller to treat as a guess.

use std::collections::HashMap;

use klaub_core::language::Language;
use klaub_core::symbol::ImportRef;

use crate::IndexedFile;

const JS_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts"];

/// Lookup tables for resolving imports.
pub(crate) struct ModuleIndex {
    by_path: HashMap<String, usize>,
    /// Dotted suffixes of Python module paths (`pkg.mod`) to files.
    dotted: HashMap<String, Vec<usize>>,
    /// Directory to the files directly inside it (Go packages).
    by_dir: HashMap<String, Vec<usize>>,
}

fn components(path: &str) -> Vec<&str> {
    path.split('/').filter(|c| !c.is_empty()).collect()
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn join(dir: &str, rest: &str) -> String {
    if dir.is_empty() {
        rest.to_owned()
    } else {
        format!("{dir}/{rest}")
    }
}

/// Collapses `.` and `..` components. `None` if the path escapes the root.
fn normalize(path: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for part in components(path) {
        match part {
            "." => {}
            ".." => {
                out.pop()?;
            }
            other => out.push(other),
        }
    }
    Some(out.join("/"))
}

fn shared_prefix(a: &str, b: &str) -> usize {
    components(a)
        .iter()
        .zip(components(b))
        .take_while(|(x, y)| **x == *y)
        .count()
}

impl ModuleIndex {
    pub(crate) fn new(files: &[IndexedFile]) -> Self {
        let mut index = Self {
            by_path: HashMap::new(),
            dotted: HashMap::new(),
            by_dir: HashMap::new(),
        };
        for (i, file) in files.iter().enumerate() {
            let path = file.path.as_str();
            index.by_path.insert(path.to_owned(), i);
            index
                .by_dir
                .entry(dir_of(path).to_owned())
                .or_default()
                .push(i);
            if file.parsed.language == Language::Python {
                let stem = path
                    .strip_suffix(".pyi")
                    .or_else(|| path.strip_suffix(".py"))
                    .unwrap_or(path);
                let mut parts = components(stem);
                if parts.last() == Some(&"__init__") {
                    parts.pop();
                }
                for start in 0..parts.len() {
                    index
                        .dotted
                        .entry(parts[start..].join("."))
                        .or_default()
                        .push(i);
                }
            }
        }
        index
    }

    /// Files an import of `import` made from file `from` may refer to.
    pub(crate) fn resolve(
        &self,
        files: &[IndexedFile],
        from: usize,
        import: &ImportRef,
    ) -> Vec<usize> {
        let origin = &files[from];
        match origin.parsed.language {
            Language::Python => {
                self.python(origin.path.as_str(), &import.module, import.relative_level)
            }
            Language::JavaScript | Language::TypeScript | Language::Tsx => {
                self.javascript(origin.path.as_str(), &import.module)
            }
            Language::Rust => self.rust(origin.path.as_str(), &import.module),
            Language::Go => self.go(&import.module),
        }
    }

    pub(crate) fn python(&self, from: &str, module: &str, level: u32) -> Vec<usize> {
        if level > 0 {
            let mut base = dir_of(from).to_owned();
            for _ in 1..level {
                let parent = dir_of(&base).to_owned();
                base = parent;
            }
            let target = if module.is_empty() {
                base
            } else {
                join(&base, &module.replace('.', "/"))
            };
            let candidates = if module.is_empty() {
                vec![join(&target, "__init__.py")]
            } else {
                vec![
                    format!("{target}.py"),
                    format!("{target}.pyi"),
                    join(&target, "__init__.py"),
                ]
            };
            return candidates
                .iter()
                .filter_map(|c| self.by_path.get(c).copied())
                .take(1)
                .collect();
        }
        let Some(found) = self.dotted.get(module) else {
            return Vec::new();
        };
        let best = found
            .iter()
            .map(|i| shared_prefix(from, &self.path_of(*i)))
            .max()
            .unwrap_or(0);
        found
            .iter()
            .copied()
            .filter(|i| shared_prefix(from, &self.path_of(*i)) == best)
            .collect()
    }

    fn path_of(&self, index: usize) -> String {
        self.by_path
            .iter()
            .find(|(_, i)| **i == index)
            .map(|(p, _)| p.clone())
            .unwrap_or_default()
    }

    pub(crate) fn javascript(&self, from: &str, specifier: &str) -> Vec<usize> {
        if !specifier.starts_with('.') {
            return Vec::new(); // a package from node_modules or a path alias
        }
        let Some(base) = normalize(&join(dir_of(from), specifier)) else {
            return Vec::new();
        };
        let mut candidates = vec![base.clone()];
        if let Some((stem, ext)) = base.rsplit_once('.')
            && matches!(ext, "js" | "jsx" | "mjs" | "cjs")
        {
            candidates.extend(
                ["ts", "tsx", "mts", "cts"]
                    .iter()
                    .map(|e| format!("{stem}.{e}")),
            );
        }
        candidates.extend(JS_EXTENSIONS.iter().map(|e| format!("{base}.{e}")));
        candidates.extend(
            JS_EXTENSIONS
                .iter()
                .map(|e| join(&base, &format!("index.{e}"))),
        );
        candidates
            .iter()
            .find_map(|c| self.by_path.get(c).copied())
            .into_iter()
            .collect()
    }

    pub(crate) fn rust(&self, from: &str, module: &str) -> Vec<usize> {
        let segments: Vec<&str> = module.split("::").filter(|s| !s.is_empty()).collect();
        let Some(first) = segments.first() else {
            return Vec::new();
        };
        let file_name = from.rsplit('/').next().unwrap_or("");
        let mod_like = matches!(file_name, "mod.rs" | "lib.rs" | "main.rs");
        let (base, rest): (String, &[&str]) = match *first {
            "crate" => (self.crate_src_dir(from), &segments[1..]),
            // The module of `a/b.rs` is `a::b`, whose children live in `a/b/`; for `mod.rs`
            // and crate roots the module is the directory itself.
            "self" => (
                if mod_like {
                    dir_of(from).to_owned()
                } else {
                    from.strip_suffix(".rs").unwrap_or(from).to_owned()
                },
                &segments[1..],
            ),
            "super" => (
                if mod_like {
                    dir_of(dir_of(from)).to_owned()
                } else {
                    dir_of(from).to_owned()
                },
                &segments[1..],
            ),
            _ => return Vec::new(), // an external crate (or `std`)
        };
        // The last segment may be an item rather than a module: try shorter prefixes too.
        for take in (1..=rest.len()).rev() {
            let target = join(&base, &rest[..take].join("/"));
            for candidate in [format!("{target}.rs"), join(&target, "mod.rs")] {
                if let Some(found) = self.by_path.get(&candidate) {
                    return vec![*found];
                }
            }
        }
        if rest.is_empty() {
            return ["lib.rs", "main.rs"]
                .iter()
                .filter_map(|f| self.by_path.get(&join(&base, f)).copied())
                .take(1)
                .collect();
        }
        Vec::new()
    }

    /// The `src` directory of the crate that contains `from`.
    fn crate_src_dir(&self, from: &str) -> String {
        let mut dir = dir_of(from);
        loop {
            if self.by_path.contains_key(&join(dir, "lib.rs"))
                || self.by_path.contains_key(&join(dir, "main.rs"))
            {
                return dir.to_owned();
            }
            if dir.is_empty() {
                return dir_of(from).to_owned();
            }
            dir = dir_of(dir);
        }
    }

    pub(crate) fn go(&self, import_path: &str) -> Vec<usize> {
        let parts = components(import_path);
        for take in (1..=parts.len()).rev() {
            let suffix = parts[parts.len() - take..].join("/");
            let found: Vec<usize> = self
                .by_dir
                .iter()
                .filter(|(dir, _)| **dir == suffix || dir.ends_with(&format!("/{suffix}")))
                .flat_map(|(_, files)| files.iter().copied())
                .collect();
            if !found.is_empty() {
                return found;
            }
        }
        Vec::new()
    }

    pub(crate) fn lookup_path(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klaub_core::path::RepoPath;
    use std::sync::Arc;

    use klaub_core::symbol::{
        Fingerprint, ParsedFile, Span, Symbol, SymbolFeatures, SymbolKind, Visibility,
    };

    fn file(path: &str, language: Language) -> IndexedFile {
        let module = Symbol {
            name: "<module>".into(),
            qualified_name: "<module>".into(),
            kind: SymbolKind::Module,
            signature: String::new(),
            visibility: Visibility::Public,
            span: Span {
                start_byte: 0,
                end_byte: 0,
                start_line: 1,
                end_line: 1,
            },
            parent: None,
            decorators: vec![],
            fingerprint: Fingerprint::default(),
            features: SymbolFeatures::default(),
        };
        IndexedFile {
            path: RepoPath::new(path).unwrap(),
            parsed: Arc::new(ParsedFile {
                language,
                symbols: vec![module],
                imports: vec![],
                calls: vec![],
                entrypoints: vec![],
                has_syntax_errors: false,
                parser_version: 1,
            }),
        }
    }

    fn paths(files: &[IndexedFile], found: &[usize]) -> Vec<String> {
        found
            .iter()
            .map(|i| files[*i].path.as_str().to_owned())
            .collect()
    }

    #[test]
    fn python_absolute_relative_and_packages() {
        let files = vec![
            file("src/auth/tokens.py", Language::Python),
            file("src/auth/__init__.py", Language::Python),
            file("src/app/views.py", Language::Python),
            file("src/shared/util.py", Language::Python),
        ];
        let index = ModuleIndex::new(&files);
        assert_eq!(
            paths(&files, &index.python("src/app/views.py", "auth.tokens", 0)),
            vec!["src/auth/tokens.py"]
        );
        assert_eq!(
            paths(&files, &index.python("src/app/views.py", "auth", 0)),
            vec!["src/auth/__init__.py"]
        );
        assert_eq!(
            paths(&files, &index.python("src/auth/tokens.py", "", 1)),
            vec!["src/auth/__init__.py"]
        );
        assert_eq!(
            paths(&files, &index.python("src/app/views.py", "util", 2)),
            Vec::<String>::new(),
            "..util is shared/util only via ..shared"
        );
        assert_eq!(
            paths(&files, &index.python("src/app/views.py", "shared.util", 2)),
            vec!["src/shared/util.py"]
        );
        assert!(index.python("src/app/views.py", "numpy", 0).is_empty());
    }

    #[test]
    fn javascript_relative_specifiers() {
        let files = vec![
            file("web/app.ts", Language::TypeScript),
            file("web/tokens.ts", Language::TypeScript),
            file("web/lib/index.ts", Language::TypeScript),
            file("shared/util.js", Language::JavaScript),
        ];
        let index = ModuleIndex::new(&files);
        assert_eq!(
            paths(&files, &index.javascript("web/app.ts", "./tokens")),
            vec!["web/tokens.ts"]
        );
        assert_eq!(
            paths(&files, &index.javascript("web/app.ts", "./tokens.js")),
            vec!["web/tokens.ts"],
            "TS ESM imports name .js"
        );
        assert_eq!(
            paths(&files, &index.javascript("web/app.ts", "./lib")),
            vec!["web/lib/index.ts"]
        );
        assert_eq!(
            paths(&files, &index.javascript("web/app.ts", "../shared/util")),
            vec!["shared/util.js"]
        );
        assert!(index.javascript("web/app.ts", "react").is_empty());
        assert!(index.javascript("web/app.ts", "../../outside").is_empty());
    }

    #[test]
    fn rust_crate_paths() {
        let files = vec![
            file("src/lib.rs", Language::Rust),
            file("src/auth/mod.rs", Language::Rust),
            file("src/auth/tokens.rs", Language::Rust),
            file("src/main_helpers.rs", Language::Rust),
        ];
        let index = ModuleIndex::new(&files);
        assert_eq!(
            paths(&files, &index.rust("src/lib.rs", "crate::auth::tokens")),
            vec!["src/auth/tokens.rs"]
        );
        assert_eq!(
            paths(&files, &index.rust("src/lib.rs", "crate::auth")),
            vec!["src/auth/mod.rs"]
        );
        assert_eq!(
            paths(&files, &index.rust("src/auth/mod.rs", "self::tokens")),
            vec!["src/auth/tokens.rs"]
        );
        assert_eq!(
            paths(&files, &index.rust("src/auth/tokens.rs", "super::tokens")),
            vec!["src/auth/tokens.rs"]
        );
        assert!(index.rust("src/lib.rs", "std::collections").is_empty());
    }

    #[test]
    fn go_packages_by_directory_suffix() {
        let files = vec![
            file("pkg/auth/tokens.go", Language::Go),
            file("pkg/auth/session.go", Language::Go),
            file("cmd/app/main.go", Language::Go),
        ];
        let index = ModuleIndex::new(&files);
        assert_eq!(index.go("github.com/acme/app/pkg/auth").len(), 2);
        assert!(index.go("net/http").is_empty());
    }

    #[test]
    fn normalisation() {
        assert_eq!(normalize("a/./b/../c").as_deref(), Some("a/c"));
        assert_eq!(normalize("../x"), None);
    }
}

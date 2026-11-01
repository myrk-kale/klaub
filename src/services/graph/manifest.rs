//! Entrypoints declared by package manifests (`pyproject.toml`, `package.json`).

use crate::domain::path::RepoPath;

/// The text of a manifest file in the project.
#[derive(Debug, Clone)]
pub struct ManifestText {
    /// Where it lives.
    pub path: RepoPath,
    /// Its content.
    pub text: String,
}

/// What a manifest points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// A Python `module.path:function` reference.
    PythonCallable { module: String, function: String },
    /// A file path relative to the manifest's directory.
    File(String),
}

/// How the target is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Use {
    /// A command line script.
    Script,
    /// The public API of a package.
    PackageApi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestEntry {
    pub(crate) target: Target,
    pub(crate) usage: Use,
    pub(crate) evidence: String,
}

/// Extracts the entries of a manifest; unknown or malformed manifests yield nothing.
pub(crate) fn entries(manifest: &ManifestText) -> Vec<ManifestEntry> {
    match manifest.path.file_name() {
        "pyproject.toml" => pyproject(&manifest.text),
        "package.json" => package_json(&manifest.text),
        _ => Vec::new(),
    }
}

fn pyproject(text: &str) -> Vec<ManifestEntry> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let scripts = |path: &[&str]| -> Option<&toml::Table> {
        let mut current = &table;
        for key in path {
            current = current.get(*key)?.as_table()?;
        }
        Some(current)
    };
    let mut out = Vec::new();
    for (path, label) in [
        (&["project", "scripts"][..], "[project.scripts]"),
        (&["tool", "poetry", "scripts"][..], "[tool.poetry.scripts]"),
    ] {
        for (name, value) in scripts(path).into_iter().flatten() {
            let Some((module, function)) = value.as_str().and_then(|v| v.split_once(':')) else {
                continue;
            };
            out.push(ManifestEntry {
                target: Target::PythonCallable {
                    module: module.trim().to_owned(),
                    function: function.trim().to_owned(),
                },
                usage: Use::Script,
                evidence: format!("pyproject.toml {label} `{name}`"),
            });
        }
    }
    out
}

fn package_json(text: &str) -> Vec<ManifestEntry> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let file = |s: &str| Target::File(s.trim_start_matches("./").to_owned());
    match value.get("bin") {
        Some(serde_json::Value::String(path)) => {
            out.push(ManifestEntry {
                target: file(path),
                usage: Use::Script,
                evidence: "package.json bin".to_owned(),
            });
        }
        Some(serde_json::Value::Object(map)) => {
            for (name, path) in map {
                if let Some(path) = path.as_str() {
                    out.push(ManifestEntry {
                        target: file(path),
                        usage: Use::Script,
                        evidence: format!("package.json bin `{name}`"),
                    });
                }
            }
        }
        _ => {}
    }
    for key in ["main", "module"] {
        if let Some(path) = value.get(key).and_then(|v| v.as_str()) {
            out.push(ManifestEntry {
                target: file(path),
                usage: Use::PackageApi,
                evidence: format!("package.json {key}"),
            });
        }
    }
    let mut stack = vec![value.get("exports")];
    while let Some(Some(node)) = stack.pop() {
        match node {
            serde_json::Value::String(path) if path.starts_with("./") => {
                out.push(ManifestEntry {
                    target: file(path),
                    usage: Use::PackageApi,
                    evidence: "package.json exports".to_owned(),
                });
            }
            serde_json::Value::Object(map) => stack.extend(map.values().map(Some)),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(path: &str, text: &str) -> ManifestText {
        ManifestText {
            path: RepoPath::new(path).unwrap(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn pyproject_scripts() {
        let found = entries(&manifest(
            "pyproject.toml",
            "[project.scripts]\nserve = \"app.cli:main\"\n[tool.poetry.scripts]\nother = \"pkg.run:go\"\n[tool.black]\nline-length = 100\n",
        ));
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].target,
            Target::PythonCallable {
                module: "app.cli".into(),
                function: "main".into()
            }
        );
        assert_eq!(found[0].usage, Use::Script);
        assert!(found[1].evidence.contains("tool.poetry"));
    }

    #[test]
    fn package_json_entries() {
        let found = entries(&manifest(
            "web/package.json",
            r#"{"main":"./dist/index.js","bin":{"tool":"bin/cli.js"},"exports":{".":{"import":"./esm/index.js","require":"./cjs/index.js"},"./x":"./x.js"}}"#,
        ));
        let files: Vec<_> = found.iter().map(|e| (e.target.clone(), e.usage)).collect();
        assert!(files.contains(&(Target::File("dist/index.js".into()), Use::PackageApi)));
        assert!(files.contains(&(Target::File("bin/cli.js".into()), Use::Script)));
        assert!(files.contains(&(Target::File("esm/index.js".into()), Use::PackageApi)));
        assert!(files.contains(&(Target::File("x.js".into()), Use::PackageApi)));
    }

    #[test]
    fn malformed_manifests_are_ignored() {
        assert_eq!(entries(&manifest("pyproject.toml", "not toml [[[")), vec![]);
        assert_eq!(entries(&manifest("package.json", "{")), vec![]);
        assert_eq!(entries(&manifest("README.md", "# hi")), vec![]);
    }
}

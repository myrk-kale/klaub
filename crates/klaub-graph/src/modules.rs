//! Which module a file belongs to.
//!
//! "Module" here is an architectural unit, not a language module: a configured root such as
//! `src/domain`, or, without configuration, an explicit heuristic based on the directory layout.

use klaub_core::path::RepoPath;

/// Directory names under which each child directory is a module of its own (`crates/<name>`).
const CONTAINER_DIRS: &[&str] = &[
    "src", "lib", "packages", "apps", "crates", "internal", "pkg", "cmd", "services", "modules",
];

/// Assigns files to modules.
#[derive(Debug, Clone, Default)]
pub struct ModuleMap {
    roots: Vec<String>,
}

impl ModuleMap {
    /// Uses configured module roots (directories); the longest matching root wins.
    pub fn configured(roots: &[String]) -> Self {
        let mut roots: Vec<String> = roots
            .iter()
            .map(|r| r.trim_end_matches('/').to_owned())
            .filter(|r| !r.is_empty())
            .collect();
        roots.sort_by_key(|r| std::cmp::Reverse(r.len()));
        Self { roots }
    }

    /// Whether modules come from configuration (as opposed to the directory heuristic).
    pub fn is_configured(&self) -> bool {
        !self.roots.is_empty()
    }

    /// The module of a file.
    ///
    /// With configured roots, files outside every root belong to `<other>`. Without them,
    /// the first directory below a container directory (`src`, `crates`, ...) names the module,
    /// otherwise the first directory does, and files at the root belong to `<root>`.
    pub fn module_of(&self, path: &RepoPath) -> String {
        if self.is_configured() {
            return self
                .roots
                .iter()
                .find(|root| path.is_within(root))
                .cloned()
                .unwrap_or_else(|| "<other>".to_owned());
        }
        let parts: Vec<&str> = path.as_str().split('/').collect();
        match parts.as_slice() {
            [_file] => "<root>".to_owned(),
            [first, second, _, ..] if CONTAINER_DIRS.contains(first) => format!("{first}/{second}"),
            [first, ..] => (*first).to_owned(),
            [] => "<root>".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(map: &ModuleMap, path: &str) -> String {
        map.module_of(&RepoPath::new(path).unwrap())
    }

    #[test]
    fn heuristic_modules() {
        let map = ModuleMap::default();
        assert_eq!(module(&map, "setup.py"), "<root>");
        assert_eq!(module(&map, "src/api/routes.py"), "src/api");
        assert_eq!(
            module(&map, "crates/klaub-core/src/lib.rs"),
            "crates/klaub-core"
        );
        assert_eq!(module(&map, "auth/session.py"), "auth");
        assert_eq!(
            module(&map, "src/main.py"),
            "src",
            "a file directly in a container directory"
        );
    }

    #[test]
    fn configured_roots_win_and_the_longest_matches() {
        let map =
            ModuleMap::configured(&["src/domain".into(), "src".into(), "src/domain/auth".into()]);
        assert_eq!(module(&map, "src/domain/auth/x.py"), "src/domain/auth");
        assert_eq!(module(&map, "src/domain/y.py"), "src/domain");
        assert_eq!(module(&map, "src/infra/z.py"), "src");
        assert_eq!(module(&map, "docs/a.md"), "<other>");
        assert_eq!(
            module(&map, "src/domainx/y.py"),
            "src",
            "whole path components only"
        );
    }
}

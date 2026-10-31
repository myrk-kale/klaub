//! Resolution of user-level directories from the environment.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use klaub_core::dirs::AppDirs;

/// A lookup of environment variables, replaceable in tests.
pub(crate) type Lookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn absolute(get: Lookup<'_>, key: &str) -> Option<PathBuf> {
    get(key)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The user's home directory.
pub(crate) fn home(get: Lookup<'_>) -> Option<PathBuf> {
    absolute(get, "HOME")
}

/// Resolves Klaub's cache, state and configuration directories.
///
/// `KLAUB_CACHE_DIR`, `KLAUB_STATE_DIR` and `KLAUB_CONFIG_DIR` override everything.
/// Otherwise Linux follows the XDG base directories and macOS uses
/// `~/Library/Caches/klaub` and `~/Library/Application Support/klaub`, with the
/// configuration under `~/.config/klaub`.
pub(crate) fn resolve_dirs(get: Lookup<'_>) -> anyhow::Result<AppDirs> {
    let home = home(get);
    let macos = cfg!(target_os = "macos");
    let need_home = || {
        home.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "HOME is not set; set KLAUB_CACHE_DIR, KLAUB_STATE_DIR and KLAUB_CONFIG_DIR"
            )
        })
    };

    let cache = match absolute(get, "KLAUB_CACHE_DIR") {
        Some(dir) => dir,
        None if macos => need_home()?.join("Library/Caches/klaub"),
        None => absolute(get, "XDG_CACHE_HOME")
            .map_or_else(|| need_home().map(|h| h.join(".cache")), Ok)?
            .join("klaub"),
    };
    let state = match absolute(get, "KLAUB_STATE_DIR") {
        Some(dir) => dir,
        None if macos => need_home()?.join("Library/Application Support/klaub"),
        None => absolute(get, "XDG_STATE_HOME")
            .map_or_else(|| need_home().map(|h| h.join(".local/state")), Ok)?
            .join("klaub"),
    };
    let config = match absolute(get, "KLAUB_CONFIG_DIR") {
        Some(dir) => dir,
        None => absolute(get, "XDG_CONFIG_HOME")
            .map_or_else(|| need_home().map(|h| h.join(".config")), Ok)?
            .join("klaub"),
    };
    Ok(AppDirs::new(cache, state, config))
}

/// Walks up from `start` to the directory that contains `.git` (a directory or a file, as in linked worktrees).
pub(crate) fn find_worktree_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn explicit_overrides_win() {
        let get = lookup(&[
            ("KLAUB_CACHE_DIR", "/c"),
            ("KLAUB_STATE_DIR", "/s"),
            ("KLAUB_CONFIG_DIR", "/g"),
        ]);
        let dirs = resolve_dirs(&get).unwrap();
        assert_eq!(
            (dirs.cache, dirs.state, dirs.config),
            ("/c".into(), "/s".into(), "/g".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_uses_library_directories() {
        let dirs = resolve_dirs(&lookup(&[("HOME", "/Users/ana")])).unwrap();
        assert_eq!(dirs.cache, Path::new("/Users/ana/Library/Caches/klaub"));
        assert_eq!(
            dirs.state,
            Path::new("/Users/ana/Library/Application Support/klaub")
        );
        assert_eq!(dirs.config, Path::new("/Users/ana/.config/klaub"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn linux_follows_xdg() {
        let dirs = resolve_dirs(&lookup(&[("HOME", "/home/ana")])).unwrap();
        assert_eq!(dirs.cache, Path::new("/home/ana/.cache/klaub"));
        assert_eq!(dirs.state, Path::new("/home/ana/.local/state/klaub"));
        let dirs = resolve_dirs(&lookup(&[
            ("HOME", "/home/ana"),
            ("XDG_CACHE_HOME", "/x/cache"),
        ]))
        .unwrap();
        assert_eq!(dirs.cache, Path::new("/x/cache/klaub"));
    }

    #[test]
    fn relative_xdg_values_are_ignored_and_missing_home_is_an_error() {
        assert!(resolve_dirs(&lookup(&[])).is_err());
        let dirs =
            resolve_dirs(&lookup(&[("HOME", "/h"), ("XDG_CONFIG_HOME", "relative")])).unwrap();
        assert_eq!(dirs.config, Path::new("/h/.config/klaub"));
    }

    #[test]
    fn finds_the_worktree_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join(".git"), "gitdir: /elsewhere").unwrap();
        assert_eq!(
            find_worktree_root(&root.join("a/b")).as_deref(),
            Some(root.as_path())
        );
    }
}

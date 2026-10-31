//! File system helpers shared by the stores: private directories, atomic writes, bounded reads.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use klaub_core::{Error, Result};

/// Creates `dir` (and parents) with owner-only permissions.
pub(crate) fn private_dir(dir: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| Error::io(dir.display(), e))
}

/// Refuses to continue when any existing component of `path` below `root` is a symbolic link.
///
/// `.klaub/` lives inside a repository the user may not fully trust: a symlinked `.klaub` must
/// not redirect writes (or reads) outside the worktree.
pub(crate) fn ensure_no_symlinks_below(root: &Path, path: &Path) -> Result<()> {
    let relative = path.strip_prefix(root).map_err(|_| {
        Error::invalid(
            "path",
            format!("{} is outside {}", path.display(), root.display()),
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(Error::invalid(
                "path",
                format!("unexpected component in {}", path.display()),
            ));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::invalid(
                    "path",
                    format!(
                        "{} is a symbolic link; Klaub does not follow links for its own data",
                        current.display()
                    ),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(Error::io(current.display(), e)),
        }
    }
    Ok(())
}

/// Writes `bytes` to `path` atomically: temporary file in the same directory, flushed, renamed over
/// the target, directory flushed. A crash leaves either the old or the new content, never a mix.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::Storage(format!("{} has no parent directory", path.display())))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp: PathBuf = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temp);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|e| Error::io(temp.display(), e))?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(e) = written {
        let _ = fs::remove_file(&temp);
        return Err(Error::io(temp.display(), e));
    }
    if let Err(e) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(Error::io(path.display(), e));
    }
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Reads at most `max_bytes` from `path`; fails when the file is larger.
pub(crate) fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| Error::io(path.display(), e))?;
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io(path.display(), e))?;
    if bytes.len() as u64 > max_bytes {
        return Err(Error::invalid(
            "file",
            format!("{} is larger than {max_bytes} bytes", path.display()),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn atomic_writes_replace_content_and_are_private() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("a.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let leftovers: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(
            leftovers.len(),
            1,
            "no temporary files remain: {leftovers:?}"
        );
    }

    #[test]
    fn a_stale_temporary_file_does_not_block_or_corrupt() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("a.json");
        write_atomic(&path, b"good").unwrap();
        // A previous process died after creating its temporary file.
        fs::write(
            temp.path()
                .join(format!(".a.json.{}.tmp", std::process::id())),
            b"half-writ",
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"good");
        write_atomic(&path, b"better").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"better");
    }

    #[test]
    fn symlinks_below_the_root_are_refused() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join(".klaub")).unwrap();
        let error = ensure_no_symlinks_below(&root, &root.join(".klaub/reviews")).unwrap_err();
        assert!(error.to_string().contains("symbolic link"), "{error}");
        fs::create_dir_all(root.join("plain/dir")).unwrap();
        ensure_no_symlinks_below(&root, &root.join("plain/dir/file")).unwrap();
        assert!(ensure_no_symlinks_below(&root, Path::new("/elsewhere/x")).is_err());
    }

    #[test]
    fn bounded_reads() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("f");
        fs::write(&path, b"12345").unwrap();
        assert_eq!(read_bounded(&path, 5).unwrap(), b"12345");
        assert!(read_bounded(&path, 4).is_err());
        assert!(read_bounded(&temp.path().join("missing"), 4).is_err());
    }
}

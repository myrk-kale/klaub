//! Locating, extracting and verifying the tmux executable.
//!
//! Release builds embed a gzip-compressed tmux together with the SHA-256 of the
//! decompressed executable. It is extracted once into a private directory and
//! its hash is verified **before every execution**. A development build without
//! an embedded asset never guesses: the user must name a tmux binary explicitly.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;
use klaub_core::{Error, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Oldest tmux that supports everything Klaub uses (`new-session -e`, `-P -F`, hooks).
pub const MIN_TMUX_VERSION: (u32, u32) = (3, 2);

/// Upper bound for a decompressed tmux executable.
const MAX_EXECUTABLE_BYTES: u64 = 64 * 1024 * 1024;

#[cfg(klaub_embedded_tmux)]
static EMBEDDED_GZ: &[u8] = include_bytes!(env!("KLAUB_TMUX_GZ"));
#[cfg(klaub_embedded_tmux)]
const EMBEDDED_MANIFEST: &str = include_str!(env!("KLAUB_TMUX_MANIFEST"));

/// Describes the embedded executable. Written by `scripts/build-tmux.sh`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    /// Manifest format version.
    pub schema_version: u32,
    /// The tmux release (`3.8`).
    pub tmux_version: String,
    /// The Rust target triple it was built for.
    pub target: String,
    /// SHA-256 of the decompressed executable, lowercase hexadecimal.
    pub sha256: String,
}

/// The manifest of the tmux compiled into this build, if there is one.
pub fn embedded_manifest() -> Option<Manifest> {
    #[cfg(klaub_embedded_tmux)]
    {
        serde_json::from_str(EMBEDDED_MANIFEST).ok()
    }
    #[cfg(not(klaub_embedded_tmux))]
    {
        None
    }
}

/// Where the tmux binary in use came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxSource {
    /// Extracted from this executable and hash-verified.
    Embedded {
        /// The tmux release.
        version: String,
        /// Verified SHA-256.
        sha256: String,
    },
    /// Named explicitly by the user (`--tmux-path`, `KLAUB_TMUX_PATH`). Not verified.
    Explicit,
}

/// A tmux executable ready to be run.
#[derive(Debug, Clone)]
pub struct ResolvedTmux {
    /// Absolute path of the executable.
    pub path: PathBuf,
    /// Where it came from.
    pub source: TmuxSource,
    /// Version as reported by `tmux -V` (`3.8`).
    pub version: String,
}

/// Chooses the tmux binary: an explicit path wins, otherwise the embedded one.
///
/// `bin_dir` is where the embedded executable is extracted (`<cache>/bin`).
pub fn resolve(explicit: Option<&Path>, bin_dir: &Path) -> Result<ResolvedTmux> {
    if let Some(path) = explicit {
        return resolve_explicit(path);
    }
    if let Some(manifest) = embedded_manifest() {
        #[cfg(klaub_embedded_tmux)]
        {
            let path = install_from_gz(bin_dir, EMBEDDED_GZ, &manifest)?;
            return Ok(ResolvedTmux {
                path,
                version: manifest.tmux_version.clone(),
                source: TmuxSource::Embedded {
                    version: manifest.tmux_version,
                    sha256: manifest.sha256,
                },
            });
        }
        #[cfg(not(klaub_embedded_tmux))]
        {
            let _ = (manifest, bin_dir);
        }
    }
    Err(Error::Tmux(
        "this build of klaub does not embed tmux; pass --tmux-path <path> or set KLAUB_TMUX_PATH \
         (release builds embed a verified tmux, see docs/adr/0004-embedded-verified-tmux.md)"
            .to_owned(),
    ))
}

fn resolve_explicit(path: &Path) -> Result<ResolvedTmux> {
    let metadata = fs::metadata(path).map_err(|e| Error::io(path.display(), e))?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(Error::Tmux(format!(
            "{} is not an executable file",
            path.display()
        )));
    }
    let path = path
        .canonicalize()
        .map_err(|e| Error::io(path.display(), e))?;
    let version = version_of(&path)?;
    check_minimum(&version)?;
    Ok(ResolvedTmux {
        path,
        source: TmuxSource::Explicit,
        version,
    })
}

/// Runs `tmux -V` and returns the version (`3.5a`).
pub fn version_of(path: &Path) -> Result<String> {
    let output = Command::new(path)
        .arg("-V")
        .env_remove("TMUX")
        .output()
        .map_err(|e| Error::Tmux(format!("cannot run {}: {e}", path.display())))?;
    if !output.status.success() {
        return Err(Error::Tmux(format!("{} -V failed", path.display())));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.trim().strip_prefix("tmux ").unwrap_or(text.trim());
    if version.is_empty() {
        return Err(Error::Tmux(format!(
            "{} did not report a version",
            path.display()
        )));
    }
    Ok(version.to_owned())
}

/// Extracts `(major, minor)` from strings such as `3.5a`, `3.7c` or `next-3.6`.
pub fn parse_version(version: &str) -> Option<(u32, u32)> {
    let version = version.trim().trim_start_matches("next-");
    let mut parts = version.splitn(2, '.');
    let major = parts.next()?.parse().ok()?;
    let minor_text = parts.next()?;
    let digits: String = minor_text
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some((major, digits.parse().ok()?))
}

fn check_minimum(version: &str) -> Result<()> {
    match parse_version(version) {
        Some(found) if found >= MIN_TMUX_VERSION => Ok(()),
        Some(_) | None => Err(Error::Tmux(format!(
            "tmux {version} is not supported; Klaub needs tmux {}.{} or newer",
            MIN_TMUX_VERSION.0, MIN_TMUX_VERSION.1
        ))),
    }
}

/// Lowercase hexadecimal SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

fn private_dir(path: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|e| Error::io(path.display(), e))
}

/// Installs a gzip-compressed executable under `bin_dir` and returns its path.
///
/// The decompressed bytes must match `manifest.sha256`. An existing installation
/// is reused only if its hash matches; otherwise it is replaced atomically.
pub fn install_from_gz(bin_dir: &Path, gz: &[u8], manifest: &Manifest) -> Result<PathBuf> {
    let dir = bin_dir.join(format!(
        "tmux-{}-{}",
        manifest.tmux_version,
        &manifest.sha256[..manifest.sha256.len().min(12)]
    ));
    private_dir(&dir)?;
    let target = dir.join("tmux");

    if verify_installed(&target, &manifest.sha256).is_ok() {
        return Ok(target);
    }

    let mut bytes = Vec::new();
    GzDecoder::new(gz)
        .take(MAX_EXECUTABLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Tmux(format!("cannot decompress the embedded tmux: {e}")))?;
    if bytes.len() as u64 > MAX_EXECUTABLE_BYTES {
        return Err(Error::Tmux("embedded tmux is implausibly large".to_owned()));
    }
    let actual = sha256_hex(&bytes);
    if actual != manifest.sha256 {
        return Err(Error::Tmux(format!(
            "embedded tmux failed verification (expected sha256 {}, got {actual})",
            manifest.sha256
        )));
    }

    let temp = dir.join(format!(".tmux.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temp);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&temp)
        .map_err(|e| Error::io(temp.display(), e))?;
    file.write_all(&bytes)
        .map_err(|e| Error::io(temp.display(), e))?;
    file.sync_all().map_err(|e| Error::io(temp.display(), e))?;
    drop(file);
    fs::rename(&temp, &target).map_err(|e| Error::io(target.display(), e))?;
    if let Ok(dir_handle) = fs::File::open(&dir) {
        let _ = dir_handle.sync_all();
    }
    verify_installed(&target, &manifest.sha256)?;
    Ok(target)
}

/// Checks that the file at `path` still has the expected hash and is private to the user.
pub fn verify_installed(path: &Path, expected_sha256: &str) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|e| Error::io(path.display(), e))?;
    if metadata.permissions().mode() & 0o022 != 0 {
        return Err(Error::Tmux(format!(
            "{} is writable by other users",
            path.display()
        )));
    }
    let bytes = fs::read(path).map_err(|e| Error::io(path.display(), e))?;
    let actual = sha256_hex(&bytes);
    if actual == expected_sha256 {
        Ok(())
    } else {
        Err(Error::Tmux(format!(
            "{} does not match its expected sha256",
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    const SCRIPT: &[u8] = b"#!/bin/sh\necho tmux 3.9z\n";

    fn gz(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn manifest(sha256: String) -> Manifest {
        Manifest {
            schema_version: 1,
            tmux_version: "3.9z".into(),
            target: "test".into(),
            sha256,
        }
    }

    #[test]
    fn parses_tmux_version_strings() {
        assert_eq!(parse_version("3.5a"), Some((3, 5)));
        assert_eq!(parse_version("3.7c"), Some((3, 7)));
        assert_eq!(parse_version("next-3.6"), Some((3, 6)));
        assert_eq!(parse_version("3.10"), Some((3, 10)));
        assert_eq!(parse_version("master"), None);
        assert!(check_minimum("3.1").is_err());
        assert!(check_minimum("3.2").is_ok());
        assert!(check_minimum("2.9").is_err());
    }

    #[test]
    fn hashes_are_lowercase_hex() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn installs_privately_and_runs() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = manifest(sha256_hex(SCRIPT));
        let path = install_from_gz(temp.path(), &gz(SCRIPT), &manifest).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let dir_mode = fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(version_of(&path).unwrap(), "3.9z");
    }

    #[test]
    fn a_wrong_hash_is_refused_and_nothing_is_installed() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = manifest("0".repeat(64));
        let error = install_from_gz(temp.path(), &gz(SCRIPT), &manifest).unwrap_err();
        assert!(error.to_string().contains("failed verification"), "{error}");
        let installed = temp
            .path()
            .join(format!("tmux-3.9z-{}", "0".repeat(12)))
            .join("tmux");
        assert!(!installed.exists());
    }

    #[test]
    fn a_tampered_installation_is_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = manifest(sha256_hex(SCRIPT));
        let path = install_from_gz(temp.path(), &gz(SCRIPT), &manifest).unwrap();
        fs::write(&path, b"#!/bin/sh\necho evil\n").unwrap();
        assert!(verify_installed(&path, &manifest.sha256).is_err());
        let again = install_from_gz(temp.path(), &gz(SCRIPT), &manifest).unwrap();
        assert_eq!(again, path);
        verify_installed(&again, &manifest.sha256).unwrap();
    }

    #[test]
    fn group_writable_installations_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = manifest(sha256_hex(SCRIPT));
        let path = install_from_gz(temp.path(), &gz(SCRIPT), &manifest).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o770)).unwrap();
        assert!(verify_installed(&path, &manifest.sha256).is_err());
    }

    #[test]
    fn explicit_paths_must_be_executable_files() {
        let temp = tempfile::tempdir().unwrap();
        let plain = temp.path().join("tmux");
        fs::write(&plain, SCRIPT).unwrap();
        assert!(resolve(Some(&plain), temp.path()).is_err());
        fs::set_permissions(&plain, fs::Permissions::from_mode(0o700)).unwrap();
        // The fake reports 3.9z, which satisfies the minimum.
        let resolved = resolve(Some(&plain), temp.path()).unwrap();
        assert_eq!(resolved.version, "3.9z");
        assert_eq!(resolved.source, TmuxSource::Explicit);
    }

    #[cfg(not(klaub_embedded_tmux))]
    #[test]
    fn without_an_embedded_asset_nothing_is_guessed() {
        let temp = tempfile::tempdir().unwrap();
        let error = resolve(None, temp.path()).unwrap_err().to_string();
        assert!(error.contains("--tmux-path"), "{error}");
    }
}

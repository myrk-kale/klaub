//! Running the `git` executable safely.
//!
//! Every call uses an argument array (never a shell), a scrubbed environment and
//! flags that stop repository configuration from executing programs
//! (`--no-ext-diff`, `--no-textconv`, `core.fsmonitor=false`). Repositories are
//! untrusted input: a malicious `.git/config` must not run code just because
//! someone reviews the repository.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use klaub_core::{Error, Result};

/// A handle to the `git` executable.
#[derive(Debug, Clone)]
pub struct GitRunner {
    binary: PathBuf,
}

impl Default for GitRunner {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("git"),
        }
    }
}

impl GitRunner {
    /// Uses `git` from `PATH`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Uses a specific git executable.
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    fn command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .current_dir(cwd)
            .args([
                "--no-optional-locks",
                "-c",
                "core.quotepath=false",
                "-c",
                "core.fsmonitor=false",
            ])
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_PAGER", "cat")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE");
        command
    }

    /// Runs git and returns its standard output.
    pub fn run<I, S>(&self, cwd: &Path, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<S> = args.into_iter().collect();
        let output = self
            .command(cwd)
            .args(args.iter().map(AsRef::as_ref))
            .stdin(Stdio::null())
            .output()
            .map_err(|e| Error::Git(format!("cannot run {}: {e}", self.binary.display())))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            let name = args
                .first()
                .map(|a| a.as_ref().to_string_lossy().into_owned())
                .unwrap_or_default();
            let stderr = String::from_utf8_lossy(&output.stderr);
            let first_line = stderr.lines().next().unwrap_or("").trim();
            Err(Error::Git(format!("git {name} failed: {first_line}")))
        }
    }

    /// Runs git and returns its output as trimmed UTF-8 text.
    pub fn run_text<I, S>(&self, cwd: &Path, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let bytes = self.run(cwd, args)?;
        String::from_utf8(bytes)
            .map(|text| text.trim_end_matches(['\n', '\r']).to_owned())
            .map_err(|_| Error::Git("git produced output that is not valid UTF-8".to_owned()))
    }

    /// Runs git and reports only whether it succeeded.
    pub fn succeeds<I, S>(&self, cwd: &Path, args: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command(cwd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// `git --version`, without the leading words (`2.50.1`).
    pub fn version(&self) -> Result<String> {
        let text = self.run_text(Path::new("."), ["--version"])?;
        Ok(text.trim_start_matches("git version ").to_owned())
    }

    /// Reads many blobs with a single `git cat-file --batch` process.
    ///
    /// Blobs larger than `max_bytes` are not read; their size is reported instead.
    /// Missing objects are absent from the result.
    pub fn read_blobs(
        &self,
        cwd: &Path,
        ids: &[String],
        max_bytes: u64,
    ) -> Result<HashMap<String, BlobData>> {
        let mut unique: Vec<&String> = ids.iter().collect();
        unique.sort();
        unique.dedup();
        if unique.is_empty() {
            return Ok(HashMap::new());
        }

        // Pass 1: sizes only, so huge blobs never enter memory.
        let sizes = self.batch(cwd, "--batch-check", &unique, |header, _| {
            Ok(header
                .split(' ')
                .nth(2)
                .and_then(|s| s.parse::<u64>().ok())
                .map(|size| (size, Vec::new())))
        })?;

        let mut result = HashMap::new();
        let mut to_fetch = Vec::new();
        for id in unique {
            match sizes.get(id) {
                None => {}
                Some((size, _)) if *size > max_bytes => {
                    result.insert(id.clone(), BlobData::TooLarge { size: *size });
                }
                Some(_) => to_fetch.push(id),
            }
        }

        // Pass 2: contents of the blobs that fit.
        let contents = self.batch(cwd, "--batch", &to_fetch, |header, reader| {
            let Some(size) = header
                .split(' ')
                .nth(2)
                .and_then(|s| s.parse::<usize>().ok())
            else {
                return Ok(None);
            };
            let mut data = vec![0_u8; size];
            reader
                .read_exact(&mut data)
                .map_err(|e| Error::Git(format!("short read from git cat-file: {e}")))?;
            let mut newline = [0_u8; 1];
            reader
                .read_exact(&mut newline)
                .map_err(|e| Error::Git(format!("short read from git cat-file: {e}")))?;
            Ok(Some((size as u64, data)))
        })?;
        for (id, (_, data)) in contents {
            result.insert(id, BlobData::Bytes(data));
        }
        Ok(result)
    }

    /// Feeds `ids` to a `git cat-file` batch mode and parses each answer with `parse`.
    fn batch(
        &self,
        cwd: &Path,
        mode: &str,
        ids: &[&String],
        mut parse: impl FnMut(&str, &mut dyn Read) -> Result<Option<(u64, Vec<u8>)>>,
    ) -> Result<HashMap<String, (u64, Vec<u8>)>> {
        let mut found = HashMap::new();
        if ids.is_empty() {
            return Ok(found);
        }
        let mut child = self
            .command(cwd)
            .args(["cat-file", mode])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Git(format!("cannot run {}: {e}", self.binary.display())))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Git("git cat-file has no stdin".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Git("git cat-file has no stdout".to_owned()))?;
        let requests: Vec<String> = ids.iter().map(|id| format!("{id}\n")).collect();

        let outcome = std::thread::scope(|scope| -> Result<()> {
            // Writing in a separate thread avoids a deadlock when answers fill the pipe.
            let writer = scope.spawn(move || {
                for request in &requests {
                    if stdin.write_all(request.as_bytes()).is_err() {
                        break;
                    }
                }
            });
            let mut reader = std::io::BufReader::new(stdout);
            for id in ids {
                let mut header = String::new();
                std::io::BufRead::read_line(&mut reader, &mut header)
                    .map_err(|e| Error::Git(format!("cannot read from git cat-file: {e}")))?;
                let header = header.trim_end_matches('\n');
                if header.ends_with(" missing") {
                    continue;
                }
                if let Some(entry) = parse(header, &mut reader)? {
                    found.insert((*id).clone(), entry);
                }
            }
            let _ = writer.join();
            Ok(())
        });
        let _ = child.wait();
        outcome?;
        Ok(found)
    }
}

/// What `read_blobs` found for one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobData {
    /// The object's bytes.
    Bytes(Vec<u8>),
    /// Larger than the limit; contents were not read.
    TooLarge {
        /// Size in bytes.
        size: u64,
    },
}

//! Durable records of the sessions Klaub started.
//!
//! tmux holds the processes; this registry holds the mapping Klaub needs to find
//! them again: which socket, which pane runs the agent and which process was
//! there when the session began. Records are small JSON files written atomically
//! with owner-only permissions.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use klaub_core::ids::SessionId;
use klaub_core::timestamp::Timestamp;
use klaub_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// The current version of the session record schema.
pub const SESSION_SCHEMA_VERSION: u32 = 1;

/// A window and the pane inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneRecord {
    /// tmux window id (`@1`).
    pub window_id: String,
    /// tmux pane id (`%2`).
    pub pane_id: String,
    /// Process id of the pane's first process when it was created.
    pub pid: u32,
}

/// Everything needed to find a session again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Schema version.
    pub schema_version: u32,
    /// The Klaub session id.
    pub session_id: SessionId,
    /// The tmux session name.
    pub tmux_session: String,
    /// The tmux socket label of this session's private server.
    pub socket: String,
    /// The tmux executable that started the server.
    pub tmux_binary: PathBuf,
    /// The worktree the session was started in.
    pub worktree: PathBuf,
    /// When the session started.
    pub created_at: Timestamp,
    /// The agent's command line, for display.
    pub agent_command: Vec<String>,
    /// The agent's window and pane.
    pub agent: PaneRecord,
    /// The reviewer's window and pane, while it exists.
    pub review: Option<PaneRecord>,
}

impl SessionRecord {
    /// The tmux session name for a Klaub session id.
    pub fn tmux_session_name(id: &SessionId) -> String {
        format!(
            "klaub-{}",
            id.as_str().trim_start_matches(SessionId::PREFIX)
        )
    }

    /// The socket label for a Klaub session id (one private server per session).
    pub fn socket_label(id: &SessionId) -> String {
        Self::tmux_session_name(id)
    }
}

/// A directory of session records.
#[derive(Debug, Clone)]
pub struct SessionRegistry {
    dir: PathBuf,
}

impl SessionRegistry {
    /// A registry stored in `dir` (created on first write).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, id: &SessionId) -> PathBuf {
        self.dir.join(format!("{}.json", id.as_str()))
    }

    /// Writes a record atomically.
    pub fn save(&self, record: &SessionRecord) -> Result<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)
            .map_err(|e| Error::io(self.dir.display(), e))?;
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|e| Error::Storage(format!("cannot encode session record: {e}")))?;
        write_private_atomic(&self.path(&record.session_id), &bytes)
    }

    /// Loads one record.
    pub fn load(&self, id: &SessionId) -> Result<SessionRecord> {
        let path = self.path(id);
        let bytes = fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::not_found("session", id),
            _ => Error::io(path.display(), e),
        })?;
        let record: SessionRecord = serde_json::from_slice(&bytes).map_err(|e| {
            Error::Storage(format!(
                "{} is not a valid session record: {e}",
                path.display()
            ))
        })?;
        if record.schema_version != SESSION_SCHEMA_VERSION {
            return Err(Error::Storage(format!(
                "{} has unsupported schema_version {}",
                path.display(),
                record.schema_version
            )));
        }
        Ok(record)
    }

    /// Lists readable records, newest first. Unreadable files are skipped with a warning.
    pub fn list(&self) -> Result<Vec<SessionRecord>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(self.dir.display(), e)),
        };
        let mut records = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".json") else {
                continue;
            };
            let Ok(id) = SessionId::parse(stem) else {
                continue;
            };
            match self.load(&id) {
                Ok(record) => records.push(record),
                Err(error) => {
                    tracing::warn!(%error, file = %name, "skipping unreadable session record");
                }
            }
        }
        records.sort_by_key(|record| std::cmp::Reverse(record.created_at));
        Ok(records)
    }

    /// Deletes a record. Missing records are fine.
    pub fn remove(&self, id: &SessionId) -> Result<()> {
        match fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::io(self.path(id).display(), e)),
        }
    }
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::Storage("record path has no parent".to_owned()))?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id()
    ));
    let _ = fs::remove_file(&temp);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|e| Error::io(temp.display(), e))?;
    file.write_all(bytes)
        .map_err(|e| Error::io(temp.display(), e))?;
    file.sync_all().map_err(|e| Error::io(temp.display(), e))?;
    drop(file);
    fs::rename(&temp, path).map_err(|e| Error::io(path.display(), e))?;
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn record(id: &str, created: i64) -> SessionRecord {
        let id = SessionId::parse(id).unwrap();
        SessionRecord {
            schema_version: SESSION_SCHEMA_VERSION,
            tmux_session: SessionRecord::tmux_session_name(&id),
            socket: SessionRecord::socket_label(&id),
            session_id: id,
            tmux_binary: PathBuf::from("/usr/bin/tmux"),
            worktree: PathBuf::from("/work/repo"),
            created_at: Timestamp::from_unix(created).unwrap(),
            agent_command: vec!["codex".into()],
            agent: PaneRecord {
                window_id: "@0".into(),
                pane_id: "%0".into(),
                pid: 10,
            },
            review: None,
        }
    }

    #[test]
    fn names_derive_from_the_session_id() {
        let id = SessionId::parse("ses_0123456789ab").unwrap();
        assert_eq!(SessionRecord::tmux_session_name(&id), "klaub-0123456789ab");
        assert_eq!(SessionRecord::socket_label(&id), "klaub-0123456789ab");
    }

    #[test]
    fn save_load_list_remove() {
        let temp = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new(temp.path().join("sessions"));
        assert_eq!(registry.list().unwrap(), Vec::new());

        registry.save(&record("ses_a", 100)).unwrap();
        registry.save(&record("ses_b", 200)).unwrap();
        let listed = registry.list().unwrap();
        assert_eq!(
            listed
                .iter()
                .map(|r| r.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ses_b", "ses_a"]
        );

        let loaded = registry.load(&SessionId::parse("ses_a").unwrap()).unwrap();
        assert_eq!(loaded, record("ses_a", 100));

        registry
            .remove(&SessionId::parse("ses_a").unwrap())
            .unwrap();
        registry
            .remove(&SessionId::parse("ses_a").unwrap())
            .unwrap();
        assert_eq!(registry.list().unwrap().len(), 1);
    }

    #[test]
    fn files_and_directory_are_private() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("sessions");
        let registry = SessionRegistry::new(&dir);
        registry.save(&record("ses_a", 1)).unwrap();
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(dir.join("ses_a.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn corrupt_and_foreign_files_do_not_break_listing() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("sessions");
        let registry = SessionRegistry::new(&dir);
        registry.save(&record("ses_ok", 1)).unwrap();
        fs::write(dir.join("ses_bad.json"), b"{not json").unwrap();
        fs::write(dir.join("notes.txt"), b"hello").unwrap();
        let listed = registry.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert!(
            registry
                .load(&SessionId::parse("ses_bad").unwrap())
                .is_err()
        );
        assert!(matches!(
            registry.load(&SessionId::parse("ses_none").unwrap()),
            Err(Error::NotFound { .. })
        ));
    }
}

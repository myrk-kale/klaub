//! Starting, finding and ending Klaub sessions.
//!
//! One Klaub session is one private tmux server with two windows: `agent`, which
//! runs the harness exactly as the user typed it, and `review`, which runs the
//! reviewer. The server's socket is dedicated to the session, so ending the
//! session means stopping *that* server and nothing else.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use klaub_core::ids::SessionId;
use klaub_core::timestamp::Timestamp;
use klaub_core::{Error, Result};

use crate::client::{Spawn, TmuxClient};
use crate::config::{SessionConfig, configuration};
use crate::registry::{PaneRecord, SESSION_SCHEMA_VERSION, SessionRecord, SessionRegistry};

/// Name of the window that runs the harness.
pub const AGENT_WINDOW: &str = "agent";
/// Name of the window that runs the reviewer.
pub const REVIEW_WINDOW: &str = "review";

/// Everything needed to start a session.
#[derive(Debug, Clone)]
pub struct LaunchSpec<'a> {
    /// The new session's id.
    pub session_id: &'a SessionId,
    /// The worktree both windows start in.
    pub worktree: &'a Path,
    /// The harness command line, run without a shell.
    pub agent_command: &'a [String],
    /// The reviewer command line (usually `klaub review --session <id>`).
    pub review_command: &'a [String],
    /// This executable; the key bindings call back into it.
    pub klaub_exe: &'a Path,
    /// The tmux prefix key.
    pub prefix: &'a str,
    /// The tmux executable.
    pub tmux_binary: &'a Path,
    /// Creation time recorded in the session record.
    pub created_at: Timestamp,
}

/// Which window to bring to the front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// The harness.
    Agent,
    /// The reviewer (recreated if it was closed).
    Review,
}

/// How a session is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionHealth {
    /// Server up and the agent process alive.
    Running,
    /// The agent process exited but its pane is still shown (a failed exit).
    AgentExited,
    /// The agent pane is gone.
    AgentGone,
    /// No server is listening: the session is over and only its record remains.
    ServerDown,
}

/// The client for a recorded session's private server.
pub fn client_for(record: &SessionRecord) -> TmuxClient {
    TmuxClient::new(&record.tmux_binary, &record.socket)
}

/// Starts a session and records it.
///
/// On failure the private server is stopped and no record is left behind.
pub fn launch(registry: &SessionRegistry, spec: &LaunchSpec<'_>) -> Result<SessionRecord> {
    let Some(program) = spec.agent_command.first() else {
        return Err(Error::invalid(
            "command",
            "nothing to run; usage: klaub wrap <command> [args...]",
        ));
    };
    if let Some(argument) = spec.agent_command.iter().find(|a| a.ends_with(';')) {
        return Err(Error::invalid(
            "command",
            format!(
                "argument {argument:?} ends with ';', which tmux would read as a command separator"
            ),
        ));
    }
    resolve_program(program)?;

    let client = TmuxClient::new(
        spec.tmux_binary,
        SessionRecord::socket_label(spec.session_id),
    );
    if client.server_running() {
        return Err(Error::Conflict(format!(
            "a tmux server already listens on socket {}",
            client.socket()
        )));
    }
    let result = launch_on(&client, registry, spec);
    if result.is_err() {
        let _ = client.kill_server();
        let _ = registry.remove(spec.session_id);
    }
    result
}

fn launch_on(
    client: &TmuxClient,
    registry: &SessionRegistry,
    spec: &LaunchSpec<'_>,
) -> Result<SessionRecord> {
    let name = SessionRecord::tmux_session_name(spec.session_id);
    let env = session_env(spec.session_id, spec.worktree);

    let agent = client.new_session(
        &name,
        &Spawn {
            window_name: AGENT_WINDOW,
            cwd: spec.worktree,
            env: &env,
            command: spec.agent_command,
        },
    )?;
    client
        .batch(&configuration(&SessionConfig {
            session_id: spec.session_id,
            klaub_exe: spec.klaub_exe,
            prefix: spec.prefix,
        })?)
        .map_err(|e| Error::Tmux(format!("{e} (did the agent exit immediately?)")))?;
    // The reviewer reads this record as soon as it starts, so it must exist before the
    // review window does. Otherwise the first `klaub review --session` races the registry.
    let mut record = SessionRecord {
        schema_version: SESSION_SCHEMA_VERSION,
        session_id: spec.session_id.clone(),
        tmux_session: name.clone(),
        socket: client.socket().to_owned(),
        tmux_binary: spec.tmux_binary.to_path_buf(),
        worktree: spec.worktree.to_path_buf(),
        created_at: spec.created_at,
        agent_command: spec.agent_command.to_vec(),
        agent: PaneRecord {
            window_id: agent.window_id,
            pane_id: agent.pane_id,
            pid: agent.pid,
        },
        review: None,
    };
    registry.save(&record)?;

    let review = client.new_window(
        &name,
        &Spawn {
            window_name: REVIEW_WINDOW,
            cwd: spec.worktree,
            env: &env,
            command: spec.review_command,
        },
        false,
    )?;
    record.review = Some(PaneRecord {
        window_id: review.window_id,
        pane_id: review.pane_id,
        pid: review.pid,
    });
    registry.save(&record)?;
    Ok(record)
}

fn session_env(id: &SessionId, worktree: &Path) -> Vec<(String, String)> {
    vec![
        ("KLAUB_SESSION".to_owned(), id.as_str().to_owned()),
        (
            "KLAUB_WORKTREE".to_owned(),
            worktree.to_string_lossy().into_owned(),
        ),
    ]
}

/// Brings a window to the front. The review window is recreated when it was closed;
/// the agent pane is never touched.
pub fn focus(
    registry: &SessionRegistry,
    record: &mut SessionRecord,
    target: FocusTarget,
    review_command: &[String],
) -> Result<()> {
    let client = client_for(record);
    let panes = client.list_panes()?;
    match target {
        FocusTarget::Agent => {
            let pane = panes
                .iter()
                .find(|p| p.pane_id == record.agent.pane_id)
                .ok_or_else(|| Error::not_found("agent pane", &record.agent.pane_id))?;
            client.select_window(&pane.window_id)?;
            client.select_pane(&pane.pane_id)
        }
        FocusTarget::Review => {
            if let Some(review) = &record.review
                && let Some(pane) = panes
                    .iter()
                    .find(|p| p.pane_id == review.pane_id && !p.dead)
            {
                return client.select_window(&pane.window_id);
            }
            let env = session_env(&record.session_id, &record.worktree);
            let pane = client.new_window(
                &record.tmux_session,
                &Spawn {
                    window_name: REVIEW_WINDOW,
                    cwd: &record.worktree,
                    env: &env,
                    command: review_command,
                },
                true,
            )?;
            record.review = Some(PaneRecord {
                window_id: pane.window_id,
                pane_id: pane.pane_id,
                pid: pane.pid,
            });
            registry.save(record)
        }
    }
}

/// Reports how the session is doing.
pub fn health(record: &SessionRecord) -> Result<SessionHealth> {
    let client = client_for(record);
    if !client.server_running() {
        return Ok(SessionHealth::ServerDown);
    }
    Ok(match client.pane(&record.agent.pane_id)? {
        None => SessionHealth::AgentGone,
        Some(pane) if pane.dead => SessionHealth::AgentExited,
        Some(_) => SessionHealth::Running,
    })
}

/// Ends the session if its agent pane disappeared. Returns whether it ended.
///
/// A dead pane that tmux keeps for inspection (`remain-on-exit failed`) is left alone
/// so the user can read the error.
pub fn reap(registry: &SessionRegistry, record: &SessionRecord) -> Result<bool> {
    match health(record)? {
        SessionHealth::AgentGone | SessionHealth::ServerDown => {
            kill(registry, record)?;
            Ok(true)
        }
        SessionHealth::Running | SessionHealth::AgentExited => Ok(false),
    }
}

/// Stops the session's private server and forgets the record.
///
/// This ends the agent as well; callers confirm with the user first.
pub fn kill(registry: &SessionRegistry, record: &SessionRecord) -> Result<()> {
    client_for(record).kill_server()?;
    registry.remove(&record.session_id)
}

/// Finds the executable a bare command name or a path refers to.
pub fn resolve_program(program: &str) -> Result<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return if is_executable_file(&path) {
            Ok(path)
        } else {
            Err(Error::invalid(
                "command",
                format!("{program} is not an executable file"),
            ))
        };
    }
    let search = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&search)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable_file(candidate))
        .ok_or_else(|| Error::invalid("command", format!("`{program}` was not found in PATH")))
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_commands_from_path_and_explicit_paths() {
        assert!(resolve_program("sh").is_ok());
        assert!(resolve_program("/bin/sh").is_ok());
        assert!(resolve_program("definitely-not-a-real-command-xyz").is_err());
        assert!(resolve_program("/definitely/not/here").is_err());
    }

    #[test]
    fn launch_rejects_commands_before_touching_tmux() {
        let temp = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new(temp.path());
        let id = SessionId::parse("ses_unit").unwrap();
        let tmux = Path::new("/nonexistent/tmux");
        let spec = |command: &'static [String]| LaunchSpec {
            session_id: &id,
            worktree: temp.path(),
            agent_command: command,
            review_command: &[],
            klaub_exe: Path::new("/bin/klaub"),
            prefix: "C-b",
            tmux_binary: tmux,
            created_at: Timestamp::from_unix(0).unwrap(),
        };
        let empty: &'static [String] = &[];
        assert!(
            launch(&registry, &spec(empty))
                .unwrap_err()
                .to_string()
                .contains("nothing to run")
        );
        let separator: &'static [String] = Box::leak(
            vec!["sh".to_owned(), "-c".to_owned(), "true;".to_owned()].into_boxed_slice(),
        );
        assert!(
            launch(&registry, &spec(separator))
                .unwrap_err()
                .to_string()
                .contains("separator")
        );
        let missing: &'static [String] =
            Box::leak(vec!["no-such-binary-xyz".to_owned()].into_boxed_slice());
        assert!(
            launch(&registry, &spec(missing))
                .unwrap_err()
                .to_string()
                .contains("not found")
        );
    }
}

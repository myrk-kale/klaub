//! A thin, safe wrapper around the `tmux` executable.
//!
//! Every call spawns tmux with an argument array (never a shell), talks to a
//! dedicated socket label and an empty configuration, and removes `TMUX` and
//! `TMUX_PANE` from the environment so it cannot be redirected to another server.

use std::ffi::OsStr;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::domain::{Error, Result};

/// Format used to describe a pane. The only free-text field (`session_name`) is Klaub-controlled.
const PANE_FORMAT: &str =
    "#{pane_id}|#{pane_pid}|#{pane_dead}|#{window_id}|#{session_name}|#{pane_current_command}";

/// What tmux reports about one pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneState {
    /// Pane id (`%3`). Unlike an index, it is never reused during the server's lifetime.
    pub pane_id: String,
    /// Process id of the pane's first process.
    pub pid: u32,
    /// The process has exited but the pane is kept (`remain-on-exit`).
    pub dead: bool,
    /// Window id (`@2`).
    pub window_id: String,
    /// The tmux session containing the pane.
    pub session_name: String,
    /// The foreground command name.
    pub command: String,
}

impl PaneState {
    fn parse(line: &str) -> Result<Self> {
        let mut fields = line.splitn(6, '|');
        let mut next = |what: &str| {
            fields.next().ok_or_else(|| {
                Error::Tmux(format!(
                    "unexpected pane description (missing {what}): {line:?}"
                ))
            })
        };
        let pane_id = next("pane id")?.to_owned();
        // A pane whose process already exited can report an empty pid (seen on macOS).
        let pid = match next("pid")? {
            "" => 0,
            digits => digits
                .parse()
                .map_err(|_| Error::Tmux(format!("unexpected pane pid in {line:?}")))?,
        };
        let dead = next("dead flag")? == "1";
        let window_id = next("window id")?.to_owned();
        let session_name = next("session")?.to_owned();
        let command = next("command")?.to_owned();
        Ok(Self {
            pane_id,
            pid,
            dead,
            window_id,
            session_name,
            command,
        })
    }
}

/// How to start a session or window.
#[derive(Debug, Clone)]
pub struct Spawn<'a> {
    /// Window name.
    pub window_name: &'a str,
    /// Working directory.
    pub cwd: &'a Path,
    /// Extra environment variables for the process.
    pub env: &'a [(String, String)],
    /// The program and its arguments, run without a shell.
    pub command: &'a [String],
}

/// A handle to one private tmux server.
#[derive(Debug, Clone)]
pub struct TmuxClient {
    binary: PathBuf,
    socket: String,
}

impl TmuxClient {
    /// Creates a client for the server behind socket label `socket`.
    pub fn new(binary: impl Into<PathBuf>, socket: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
            socket: socket.into(),
        }
    }

    /// The tmux executable in use.
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// The socket label (`klaub-1a2b3c4d5e6f`).
    pub fn socket(&self) -> &str {
        &self.socket
    }

    fn base(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .arg("-L")
            .arg(&self.socket)
            .arg("-f")
            .arg("/dev/null");
        command.env_remove("TMUX").env_remove("TMUX_PANE");
        command
    }

    fn execute(&self, args: &[&OsStr], stdin: Option<&[u8]>) -> Result<(bool, String, String)> {
        let mut command = self.base();
        command
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        let mut child = command
            .spawn()
            .map_err(|e| Error::Tmux(format!("cannot run {}: {e}", self.binary.display())))?;
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(input)
                .map_err(|e| Error::Tmux(format!("cannot write to tmux: {e}")))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| Error::Tmux(format!("tmux did not finish: {e}")))?;
        Ok((
            output.status.success(),
            String::from_utf8_lossy(&output.stdout)
                .trim_end_matches('\n')
                .to_owned(),
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }

    /// Runs a tmux command and returns its standard output without the trailing newline.
    pub fn run<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let owned: Vec<S> = args.into_iter().collect();
        let refs: Vec<&OsStr> = owned.iter().map(AsRef::as_ref).collect();
        let (ok, stdout, stderr) = self.execute(&refs, None)?;
        if ok {
            Ok(stdout)
        } else {
            let name = refs
                .first()
                .map(|a| a.to_string_lossy().into_owned())
                .unwrap_or_default();
            Err(Error::Tmux(format!("tmux {name} failed: {stderr}")))
        }
    }

    /// Runs a tmux command feeding `input` to its standard input.
    pub fn run_with_stdin<I, S>(&self, args: I, input: &[u8]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let owned: Vec<S> = args.into_iter().collect();
        let refs: Vec<&OsStr> = owned.iter().map(AsRef::as_ref).collect();
        let (ok, stdout, stderr) = self.execute(&refs, Some(input))?;
        if ok {
            Ok(stdout)
        } else {
            let name = refs
                .first()
                .map(|a| a.to_string_lossy().into_owned())
                .unwrap_or_default();
            Err(Error::Tmux(format!("tmux {name} failed: {stderr}")))
        }
    }

    /// Whether a server is listening on this client's socket.
    pub fn server_running(&self) -> bool {
        self.execute(&[OsStr::new("list-sessions")], None)
            .is_ok_and(|(ok, ..)| ok)
    }

    /// Whether the session exists.
    pub fn has_session(&self, name: &str) -> bool {
        self.execute(
            &[
                OsStr::new("has-session"),
                OsStr::new("-t"),
                OsStr::new(&format!("={name}")),
            ],
            None,
        )
        .is_ok_and(|(ok, ..)| ok)
    }

    /// Creates a detached session whose first window runs `spawn.command`.
    pub fn new_session(&self, name: &str, spawn: &Spawn<'_>) -> Result<PaneState> {
        let mut args: Vec<String> = [
            "new-session",
            "-d",
            "-s",
            name,
            "-n",
            spawn.window_name,
            "-c",
        ]
        .map(str::to_owned)
        .to_vec();
        args.push(spawn.cwd.to_string_lossy().into_owned());
        push_env(&mut args, spawn.env);
        args.extend(["-P", "-F", PANE_FORMAT, "--"].map(str::to_owned));
        args.extend(spawn.command.iter().cloned());
        PaneState::parse(&self.run(&args)?)
    }

    /// Adds a window to `session`. With `select` false the current window does not change.
    pub fn new_window(&self, session: &str, spawn: &Spawn<'_>, select: bool) -> Result<PaneState> {
        let mut args: Vec<String> = vec!["new-window".into()];
        if !select {
            args.push("-d".into());
        }
        args.extend([
            "-t".to_owned(),
            format!("={session}:"),
            "-n".to_owned(),
            spawn.window_name.to_owned(),
            "-c".to_owned(),
        ]);
        args.push(spawn.cwd.to_string_lossy().into_owned());
        push_env(&mut args, spawn.env);
        args.extend(["-P", "-F", PANE_FORMAT, "--"].map(str::to_owned));
        args.extend(spawn.command.iter().cloned());
        PaneState::parse(&self.run(&args)?)
    }

    /// Runs several tmux commands in one invocation. Each inner vector is one command.
    pub fn batch(&self, commands: &[Vec<String>]) -> Result<()> {
        let mut args: Vec<&str> = Vec::new();
        for (index, command) in commands.iter().enumerate() {
            if index > 0 {
                args.push(";");
            }
            for argument in command {
                if argument.ends_with(';') {
                    return Err(Error::Tmux(format!(
                        "argument {argument:?} would be read as a command separator"
                    )));
                }
                args.push(argument);
            }
        }
        self.run(args).map(|_| ())
    }

    /// Lists every pane on the server.
    pub fn list_panes(&self) -> Result<Vec<PaneState>> {
        let (ok, stdout, stderr) = self.execute(
            &[
                OsStr::new("list-panes"),
                OsStr::new("-a"),
                OsStr::new("-F"),
                OsStr::new(PANE_FORMAT),
            ],
            None,
        )?;
        if !ok {
            if stderr.contains("no server running")
                || stderr.contains("error connecting")
                || stderr.contains("No such file")
            {
                return Ok(Vec::new());
            }
            return Err(Error::Tmux(format!("tmux list-panes failed: {stderr}")));
        }
        stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(PaneState::parse)
            .collect()
    }

    /// Looks up one pane by id.
    pub fn pane(&self, pane_id: &str) -> Result<Option<PaneState>> {
        Ok(self
            .list_panes()?
            .into_iter()
            .find(|pane| pane.pane_id == pane_id))
    }

    /// Makes `window_id` the current window of its session.
    pub fn select_window(&self, window_id: &str) -> Result<()> {
        self.run(["select-window", "-t", window_id]).map(|_| ())
    }

    /// Makes `pane_id` the active pane of its window.
    pub fn select_pane(&self, pane_id: &str) -> Result<()> {
        self.run(["select-pane", "-t", pane_id]).map(|_| ())
    }

    /// The visible text of a pane.
    pub fn capture_pane(&self, pane_id: &str) -> Result<String> {
        self.run(["capture-pane", "-p", "-t", pane_id])
    }

    /// Stops the server behind this client's own socket. Succeeds when none is running.
    ///
    /// This only ever targets the socket Klaub created for a session; it can never
    /// reach the user's own tmux server.
    pub fn kill_server(&self) -> Result<()> {
        let (ok, _, stderr) = self.execute(&[OsStr::new("kill-server")], None)?;
        if ok
            || stderr.contains("no server running")
            || stderr.contains("error connecting")
            || stderr.contains("No such file")
        {
            Ok(())
        } else {
            Err(Error::Tmux(format!("tmux kill-server failed: {stderr}")))
        }
    }

    /// Replaces the current process with `tmux attach-session`. Returns only on failure.
    pub fn attach_exec(&self, session: &str) -> Error {
        let mut command = self.base();
        command
            .arg("attach-session")
            .arg("-t")
            .arg(format!("={session}"));
        let error = command.exec();
        Error::Tmux(format!("cannot exec {}: {error}", self.binary.display()))
    }
}

fn push_env(args: &mut Vec<String>, env: &[(String, String)]) {
    for (key, value) in env {
        args.push("-e".to_owned());
        args.push(format!("{key}={value}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_pane_line() {
        let pane = PaneState::parse("%3|4242|0|@2|klaub-abc|codex").unwrap();
        assert_eq!(pane.pane_id, "%3");
        assert_eq!(pane.pid, 4242);
        assert!(!pane.dead);
        assert_eq!(pane.window_id, "@2");
        assert_eq!(pane.session_name, "klaub-abc");
        assert_eq!(pane.command, "codex");
    }

    #[test]
    fn the_command_may_contain_the_separator() {
        let pane = PaneState::parse("%1|7|1|@0|s|a|b").unwrap();
        assert!(pane.dead);
        assert_eq!(pane.command, "a|b");
    }

    #[test]
    fn a_dead_pane_may_have_no_pid() {
        let pane = PaneState::parse("%1||1|@1|klaub-abc|klaub").unwrap();
        assert!(pane.dead);
        assert_eq!(pane.pid, 0);
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(PaneState::parse("garbage").is_err());
        assert!(PaneState::parse("%1|notapid|0|@0|s|c").is_err());
    }

    #[test]
    fn separators_in_arguments_are_refused() {
        let client = TmuxClient::new("/nonexistent/tmux", "x");
        let error = client
            .batch(&[vec!["set-option".into(), "-g".into(), "foo;".into()]])
            .unwrap_err();
        assert!(error.to_string().contains("separator"));
    }
}

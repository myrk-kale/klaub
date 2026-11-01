//! Process-backed adapters: the test runner and the clipboard.
//!
//! Both spawn a program directly, never through a shell, so a test id or a copied comment cannot
//! be interpreted as shell syntax. Output is capped and sanitised before it comes back, because it
//! is later drawn on screen.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::domain::error::{Error, Result};
use crate::domain::ports::{CancelToken, Clipboard, TestRunOutcome, TestRunRequest, TestRunner};
use crate::domain::text::sanitize_terminal_text;

/// How much of a test run's output is kept. The tail is what the reader shows.
const OUTPUT_LIMIT: usize = 64 * 1024;

/// How often a running test command is checked for cancellation or timeout.
const POLL: Duration = Duration::from_millis(100);

/// How long the output readers get to drain after the command is gone. A process that escaped the
/// group (it started its own session) can keep a pipe open for as long as it likes; the run does
/// not wait for it.
const DRAIN: Duration = Duration::from_secs(2);

/// Runs the user's test command as a direct child process.
#[derive(Debug, Default)]
pub(crate) struct ProcessTestRunner;

impl TestRunner for ProcessTestRunner {
    fn run(&self, request: &TestRunRequest, cancel: &CancelToken) -> Result<TestRunOutcome> {
        let program = request
            .command
            .first()
            .ok_or_else(|| Error::invalid("test command", "the configured command is empty"))?;
        let mut command = Command::new(program);
        command
            .args(&request.command[1..])
            .args(
                request
                    .selection
                    .iter()
                    .map(crate::domain::ids::TestId::as_str),
            )
            .current_dir(&request.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Its own process group, so a timeout or a cancel can end everything the test
            // command started and not only the command itself.
            .process_group(0);

        let started = Instant::now();
        let mut child = command.spawn().map_err(|error| Error::io(program, error))?;

        // Both pipes are read while the command runs. A command that writes more than a pipe
        // holds would otherwise block on its next write while Klaub waits for it to finish.
        let output = Arc::new(Mutex::new(Tail::default()));
        let readers = [
            child
                .stdout
                .take()
                .map(|pipe| drain(pipe, Arc::clone(&output))),
            child
                .stderr
                .take()
                .map(|pipe| drain(pipe, Arc::clone(&output))),
        ];

        let outcome = wait_for(&mut child, request.timeout, cancel);
        let elapsed = started.elapsed();
        let drained = Instant::now();
        for reader in readers.into_iter().flatten() {
            while !reader.is_finished() && drained.elapsed() < DRAIN {
                thread::sleep(Duration::from_millis(10));
            }
            if reader.is_finished() {
                let _ = reader.join();
            }
        }
        let bytes = output.lock().map(|tail| tail.bytes()).unwrap_or_default();
        let output = tail(&sanitize_terminal_text(&String::from_utf8_lossy(&bytes)));

        Ok(TestRunOutcome {
            exit_code: outcome.exit_code,
            timed_out: outcome.timed_out,
            cancelled: outcome.cancelled,
            duration: elapsed,
            output_tail: output,
        })
    }
}

/// The last [`OUTPUT_LIMIT`] bytes written to either pipe, in the order they arrived.
#[derive(Debug, Default)]
struct Tail {
    buffer: Vec<u8>,
}

impl Tail {
    fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
        // Trimmed in steps, so a long run does not move the buffer on every read.
        if self.buffer.len() > OUTPUT_LIMIT * 2 {
            let excess = self.buffer.len() - OUTPUT_LIMIT;
            self.buffer.drain(..excess);
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let start = self.buffer.len().saturating_sub(OUTPUT_LIMIT);
        self.buffer[start..].to_vec()
    }
}

/// Reads `pipe` to the end on a thread of its own, keeping only the tail.
fn drain(mut pipe: impl Read + Send + 'static, output: Arc<Mutex<Tail>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut chunk = [0_u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(read) => {
                    if let Ok(mut tail) = output.lock() {
                        tail.push(&chunk[..read]);
                    }
                }
            }
        }
    })
}

struct WaitOutcome {
    exit_code: Option<i32>,
    timed_out: bool,
    cancelled: bool,
}

/// Polls the child until it exits, the time limit passes or the user cancels. Either of the last
/// two ends the whole process group; nothing is left running in the background.
fn wait_for(child: &mut Child, timeout: Duration, cancel: &CancelToken) -> WaitOutcome {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().ok().flatten() {
            return WaitOutcome {
                exit_code: status.code(),
                timed_out: false,
                cancelled: false,
            };
        }
        let timed_out = started.elapsed() >= timeout;
        let cancelled = cancel.is_cancelled();
        if timed_out || cancelled {
            kill_group(child);
            let _ = child.wait();
            return WaitOutcome {
                exit_code: None,
                timed_out,
                cancelled,
            };
        }
        thread::sleep(POLL);
    }
}

/// Ends the child and every process in its group. The child leads the group (it was started with
/// `process_group(0)`), so a negative pid addresses all of them; `kill` is run as a program, with
/// no shell, because the standard library only kills the child itself.
fn kill_group(child: &mut Child) {
    let _ = Command::new("kill")
        .args(["-KILL", "--"])
        .arg(format!("-{}", child.id()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

/// The end of `text`, cut on a character boundary so a multibyte character is never split.
fn tail(text: &str) -> String {
    if text.len() <= OUTPUT_LIMIT {
        return text.to_owned();
    }
    let mut cut = text.len() - OUTPUT_LIMIT;
    while !text.is_char_boundary(cut) {
        cut += 1;
    }
    text[cut..].to_owned()
}

/// Copies text with `pbcopy` on macOS and `wl-copy` or `xclip` elsewhere. A missing program is an
/// error the reader reports, not a panic.
#[derive(Debug, Default)]
pub(crate) struct ClipboardCopy;

impl Clipboard for ClipboardCopy {
    fn copy(&self, text: &str) -> Result<()> {
        let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
            &[("pbcopy", &[])]
        } else {
            &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
        };
        let mut last = None;
        for (program, args) in candidates {
            match pipe_to(program, args, text) {
                Ok(()) => return Ok(()),
                Err(error) => last = Some(error),
            }
        }
        Err(last
            .unwrap_or_else(|| Error::invalid("clipboard", "no clipboard program is available")))
    }
}

fn pipe_to(program: &str, args: &[&str], text: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| Error::io(program, error))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| Error::io(program, error))?;
    }
    let status = child.wait().map_err(|error| Error::io(program, error))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::invalid(
            "clipboard",
            format!("{program} exited with {status}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_keeps_the_end_and_respects_character_boundaries() {
        assert_eq!(tail("short"), "short");
        let long = "a".repeat(OUTPUT_LIMIT + 10);
        assert_eq!(tail(&long).len(), OUTPUT_LIMIT);
        let multibyte = format!("{}{}", "x".repeat(OUTPUT_LIMIT - 1), "é");
        assert!(tail(&multibyte).is_char_boundary(0));
    }

    #[test]
    fn a_command_runs_without_a_shell_and_reports_its_exit_code() {
        let request = TestRunRequest {
            command: vec!["true".to_owned()],
            selection: Vec::new(),
            cwd: std::env::temp_dir(),
            timeout: Duration::from_secs(5),
        };
        let outcome = ProcessTestRunner
            .run(&request, &CancelToken::default())
            .unwrap();
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.timed_out);
    }

    fn shell(script: &str, timeout: Duration) -> TestRunRequest {
        TestRunRequest {
            command: vec!["sh".to_owned(), "-c".to_owned(), script.to_owned()],
            selection: Vec::new(),
            cwd: std::env::temp_dir(),
            timeout,
        }
    }

    #[test]
    fn output_far_larger_than_a_pipe_does_not_stall_the_command() {
        // Three megabytes on standard output and a lot on standard error: far past any pipe
        // buffer. The command must finish on its own and the tail must be the end of the output.
        let request = shell(
            "yes line-on-stdout | head -c 3000000; yes line-on-stderr | head -c 1000000 >&2; echo THE-END",
            Duration::from_secs(20),
        );
        let started = Instant::now();
        let outcome = ProcessTestRunner
            .run(&request, &CancelToken::default())
            .unwrap();
        assert_eq!(outcome.exit_code, Some(0), "{outcome:?}");
        assert!(!outcome.timed_out);
        assert!(started.elapsed() < Duration::from_secs(15));
        assert!(outcome.output_tail.len() <= OUTPUT_LIMIT);
        assert!(outcome.output_tail.trim_end().ends_with("THE-END"));
    }

    #[test]
    fn both_streams_are_captured() {
        let request = shell("echo to-stdout; echo to-stderr >&2", Duration::from_secs(5));
        let outcome = ProcessTestRunner
            .run(&request, &CancelToken::default())
            .unwrap();
        assert!(outcome.output_tail.contains("to-stdout"));
        assert!(outcome.output_tail.contains("to-stderr"));
    }

    #[test]
    fn a_timeout_ends_the_processes_the_command_started() {
        // The shell starts a background sleep and then waits on another. Killing only the shell
        // would leave the first one holding the output pipes open.
        let marker = std::env::temp_dir().join(format!("klaub-grandchild-{}", std::process::id()));
        let script = format!("(sleep 3; touch {m}) & sleep 30", m = marker.display());
        let request = shell(&script, Duration::from_secs(1));
        let started = Instant::now();
        let outcome = ProcessTestRunner
            .run(&request, &CancelToken::default())
            .unwrap();
        assert!(outcome.timed_out);
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "the run waited for the grandchild: {:?}",
            started.elapsed()
        );
        // Had the background job survived, it would have written the marker by now.
        thread::sleep(Duration::from_secs(4));
        assert!(!marker.exists(), "the background job outlived the timeout");
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn a_slow_command_is_stopped_at_the_time_limit() {
        let request = TestRunRequest {
            command: vec!["sleep".to_owned(), "30".to_owned()],
            selection: Vec::new(),
            cwd: std::env::temp_dir(),
            timeout: Duration::from_millis(200),
        };
        let started = Instant::now();
        let outcome = ProcessTestRunner
            .run(&request, &CancelToken::default())
            .unwrap();
        assert!(outcome.timed_out, "expected the time limit to fire");
        assert!(outcome.exit_code.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the process was not killed"
        );
    }
}

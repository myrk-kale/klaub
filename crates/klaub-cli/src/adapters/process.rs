//! Process-backed adapters: the test runner and the clipboard.
//!
//! Both spawn a program directly, never through a shell, so a test id or a copied comment cannot
//! be interpreted as shell syntax. Output is capped and sanitised before it comes back, because it
//! is later drawn on screen.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use klaub_core::error::{Error, Result};
use klaub_core::ports::{CancelToken, Clipboard, TestRunOutcome, TestRunRequest, TestRunner};
use klaub_core::text::sanitize_terminal_text;

/// How much of a test run's output is kept. The tail is what the reader shows.
const OUTPUT_LIMIT: usize = 64 * 1024;

/// How often a running test command is checked for cancellation or timeout.
const POLL: Duration = Duration::from_millis(100);

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
                    .map(klaub_core::ids::TestId::as_str),
            )
            .current_dir(&request.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let started = Instant::now();
        let mut child = command.spawn().map_err(|error| Error::io(program, error))?;
        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();

        let outcome = wait_for(&mut child, request.timeout, cancel);
        let elapsed = started.elapsed();
        let mut output = String::new();
        if let Some(pipe) = stdout.as_mut() {
            let _ = pipe.read_to_string(&mut output);
        }
        if let Some(pipe) = stderr.as_mut() {
            let _ = pipe.read_to_string(&mut output);
        }
        let output = tail(&sanitize_terminal_text(&output));

        Ok(TestRunOutcome {
            exit_code: outcome.exit_code,
            timed_out: outcome.timed_out,
            cancelled: outcome.cancelled,
            duration: elapsed,
            output_tail: output,
        })
    }
}

struct WaitOutcome {
    exit_code: Option<i32>,
    timed_out: bool,
    cancelled: bool,
}

/// Polls the child until it exits, the time limit passes or the user cancels. Either of the last
/// two kills the process; it is never left running in the background.
fn wait_for(
    child: &mut std::process::Child,
    timeout: Duration,
    cancel: &CancelToken,
) -> WaitOutcome {
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
            let _ = child.kill();
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

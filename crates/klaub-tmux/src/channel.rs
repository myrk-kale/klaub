//! Safe delivery of text to the agent's input.
//!
//! The channel implements the `AgentChannel` port. It proves the destination
//! before pasting, pastes as a bracketed paste, and **never** sends `Enter`.
//! tmux cannot tell whether the harness enabled bracketed paste, so the text is
//! always reduced to a single line: even if the paste markers are ignored, no
//! line break can reach the harness as a submission.

use klaub_core::Result;
use klaub_core::ports::{AgentChannel, AgentTarget, PrepareOutcome};
use klaub_core::text::{sanitize_single_line, truncate_chars};

use crate::client::{PaneState, TmuxClient};
use crate::registry::SessionRecord;
use crate::session::client_for;

/// The longest text that will be pasted.
pub const MAX_PASTE_CHARS: usize = 4000;

/// The result of checking that the recorded agent pane is still the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneCheck {
    /// The pane exists and runs the process recorded at launch.
    Verified(PaneState),
    /// The pane cannot be trusted; the reason is safe to show.
    Rejected(String),
}

/// Reduces `text` to what may be pasted: control and escape sequences removed,
/// line breaks collapsed into spaces, length bounded. `None` when nothing is left.
pub fn prepare_paste_text(text: &str) -> Option<String> {
    let line = sanitize_single_line(text);
    if line.is_empty() {
        None
    } else {
        Some(truncate_chars(&line, MAX_PASTE_CHARS))
    }
}

/// The agent channel of one Klaub session.
#[derive(Debug, Clone)]
pub struct TmuxAgentChannel {
    client: TmuxClient,
    record: SessionRecord,
}

impl TmuxAgentChannel {
    /// A channel for the session described by `record`.
    pub fn new(record: SessionRecord) -> Self {
        Self {
            client: client_for(&record),
            record,
        }
    }

    /// Checks that the recorded pane still exists and still runs the process it ran at launch.
    pub fn verify(&self) -> Result<PaneCheck> {
        if !self.client.server_running() {
            return Ok(PaneCheck::Rejected(
                "the tmux session is no longer running".to_owned(),
            ));
        }
        let Some(pane) = self.client.pane(&self.record.agent.pane_id)? else {
            return Ok(PaneCheck::Rejected(
                "the agent pane no longer exists".to_owned(),
            ));
        };
        let reason = if pane.session_name != self.record.tmux_session {
            Some("the pane belongs to a different session")
        } else if pane.window_id != self.record.agent.window_id {
            Some("the agent pane was moved to another window")
        } else if pane.dead {
            Some("the agent process has exited")
        } else if pane.pid != self.record.agent.pid {
            Some("the process in the agent pane was replaced")
        } else {
            None
        };
        Ok(reason.map_or(PaneCheck::Verified(pane), |r| {
            PaneCheck::Rejected(r.to_owned())
        }))
    }
}

impl AgentChannel for TmuxAgentChannel {
    fn target(&self) -> Result<Option<AgentTarget>> {
        Ok(match self.verify()? {
            PaneCheck::Verified(pane) => Some(AgentTarget {
                description: format!(
                    "tmux session {} · window {} · pane {}",
                    pane.session_name, "agent", pane.pane_id
                ),
                command: pane.command,
                pane_id: pane.pane_id,
            }),
            PaneCheck::Rejected(_) => None,
        })
    }

    fn prepare_input(&self, text: &str) -> Result<PrepareOutcome> {
        let Some(line) = prepare_paste_text(text) else {
            return Ok(PrepareOutcome::Unavailable(
                "there is nothing to paste".to_owned(),
            ));
        };
        let pane = match self.verify()? {
            PaneCheck::Verified(pane) => pane,
            PaneCheck::Rejected(reason) => return Ok(PrepareOutcome::Unavailable(reason)),
        };

        let buffer = "klaub-feedback";
        self.client
            .run_with_stdin(["load-buffer", "-b", buffer, "-"], line.as_bytes())?;
        // -p: bracketed paste when the application asked for it; -d: drop the buffer afterwards.
        // Deliberately no trailing newline and no `send-keys Enter`: the human submits.
        if let Err(error) = self.client.run([
            "paste-buffer",
            "-p",
            "-d",
            "-b",
            buffer,
            "-t",
            &pane.pane_id,
        ]) {
            let _ = self.client.run(["delete-buffer", "-b", buffer]);
            return Err(error);
        }
        self.client.select_window(&pane.window_id)?;
        self.client.select_pane(&pane.pane_id)?;
        Ok(PrepareOutcome::Prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_text_is_one_sanitised_line() {
        assert_eq!(
            prepare_paste_text("Review\nthe file\x1b[31m now\r\n").as_deref(),
            Some("Review the file now")
        );
        assert_eq!(
            prepare_paste_text("a\x1b]52;c;ZXZpbA==\x07b").as_deref(),
            Some("ab")
        );
        assert_eq!(
            prepare_paste_text("text\x1b[201~ rm -rf /").as_deref(),
            Some("text rm -rf /")
        );
    }

    #[test]
    fn nothing_to_paste_is_none() {
        assert_eq!(prepare_paste_text(""), None);
        assert_eq!(prepare_paste_text(" \n\t\x1b[0m"), None);
    }

    #[test]
    fn length_is_bounded() {
        let long = "x".repeat(MAX_PASTE_CHARS + 50);
        let prepared = prepare_paste_text(&long).unwrap();
        assert_eq!(prepared.chars().count(), MAX_PASTE_CHARS);
        assert!(prepared.ends_with('…'));
    }
}

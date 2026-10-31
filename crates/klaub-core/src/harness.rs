//! The seam for harness-specific adapters.
//!
//! Klaub works with any interactive program by the same contract: one sanitised line, pasted as a
//! bracketed paste, never followed by `Enter`. That contract needs nothing from the program, and it
//! is the only thing the reader relies on.
//!
//! A [`HarnessAdapter`] is where something specific to one program could plug in later: telling
//! from the screen whether the program is waiting for input, and whether text was submitted. The
//! specification (§14.4, §23) keeps real adapters out of these versions until each harness has its
//! own readiness contract and its own acceptance, so this crate ships only [`GenericHarness`], which
//! knows nothing and says so. The reader asks the registry for the adapter of the program it is
//! about to paste into, shows its name in the confirmation, and treats "unknown" as "unknown", never
//! as "ready".

use std::sync::Arc;

/// What an adapter can tell about the program's input from its screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// The adapter cannot tell. Nothing is assumed.
    Unknown,
    /// The program looks to be waiting for input.
    Ready,
    /// The program looks to be busy; a paste may land in the middle of its output. The text is
    /// safe to show to the user.
    Busy(String),
}

/// Whether text that was prepared in the input was submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submission {
    /// Nothing says it was. The reader keeps the feedback as prepared until the user says otherwise.
    Unconfirmed,
    /// The screen shows the program took the text as a submitted message.
    Confirmed,
}

/// Knowledge about one interactive program. Implementations must be pure functions of what they
/// are given: they do not touch the pane, the file system or the network.
pub trait HarnessAdapter: Send + Sync {
    /// A stable identifier, such as `generic`.
    fn id(&self) -> &'static str;

    /// What to call it in a confirmation.
    fn name(&self) -> &'static str;

    /// Whether this adapter is for the program that runs as `command` (the foreground process name
    /// of the pane, already reduced to its file name).
    fn recognizes(&self, command: &str) -> bool;

    /// Whether the program looks ready for input, given the tail of its screen.
    fn readiness(&self, screen_tail: &str) -> Readiness {
        let _ = screen_tail;
        Readiness::Unknown
    }

    /// Whether text was submitted, given the screen before and after the paste.
    fn submission(&self, before: &str, after: &str) -> Submission {
        let _ = (before, after);
        Submission::Unconfirmed
    }
}

/// The adapter for any program. It recognises nothing on purpose, so it is only ever the fallback,
/// and it answers "unknown" to every question.
#[derive(Debug, Clone, Copy, Default)]
pub struct GenericHarness;

impl HarnessAdapter for GenericHarness {
    fn id(&self) -> &'static str {
        "generic"
    }

    fn name(&self) -> &'static str {
        "generic (Klaub does not read this program's state)"
    }

    fn recognizes(&self, _command: &str) -> bool {
        false
    }
}

/// The adapters Klaub knows, in the order they are tried.
#[derive(Clone)]
pub struct HarnessRegistry {
    adapters: Vec<Arc<dyn HarnessAdapter>>,
    generic: Arc<dyn HarnessAdapter>,
}

impl Default for HarnessRegistry {
    fn default() -> Self {
        Self {
            adapters: Vec::new(),
            generic: Arc::new(GenericHarness),
        }
    }
}

impl std::fmt::Debug for HarnessRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.adapters.iter().map(|a| a.id()))
            .finish()
    }
}

impl HarnessRegistry {
    /// A registry that tries `adapter` before the ones already in it.
    #[must_use]
    pub fn with(mut self, adapter: Arc<dyn HarnessAdapter>) -> Self {
        self.adapters.insert(0, adapter);
        self
    }

    /// The adapter for the program running as `command`, or the generic one.
    ///
    /// `command` may be a path or a login-shell name (`/usr/bin/codex`, `-zsh`); only the file name
    /// counts.
    #[must_use]
    pub fn select(&self, command: &str) -> Arc<dyn HarnessAdapter> {
        let name = command
            .rsplit('/')
            .next()
            .unwrap_or(command)
            .trim_start_matches('-');
        self.adapters
            .iter()
            .find(|adapter| adapter.recognizes(name))
            .cloned()
            .unwrap_or_else(|| Arc::clone(&self.generic))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Shouty;

    impl HarnessAdapter for Shouty {
        fn id(&self) -> &'static str {
            "shouty"
        }
        fn name(&self) -> &'static str {
            "Shouty"
        }
        fn recognizes(&self, command: &str) -> bool {
            command == "shouty"
        }
        fn readiness(&self, screen_tail: &str) -> Readiness {
            if screen_tail.contains("thinking") {
                Readiness::Busy("it is thinking".to_owned())
            } else {
                Readiness::Ready
            }
        }
    }

    #[test]
    fn an_unknown_program_gets_the_generic_adapter() {
        let registry = HarnessRegistry::default();
        let adapter = registry.select("codex");
        assert_eq!(adapter.id(), "generic");
        assert_eq!(adapter.readiness("anything"), Readiness::Unknown);
        assert_eq!(adapter.submission("a", "b"), Submission::Unconfirmed);
    }

    #[test]
    fn a_registered_adapter_is_chosen_by_the_name_of_the_program() {
        let registry = HarnessRegistry::default().with(Arc::new(Shouty));
        for command in ["shouty", "/opt/bin/shouty", "-shouty"] {
            assert_eq!(registry.select(command).id(), "shouty", "{command}");
        }
        assert_eq!(registry.select("claude").id(), "generic");
    }

    #[test]
    fn the_generic_adapter_never_claims_anything() {
        let generic = GenericHarness;
        assert!(!generic.recognizes("generic"));
        assert!(!generic.recognizes(""));
    }
}

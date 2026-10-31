//! Which character triggers which action.
//!
//! The reader ships one default map and `[keys]` in the configuration can move any action in
//! [`BINDABLE`] to another single character. Everything else is deliberately fixed: the arrow and
//! function keys, `Esc`, `Enter`, `Tab`, the palette (`:`), quitting (`q`), and the keys that only
//! mean something inside one mode (`j`/`k` movement, the follow-up decisions, the editor). A key a
//! mode handles for itself wins over this map while that mode is open, so a binding that lands on
//! one of those keys is refused when the map is built instead of being silently shadowed.
//!
//! The map is validated as a whole. A typo in an action id, a key that is not one printable
//! character, a reserved key and two actions on the same key each name the entry that caused them,
//! because a half-applied key map is worse than none.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::actions::{Action, Entry};
use crate::app::Mode;

/// An action that `[keys]` may move, with the characters it answers to by default.
struct Bindable {
    /// The id used in the configuration.
    id: &'static str,
    /// What the key does.
    action: Action,
    /// The default characters. The first one is the one shown in the help.
    defaults: &'static [char],
}

/// Every action `[keys]` can rebind.
const BINDABLE: &[Bindable] = &[
    Bindable {
        id: "queue",
        action: Action::Mode(Mode::Queue),
        defaults: &['1'],
    },
    Bindable {
        id: "structure",
        action: Action::Mode(Mode::Structure),
        defaults: &['2'],
    },
    Bindable {
        id: "code",
        action: Action::Mode(Mode::Code),
        defaults: &['3'],
    },
    Bindable {
        id: "impact",
        action: Action::Mode(Mode::Impact),
        defaults: &['4', 'g'],
    },
    Bindable {
        id: "comments",
        action: Action::Mode(Mode::Comments),
        defaults: &['5'],
    },
    Bindable {
        id: "context",
        action: Action::Mode(Mode::Context),
        defaults: &['6', 'b'],
    },
    Bindable {
        id: "tests",
        action: Action::Mode(Mode::Tests),
        defaults: &['7', 't'],
    },
    Bindable {
        id: "followup",
        action: Action::Mode(Mode::Followup),
        defaults: &['8'],
    },
    Bindable {
        id: "coverage",
        action: Action::Mode(Mode::Coverage),
        defaults: &['9'],
    },
    Bindable {
        id: "flow",
        action: Action::Mode(Mode::Flow),
        defaults: &['0', 'f'],
    },
    Bindable {
        id: "next_unreviewed",
        action: Action::NextUnreviewed,
        defaults: &['n'],
    },
    Bindable {
        id: "previous_unreviewed",
        action: Action::PrevUnreviewed,
        defaults: &['P'],
    },
    Bindable {
        id: "next_critical",
        action: Action::NextCritical,
        defaults: &['N'],
    },
    Bindable {
        id: "toggle_reviewed",
        action: Action::ToggleReviewed,
        defaults: &['r'],
    },
    Bindable {
        id: "toggle_bookmark",
        action: Action::ToggleBookmark,
        defaults: &['m'],
    },
    Bindable {
        id: "next_bookmark",
        action: Action::NextBookmark,
        defaults: &[','],
    },
    Bindable {
        id: "comment",
        action: Action::Comment,
        defaults: &['c'],
    },
    Bindable {
        id: "explain",
        action: Action::Explain,
        defaults: &['w'],
    },
    Bindable {
        id: "toggle_version",
        action: Action::ToggleVersion,
        defaults: &['v'],
    },
    Bindable {
        id: "next_change",
        action: Action::NextChange,
        defaults: &[']'],
    },
    Bindable {
        id: "previous_change",
        action: Action::PrevChange,
        defaults: &['['],
    },
    Bindable {
        id: "toggle_code_scope",
        action: Action::ToggleCodeScope,
        defaults: &['S'],
    },
    Bindable {
        id: "search",
        action: Action::Search,
        defaults: &['/'],
    },
    Bindable {
        id: "filter_high",
        action: Action::FilterHigh,
        defaults: &['H'],
    },
    Bindable {
        id: "filter_unreviewed",
        action: Action::FilterUnreviewed,
        defaults: &['u'],
    },
    Bindable {
        id: "filter_introduced",
        action: Action::FilterIntroduced,
        defaults: &['I'],
    },
    Bindable {
        id: "filter_no_tests",
        action: Action::FilterNoTests,
        defaults: &['T'],
    },
    Bindable {
        id: "filter_new",
        action: Action::FilterNew,
        defaults: &['W'],
    },
    Bindable {
        id: "cycle_scope",
        action: Action::CycleScope,
        defaults: &['s'],
    },
    Bindable {
        id: "cycle_module",
        action: Action::CycleModule,
        defaults: &['M'],
    },
    Bindable {
        id: "clear_filters",
        action: Action::ClearFilters,
        defaults: &['x'],
    },
    Bindable {
        id: "deeper_tree",
        action: Action::DeeperTree,
        defaults: &['+', '='],
    },
    Bindable {
        id: "shallower_tree",
        action: Action::ShallowerTree,
        defaults: &['-'],
    },
    Bindable {
        id: "run_tests",
        action: Action::RunTests,
        defaults: &['X'],
    },
    Bindable {
        id: "reload_reports",
        action: Action::ReloadReports,
        defaults: &['U'],
    },
    Bindable {
        id: "reload_flow",
        action: Action::ReloadFlow,
        defaults: &['L'],
    },
    Bindable {
        id: "refresh",
        action: Action::Refresh,
        defaults: &['R'],
    },
    Bindable {
        id: "finish",
        action: Action::Finish,
        defaults: &['F'],
    },
    Bindable {
        id: "help",
        action: Action::Help,
        defaults: &['?'],
    },
];

/// Characters no binding may take. `q` quits, `:` opens the palette, and the rest are the keys the
/// modes use for themselves (movement, the follow-up decisions, horizontal scroll, the contract
/// inspector, the flow request).
const RESERVED: &[char] = &['q', ':', ' ', 'j', 'k', 'h', 'l', 'y', 'o', 'p', 'V'];

/// Why a `[keys]` table was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapError {
    /// The configuration names an action the reader does not have.
    UnknownAction(String),
    /// The value is not exactly one printable character.
    NotOneCharacter {
        /// The action being bound.
        action: String,
        /// The value as written.
        value: String,
    },
    /// The character belongs to the reader and cannot be rebound.
    Reserved {
        /// The action being bound.
        action: String,
        /// The character as written.
        key: char,
    },
    /// Two actions ended up on the same character.
    Conflict {
        /// The character both want.
        key: char,
        /// The action that has it first.
        first: String,
        /// The action that was refused.
        second: String,
    },
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAction(id) => write!(
                f,
                "[keys] has no action called `{id}`; the known ones are: {}",
                BINDABLE.iter().map(|b| b.id).collect::<Vec<_>>().join(", ")
            ),
            Self::NotOneCharacter { action, value } => write!(
                f,
                "[keys] {action} = {value:?}: a binding is exactly one printable character"
            ),
            Self::Reserved { action, key } => write!(
                f,
                "[keys] {action} = {key:?}: that key belongs to the reader and cannot be rebound"
            ),
            Self::Conflict { key, first, second } => write!(
                f,
                "[keys] {first} and {second} are both bound to {key:?}; give one of them another key"
            ),
        }
    }
}

impl std::error::Error for KeymapError {}

/// The character-to-action table the reader consults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    /// What each character does.
    by_char: HashMap<char, Action>,
    /// The characters an action was moved to. Actions that were not moved are absent.
    moved: HashMap<&'static str, char>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut by_char = HashMap::new();
        for bindable in BINDABLE {
            for &c in bindable.defaults {
                by_char.insert(c, bindable.action);
            }
        }
        Self {
            by_char,
            moved: HashMap::new(),
        }
    }
}

impl Keymap {
    /// Builds the map from the `[keys]` table, or says why it cannot.
    ///
    /// An empty table is the default map. A moved action gives up all of its default characters
    /// and answers only to the new one, so the help never lists a key that no longer works.
    ///
    /// # Errors
    ///
    /// Returns the first problem found: an unknown action id, a value that is not one printable
    /// character, a reserved character, or two actions on one character.
    pub fn from_overrides(overrides: &BTreeMap<String, String>) -> Result<Self, KeymapError> {
        let mut moved: HashMap<&'static str, char> = HashMap::new();
        for (id, value) in overrides {
            let bindable = BINDABLE
                .iter()
                .find(|b| b.id == id)
                .ok_or_else(|| KeymapError::UnknownAction(id.clone()))?;
            let mut chars = value.chars();
            let key = match (chars.next(), chars.next()) {
                (Some(c), None) if !c.is_control() && !c.is_whitespace() => c,
                _ => {
                    return Err(KeymapError::NotOneCharacter {
                        action: id.clone(),
                        value: value.clone(),
                    });
                }
            };
            if RESERVED.contains(&key) {
                return Err(KeymapError::Reserved {
                    action: id.clone(),
                    key,
                });
            }
            moved.insert(bindable.id, key);
        }

        let mut by_char: HashMap<char, (Action, &'static str)> = HashMap::new();
        for bindable in BINDABLE {
            let keys: Vec<char> = match moved.get(bindable.id) {
                Some(&key) => vec![key],
                None => bindable.defaults.to_vec(),
            };
            for key in keys {
                if let Some((_, first)) = by_char.get(&key) {
                    return Err(KeymapError::Conflict {
                        key,
                        first: (*first).to_owned(),
                        second: bindable.id.to_owned(),
                    });
                }
                by_char.insert(key, (bindable.action, bindable.id));
            }
        }
        Ok(Self {
            by_char: by_char
                .into_iter()
                .map(|(key, (action, _))| (key, action))
                .collect(),
            moved,
        })
    }

    /// The action a character triggers.
    pub(crate) fn action(&self, c: char) -> Option<Action> {
        self.by_char.get(&c).copied()
    }

    /// The key column of the help and the palette for one entry.
    ///
    /// An action that was moved shows its new character in place of the default ones. Function-key
    /// aliases such as `F5` cannot be moved and stay.
    pub(crate) fn label(&self, entry: &Entry) -> String {
        let Some(bindable) = BINDABLE.iter().find(|b| b.action == entry.action) else {
            return entry.key.to_owned();
        };
        let Some(key) = self.moved.get(bindable.id) else {
            return entry.key.to_owned();
        };
        let mut parts = vec![key.to_string()];
        parts.extend(
            entry
                .key
                .split_whitespace()
                .filter(|token| is_function_key(token))
                .map(str::to_owned),
        );
        parts.join("  ")
    }
}

/// Whether a help token names a function key (`F1` to `F12`).
fn is_function_key(token: &str) -> bool {
    token
        .strip_prefix('F')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn the_default_map_triggers_every_bindable_action() {
        let map = Keymap::default();
        for bindable in BINDABLE {
            for &c in bindable.defaults {
                assert_eq!(map.action(c), Some(bindable.action), "default key {c}");
            }
        }
    }

    #[test]
    fn defaults_never_use_a_reserved_key() {
        for bindable in BINDABLE {
            for c in bindable.defaults {
                assert!(
                    !RESERVED.contains(c),
                    "{} defaults to the reserved key {c}",
                    bindable.id
                );
            }
        }
    }

    #[test]
    fn a_moved_action_answers_only_to_its_new_key() {
        let map = Keymap::from_overrides(&table(&[("toggle_reviewed", "e")])).unwrap();
        assert_eq!(map.action('e'), Some(Action::ToggleReviewed));
        assert_eq!(map.action('r'), None, "the old key is released");
    }

    #[test]
    fn an_action_with_two_defaults_gives_up_both_when_moved() {
        let map = Keymap::from_overrides(&table(&[("impact", "i")])).unwrap();
        assert_eq!(map.action('i'), Some(Action::Mode(Mode::Impact)));
        assert_eq!(map.action('4'), None);
        assert_eq!(map.action('g'), None);
    }

    #[test]
    fn two_actions_swapped_with_each_other_are_accepted() {
        let map =
            Keymap::from_overrides(&table(&[("toggle_reviewed", "c"), ("comment", "r")])).unwrap();
        assert_eq!(map.action('c'), Some(Action::ToggleReviewed));
        assert_eq!(map.action('r'), Some(Action::Comment));
    }

    #[test]
    fn an_unknown_action_is_named_with_the_known_ones() {
        let error = Keymap::from_overrides(&table(&[("no_such_action", "e")])).unwrap_err();
        assert_eq!(
            error,
            KeymapError::UnknownAction("no_such_action".to_owned())
        );
        assert!(error.to_string().contains("toggle_reviewed"));
    }

    #[test]
    fn a_value_must_be_exactly_one_printable_character() {
        for bad in ["", "ab", " ", "\n", "\u{1b}"] {
            let error = Keymap::from_overrides(&table(&[("comment", bad)])).unwrap_err();
            assert!(
                matches!(error, KeymapError::NotOneCharacter { .. }),
                "{bad:?} gave {error:?}"
            );
        }
    }

    #[test]
    fn a_reserved_key_is_refused() {
        for key in ["q", ":", "j"] {
            let error = Keymap::from_overrides(&table(&[("comment", key)])).unwrap_err();
            assert!(matches!(error, KeymapError::Reserved { .. }), "{key}");
        }
    }

    #[test]
    fn two_actions_on_one_key_name_both() {
        let error = Keymap::from_overrides(&table(&[("comment", "n")])).unwrap_err();
        assert_eq!(
            error,
            KeymapError::Conflict {
                key: 'n',
                first: "next_unreviewed".to_owned(),
                second: "comment".to_owned(),
            }
        );
    }

    #[test]
    fn the_help_shows_the_new_key_and_keeps_function_keys() {
        let entry = Entry {
            action: Action::Mode(Mode::Impact),
            name: "Go to Impact",
            key: "4  g  F5",
        };
        let map = Keymap::from_overrides(&table(&[("impact", "i")])).unwrap();
        assert_eq!(map.label(&entry), "i  F5");
        assert_eq!(Keymap::default().label(&entry), "4  g  F5");
    }
}

//! # klaub-tui
//!
//! Klaub's semantic code reviewer, built with [Ratatui](https://ratatui.rs).
//!
//! The crate follows an Elm-style split so everything interesting is testable without a terminal:
//! [`App`] is plain state over a [`ReviewSession`](klaub_app::ReviewSession); [`handle_key`] applies
//! one key press to it; [`draw`] renders it into a Ratatui `Frame`. Only the `terminal` module
//! touches the real terminal.
//!
//! Nothing here submits anything to the agent. Putting the instruction into the agent's input
//! always goes through a confirmation that shows the destination and the exact text, and `Enter` is
//! never sent.

mod actions;
mod app;
mod keymap;
mod labels;
mod terminal;
mod theme;
mod ui;
mod update;
mod viewport;
mod views;
mod widgets;

pub use app::{
    App, CodeState, Confirm, ConfirmAction, DELIVERY_OPTIONS, Depth, Editor, EditorPurpose,
    Filters, FinishStage, FlowLine, ImpactRow, ImpactRowKind, ImpactState, Mode, Overlay, Peek,
    StatusKind, TreeKind, TreeRow,
};
pub use keymap::{Keymap, KeymapError};
pub use terminal::run;
pub use theme::Theme;
pub use ui::draw;
pub use update::{end_peek, handle_key, handle_mouse};
pub use viewport::Viewport;

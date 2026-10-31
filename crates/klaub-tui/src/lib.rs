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
    Filters, FinishStage, ImpactRow, ImpactRowKind, ImpactState, Mode, Overlay, StatusKind,
    TreeKind, TreeRow,
};
pub use terminal::run;
pub use theme::Theme;
pub use ui::draw;
pub use update::handle_key;
pub use viewport::Viewport;

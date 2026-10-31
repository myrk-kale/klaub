//! # klaub-tui
//!
//! Klaub's terminal user interface, built with [Ratatui](https://ratatui.rs).
//!
//! The crate follows an Elm-style split so the interesting parts are testable
//! without a terminal: a plain state value, a pure function that applies input to
//! it, and a view that draws it into a Ratatui `Frame`. Only the `terminal` module touches
//! the real terminal.
//!
//! At this stage it provides the review window's welcome screen. The semantic
//! reader (queue, code, impact, comments) builds on the same structure.

mod terminal;
mod welcome;

pub use terminal::run_welcome;
pub use welcome::{Welcome, draw_welcome, is_quit_key};

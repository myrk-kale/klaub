//! # Klaub
//!
//! A terminal code reviewer for AI coding agents. It reads the changes an agent made, groups them
//! by function, ranks them by risk and lets a human review them, offline and without an LLM.
//!
//! The code follows a hexagonal layout, kept by convention and checked by `tests/architecture.rs`.
//! Dependencies only point inwards:
//!
//! ```text
//! cli ──► tui ──► app ──► services ──► domain
//!  │                                     ▲
//!  └────────────► adapters ──────────────┘
//! ```
//!
//! - [`domain`]: the values everything speaks in, the configuration schema and the **ports**
//!   (traits) the adapters implement. No I/O;
//! - [`services`]: pure rules over domain values (matching, graph, rules, test evidence, context,
//!   flow, review). No I/O;
//! - [`app`]: the application layer. It orchestrates the services through the ports and never
//!   names an adapter;
//! - [`adapters`]: Git, parsing, storage and tmux behind the ports;
//! - [`tui`]: the interactive reviewer, split so that everything but the real terminal is testable;
//! - [`cli`]: the command line and the **composition root**, the one place where adapters are
//!   chosen and wired to the ports.
//!
//! `docs/architecture.md` explains the reasoning behind each layer.

pub mod adapters;
pub mod app;
pub mod cli;
pub mod domain;
pub mod services;
pub mod tui;

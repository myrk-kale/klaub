//! # klaub-tmux
//!
//! The tmux adapter of Klaub. It implements the
//! [`AgentChannel`](klaub_core::ports::AgentChannel) port and owns everything
//! that touches tmux:
//!
//! - [`binary`]: choosing the tmux executable, extracting the embedded one into a
//!   private directory and verifying its SHA-256 before every use;
//! - [`client`]: a thin wrapper that talks to a **private** server (own socket,
//!   empty configuration, `TMUX` removed from the environment);
//! - [`config`]: the server configuration and key bindings;
//! - [`registry`] and [`session`]: starting, finding, focusing and ending sessions;
//! - [`channel`]: pane-identity checks and the bracketed, single-line, no-`Enter` paste.
//!
//! See `docs/adr/0004-embedded-verified-tmux.md` and
//! `docs/adr/0005-human-confirmed-paste-never-enter.md`.

pub mod binary;
pub mod channel;
pub mod client;
pub mod config;
mod quote;
pub mod registry;
pub mod session;

pub use channel::TmuxAgentChannel;
pub use client::{PaneState, Spawn, TmuxClient};
pub use registry::{SessionRecord, SessionRegistry};

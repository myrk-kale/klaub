//! Small adapters implementing core ports with the real system.
//!
//! These are the only place in the workspace where a review reaches the clock, the clipboard or a
//! process: the architecture test forbids all three inside the hexagon, so they live here, at the
//! composition root.

mod process;
mod system;

pub(crate) use process::{ClipboardCopy, ProcessTestRunner};
pub(crate) use system::{RandomIds, SystemClock};

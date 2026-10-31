//! The only module that touches the real terminal.
//!
//! [`run`] takes over the terminal, draws the reader and feeds it keys until the user leaves.
//! Everything else in the crate is plain data, so tests drive it through [`draw`] and [`handle_key`]
//! with a `TestBackend` instead of this loop. The terminal is restored on the way out, and a panic
//! hook restores it too, so a drawing bug cannot leave the user's shell unusable.

use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

use klaub_core::{Error, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::crossterm::execute;

use crate::app::App;
use crate::ui;
use crate::update::{self, handle_key, handle_paste};

/// How long the loop waits for a key before probing the worktree for changes.
const POLL: Duration = Duration::from_millis(250);

fn terminal_error(error: io::Error) -> Error {
    Error::io("terminal", error)
}

/// Runs the reader until the user quits. The terminal is restored afterwards, whatever happens.
pub fn run(mut app: App) -> Result<()> {
    let mut terminal = ratatui::try_init().map_err(terminal_error)?;
    // Bracketed paste lets a paste arrive as one event, so it lands in the open field instead of
    // being read as a burst of key presses.
    let _ = execute!(io::stdout(), event::EnableBracketedPaste);

    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));

    let result = panic::catch_unwind(AssertUnwindSafe(|| event_loop(&mut terminal, &mut app)));
    restore_terminal();

    match result {
        Ok(outcome) => outcome,
        Err(payload) => panic::resume_unwind(payload),
    }
}

/// Puts the terminal back the way it was found: leaves the alternate screen, turns bracketed paste
/// off and flushes, so a half-drawn frame is not left behind.
fn restore_terminal() {
    let _ = execute!(io::stdout(), event::DisableBracketedPaste);
    ratatui::restore();
    let _ = io::stdout().flush();
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal
            .draw(|frame| ui::draw(frame, app))
            .map_err(terminal_error)?;
        // A deferred action blocks, so it runs only after the frame that announced it was shown.
        if app.deferred.is_some() {
            update::run_deferred(app);
            continue;
        }
        if app.should_quit {
            return Ok(());
        }
        if !event::poll(POLL).map_err(terminal_error)? {
            update::tick(app);
            continue;
        }
        match event::read().map_err(terminal_error)? {
            Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key),
            Event::Paste(text) => handle_paste(app, &text),
            Event::Resize(_, _)
            | Event::Key(_)
            | Event::FocusGained
            | Event::FocusLost
            | Event::Mouse(_) => {}
        }
    }
}

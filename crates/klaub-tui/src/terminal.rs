//! The only module that touches the real terminal.

use std::time::Duration;

use klaub_core::{Error, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::welcome::{Welcome, draw_welcome, is_quit_key};

fn terminal_error(error: std::io::Error) -> Error {
    Error::io("terminal", error)
}

/// Shows the welcome screen until the user quits. The terminal is always restored.
pub fn run_welcome(welcome: &Welcome) -> Result<()> {
    let mut terminal = ratatui::try_init().map_err(terminal_error)?;
    let result = event_loop(&mut terminal, welcome);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, welcome: &Welcome) -> Result<()> {
    loop {
        terminal
            .draw(|frame| draw_welcome(frame, welcome))
            .map_err(terminal_error)?;
        if event::poll(Duration::from_millis(250)).map_err(terminal_error)?
            && let Event::Key(key) = event::read().map_err(terminal_error)?
            && key.kind == KeyEventKind::Press
            && is_quit_key(&key)
        {
            return Ok(());
        }
    }
}

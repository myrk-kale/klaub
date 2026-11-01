//! The only module that touches the real terminal.
//!
//! [`run`] takes over the terminal, draws the reader and feeds it keys until the user leaves.
//! Everything else in the crate is plain data, so tests drive it through [`draw`] and [`handle_key`]
//! with a `TestBackend` instead of this loop. The terminal is restored on the way out, and a panic
//! hook restores it too, so a drawing bug cannot leave the user's shell unusable.

use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::domain::{Error, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;

use crate::tui::app::App;
use crate::tui::ui;
use crate::tui::update::{self, end_peek, handle_key, handle_mouse, handle_paste};

/// How long the loop waits for a key before probing the worktree for changes.
const POLL: Duration = Duration::from_millis(250);

/// Whether mouse reporting is on, so the way out knows to turn it off.
static MOUSE_ON: AtomicBool = AtomicBool::new(false);
/// Whether the extended keyboard protocol is pushed, so the way out knows to pop it.
static KEYS_ENHANCED: AtomicBool = AtomicBool::new(false);
/// Whether focus reporting is on.
static FOCUS_ON: AtomicBool = AtomicBool::new(false);

/// What the terminal must report for a key to be held: releases of every key, including the ones
/// that type text, and the shifted character so `Shift+b` still arrives as `B`.
const HOLD_FLAGS: KeyboardEnhancementFlags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
    .union(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
    .union(KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS)
    .union(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES);

fn terminal_error(error: io::Error) -> Error {
    Error::io("terminal", error)
}

/// Runs the reader until the user quits and hands the state back, so the caller can report what the
/// review ended as. The terminal is restored afterwards, whatever happens.
pub fn run(mut app: App) -> Result<App> {
    let mut terminal = ratatui::try_init().map_err(terminal_error)?;
    // Bracketed paste lets a paste arrive as one event, so it lands in the open field instead of
    // being read as a burst of key presses.
    let _ = execute!(io::stdout(), event::EnableBracketedPaste);
    prepare_input(&mut app);

    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));

    let result = panic::catch_unwind(AssertUnwindSafe(|| event_loop(&mut terminal, &mut app)));
    restore_terminal();

    match result {
        Ok(outcome) => outcome.map(|()| app),
        Err(payload) => panic::resume_unwind(payload),
    }
}

/// Turns on what `[ui]` asked for: mouse reporting, and focus reports plus key releases when a
/// key may be held. Releases are only used if the terminal says it can send them.
fn prepare_input(app: &mut App) {
    let ui = app.session.analysis().config.ui.clone();
    if ui.mouse && execute!(io::stdout(), event::EnableMouseCapture).is_ok() {
        MOUSE_ON.store(true, Ordering::SeqCst);
    }
    if ui.hold_to_peek && supports_keyboard_enhancement().unwrap_or(false) {
        app.hold_supported = true;
        if execute!(io::stdout(), event::EnableFocusChange).is_ok() {
            FOCUS_ON.store(true, Ordering::SeqCst);
        }
    }
}

/// Keeps the extended keyboard protocol on except while text is being typed. With it on, a key
/// that types a character arrives as an escape code, which would lose characters that need a
/// compose or `AltGr` sequence, so the text fields get the terminal's ordinary input.
fn sync_keyboard(app: &mut App) {
    let wanted = app.hold_supported && !app.text_input_open();
    if wanted == KEYS_ENHANCED.load(Ordering::SeqCst) {
        return;
    }
    let done = if wanted {
        execute!(io::stdout(), PushKeyboardEnhancementFlags(HOLD_FLAGS))
    } else {
        // A key that was being held cannot be released into a text field.
        end_peek(app);
        execute!(io::stdout(), PopKeyboardEnhancementFlags)
    };
    if done.is_ok() {
        KEYS_ENHANCED.store(wanted, Ordering::SeqCst);
    }
}

/// Puts the terminal back the way it was found: leaves the alternate screen, turns bracketed paste,
/// mouse and focus reporting and the extended keyboard protocol off and flushes, so a half-drawn
/// frame is not left behind.
fn restore_terminal() {
    if KEYS_ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    if FOCUS_ON.swap(false, Ordering::SeqCst) {
        let _ = execute!(io::stdout(), event::DisableFocusChange);
    }
    if MOUSE_ON.swap(false, Ordering::SeqCst) {
        let _ = execute!(io::stdout(), event::DisableMouseCapture);
    }
    let _ = execute!(io::stdout(), event::DisableBracketedPaste);
    ratatui::restore();
    let _ = io::stdout().flush();
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        sync_keyboard(app);
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
            // Presses, repeats and releases all go to `handle_key`, which knows which it wants.
            Event::Key(key) => handle_key(app, key),
            Event::Mouse(mouse) => handle_mouse(app, mouse),
            Event::Paste(text) => handle_paste(app, &text),
            // The release of a held key never arrives once the terminal lost focus.
            Event::FocusLost => end_peek(app),
            Event::Resize(_, _) | Event::FocusGained => {}
        }
    }
}

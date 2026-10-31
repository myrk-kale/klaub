//! The screen shown in the review window until the reader is loaded.

use klaub_core::text::sanitize_single_line;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// What the welcome screen shows. Every field is sanitised on construction because
/// paths and commands are untrusted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Welcome {
    session: Option<String>,
    worktree: String,
    agent: Option<String>,
}

impl Welcome {
    /// Builds the screen state, removing terminal control sequences from every field.
    pub fn new(session: Option<&str>, worktree: &str, agent: Option<&str>) -> Self {
        Self {
            session: session.map(sanitize_single_line),
            worktree: sanitize_single_line(worktree),
            agent: agent.map(sanitize_single_line),
        }
    }
}

/// Whether `key` asks to leave the screen (`q`, `Esc` or `Ctrl+C`).
pub fn is_quit_key(key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

/// Draws the welcome screen. Meaning never depends on colour alone.
pub fn draw_welcome(frame: &mut Frame<'_>, welcome: &Welcome) {
    let [body, footer] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

    let label = Style::default().add_modifier(Modifier::BOLD);
    let row = |name: &'static str, value: &str| {
        Line::from(vec![
            Span::styled(format!("{name:<9}"), label),
            Span::raw(value.to_owned()),
        ])
    };
    let mut lines = vec![
        Line::from(Span::styled("Review window", label)),
        Line::raw(""),
        row(
            "session",
            welcome
                .session
                .as_deref()
                .unwrap_or("none (stand-alone review)"),
        ),
        row("worktree", &welcome.worktree),
        row("agent", welcome.agent.as_deref().unwrap_or("none")),
        Line::raw(""),
    ];
    lines.push(Line::raw(
        "Your agent keeps running in the `agent` window. Switch with Prefix+A (agent) and Prefix+R (review).",
    ));

    let block = Block::default().borders(Borders::ALL).title(" Klaub ");
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        body,
    );
    frame.render_widget(
        Paragraph::new("q quit · Prefix+A agent · Prefix+? help")
            .style(Style::default().add_modifier(Modifier::DIM)),
        footer,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn text(buffer: &Buffer) -> String {
        let area = buffer.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render(welcome: &Welcome, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw_welcome(frame, welcome)).unwrap();
        text(terminal.backend().buffer())
    }

    #[test]
    fn shows_the_session_and_worktree() {
        let welcome = Welcome::new(Some("ses_abc123"), "~/dev/project", Some("codex"));
        let screen = render(&welcome, 80, 12);
        assert!(screen.contains("Klaub"), "{screen}");
        assert!(screen.contains("ses_abc123"), "{screen}");
        assert!(screen.contains("~/dev/project"), "{screen}");
        assert!(screen.contains("codex"), "{screen}");
        assert!(screen.contains("q quit"), "{screen}");
    }

    #[test]
    fn stand_alone_reviews_say_so() {
        let screen = render(&Welcome::new(None, "/repo", None), 80, 12);
        assert!(screen.contains("stand-alone review"), "{screen}");
    }

    #[test]
    fn hostile_text_cannot_reach_the_screen() {
        let welcome = Welcome::new(
            Some("ses_x"),
            "/tmp/\x1b]0;pwned\x07evil\x1b[2J",
            Some("a\x1b[31mb"),
        );
        assert_eq!(welcome.worktree, "/tmp/evil");
        let screen = render(&welcome, 80, 12);
        assert!(!screen.contains('\x1b'));
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        let welcome = Welcome::new(
            Some("ses_abc123"),
            "/a/very/long/path/to/some/worktree",
            Some("codex"),
        );
        for (w, h) in [(1, 1), (10, 3), (20, 5), (40, 2), (200, 60)] {
            let _ = render(&welcome, w, h);
        }
    }

    #[test]
    fn quit_keys() {
        let key = |code, modifiers| KeyEvent::new(code, modifiers);
        assert!(is_quit_key(&key(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(is_quit_key(&key(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(is_quit_key(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(!is_quit_key(&key(KeyCode::Char('c'), KeyModifiers::NONE)));
        assert!(!is_quit_key(&key(KeyCode::Char('x'), KeyModifiers::NONE)));
    }
}

//! Colours and text styles. Meaning never depends on colour alone: every coloured cue also has a
//! symbol or a word, and the high-contrast theme drops colour in favour of weight and reversal.

use klaub_core::config::Theme as ThemeName;
use klaub_core::finding::Priority;
use klaub_core::semantic::LineMark;
use klaub_core::symbol::HighlightClass;
use ratatui::style::{Color, Modifier, Style};

/// The palette in use.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    name: ThemeName,
}

impl Theme {
    /// A theme from its configuration name.
    pub fn new(name: ThemeName) -> Self {
        Self { name }
    }

    fn high_contrast(self) -> bool {
        self.name == ThemeName::HighContrast
    }

    fn pick(self, dark: Color, light: Color) -> Color {
        if self.name == ThemeName::Light {
            light
        } else {
            dark
        }
    }

    /// Plain text.
    pub fn text(self) -> Style {
        Style::default()
    }

    /// Secondary text.
    pub fn dim(self) -> Style {
        if self.high_contrast() {
            Style::default()
        } else {
            Style::default().add_modifier(Modifier::DIM)
        }
    }

    /// Titles and labels.
    pub fn strong(self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    /// The selected row.
    pub fn selected(self) -> Style {
        Style::default().add_modifier(Modifier::REVERSED)
    }

    /// The bar at the top.
    pub fn bar(self) -> Style {
        if self.high_contrast() {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.pick(Color::Black, Color::White))
                .bg(self.pick(Color::Gray, Color::DarkGray))
        }
    }

    /// A warning.
    pub fn warn(self) -> Style {
        if self.high_contrast() {
            self.strong()
        } else {
            Style::default().fg(self.pick(Color::Yellow, Color::Rgb(150, 90, 0)))
        }
    }

    /// An error.
    pub fn error(self) -> Style {
        if self.high_contrast() {
            self.strong()
        } else {
            Style::default().fg(self.pick(Color::LightRed, Color::Red))
        }
    }

    /// Something good (reviewed, passed).
    pub fn good(self) -> Style {
        if self.high_contrast() {
            self.strong()
        } else {
            Style::default().fg(self.pick(Color::LightGreen, Color::Green))
        }
    }

    /// The colour of a priority label.
    pub fn priority(self, priority: Priority) -> Style {
        if self.high_contrast() {
            return if priority.is_high() {
                self.strong()
            } else {
                Style::default()
            };
        }
        match priority {
            Priority::P0 => Style::default()
                .fg(self.pick(Color::LightRed, Color::Red))
                .add_modifier(Modifier::BOLD),
            Priority::P1 => {
                Style::default().fg(self.pick(Color::LightYellow, Color::Rgb(150, 90, 0)))
            }
            Priority::P2 => Style::default().fg(self.pick(Color::LightBlue, Color::Blue)),
            Priority::P3 => self.dim(),
        }
    }

    /// The background hint for a changed line (the line also has a marker character).
    pub fn mark(self, mark: LineMark) -> Style {
        match (self.high_contrast(), mark) {
            (_, LineMark::Same) => Style::default(),
            (true, _) => Style::default().add_modifier(Modifier::BOLD),
            (false, LineMark::Added) => {
                Style::default().fg(self.pick(Color::LightGreen, Color::Green))
            }
            (false, LineMark::Removed) => {
                Style::default().fg(self.pick(Color::LightRed, Color::Red))
            }
            (false, LineMark::Changed) => {
                Style::default().fg(self.pick(Color::LightYellow, Color::Rgb(150, 90, 0)))
            }
        }
    }

    /// Syntax colours.
    pub fn syntax(self, class: HighlightClass) -> Style {
        if self.high_contrast() {
            return match class {
                HighlightClass::Keyword => self.strong(),
                HighlightClass::Comment => Style::default().add_modifier(Modifier::ITALIC),
                _ => Style::default(),
            };
        }
        let color = match class {
            HighlightClass::Keyword => self.pick(Color::Magenta, Color::Rgb(130, 0, 130)),
            HighlightClass::String => self.pick(Color::Green, Color::Rgb(0, 110, 0)),
            HighlightClass::Comment => self.pick(Color::DarkGray, Color::Gray),
            HighlightClass::Number => self.pick(Color::Cyan, Color::Rgb(0, 100, 130)),
            HighlightClass::Type => self.pick(Color::Yellow, Color::Rgb(130, 90, 0)),
            HighlightClass::Function => self.pick(Color::LightBlue, Color::Blue),
        };
        Style::default().fg(color)
    }
}

/// The marker character shown in the gutter for a changed line.
pub fn mark_symbol(mark: LineMark) -> char {
    match mark {
        LineMark::Same => ' ',
        LineMark::Added => '+',
        LineMark::Removed => '-',
        LineMark::Changed => '~',
    }
}

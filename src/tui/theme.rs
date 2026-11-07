//! Colours and text styles. Meaning never depends on colour alone: every coloured cue also has a
//! symbol or a word, and the high-contrast theme drops colour in favour of weight and reversal.
//!
//! The colour themes are written in true colour and are downgraded to the 256-colour
//! palette when the terminal does not say it can show 24-bit colour, so the same design degrades
//! instead of turning into a wrong guess at sixteen colours. The named themes (Catppuccin, Dracula,
//! Gruvbox, Nord, Rosé Pine) use each project's published colours. Surfaces (bars, the selected row, the
//! tint behind a changed line) sit on top of the terminal's own background, which is never painted.

use crate::domain::config::Theme as ThemeName;
use crate::domain::finding::Priority;
use crate::domain::semantic::LineMark;
use crate::domain::symbol::HighlightClass;
use ratatui::style::{Color, Modifier, Style};

/// How many colours the terminal can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// 24-bit colour.
    TrueColor,
    /// The xterm 256-colour palette.
    Indexed,
}

impl ColorDepth {
    /// What the environment says about the terminal. `COLORTERM=truecolor` is the standard signal;
    /// a few terminals that do not set it are recognised by name.
    pub fn detect() -> Self {
        let var = |name: &str| std::env::var(name).unwrap_or_default().to_lowercase();
        let colorterm = var("COLORTERM");
        let term = var("TERM");
        let program = var("TERM_PROGRAM");
        let truecolor = colorterm.contains("truecolor")
            || colorterm.contains("24bit")
            || term.contains("direct")
            || term.contains("kitty")
            || term.contains("ghostty")
            || term.contains("alacritty")
            || matches!(
                program.as_str(),
                "iterm.app" | "wezterm" | "ghostty" | "vscode" | "hyper"
            );
        if truecolor {
            Self::TrueColor
        } else {
            Self::Indexed
        }
    }
}

/// A colour as red, green and blue.
type Rgb = (u8, u8, u8);

/// Every colour a theme uses. Names say the role, not the hue.
#[derive(Debug, Clone, Copy)]
struct Palette {
    /// Body text on a surface.
    fg: Rgb,
    /// Secondary text.
    muted: Rgb,
    /// Hints, separators, line numbers.
    faint: Rgb,
    /// Panel borders at rest.
    border: Rgb,
    /// The selected row and the cursor line.
    raised: Rgb,
    /// Text drawn on a solid accent chip.
    on_chip: Rgb,
    accent: Rgb,
    violet: Rgb,
    green: Rgb,
    red: Rgb,
    yellow: Rgb,
    orange: Rgb,
    teal: Rgb,
    /// Tint behind added, removed and changed lines.
    added_bg: Rgb,
    removed_bg: Rgb,
    changed_bg: Rgb,
}

const DARK: Palette = Palette {
    fg: (192, 202, 245),
    muted: (130, 139, 184),
    faint: (104, 114, 154),
    border: (47, 53, 79),
    raised: (41, 46, 66),
    on_chip: (22, 22, 30),
    accent: (122, 162, 247),
    violet: (187, 154, 247),
    green: (158, 206, 106),
    red: (247, 118, 142),
    yellow: (224, 175, 104),
    orange: (255, 158, 100),
    teal: (115, 218, 202),
    added_bg: (28, 48, 46),
    removed_bg: (58, 33, 46),
    changed_bg: (52, 46, 36),
};

const LIGHT: Palette = Palette {
    fg: (52, 59, 88),
    muted: (96, 104, 146),
    faint: (150, 156, 186),
    border: (192, 197, 218),
    raised: (212, 218, 238),
    on_chip: (250, 250, 252),
    accent: (46, 105, 214),
    violet: (125, 62, 205),
    green: (56, 122, 45),
    red: (200, 40, 80),
    yellow: (140, 100, 20),
    orange: (178, 84, 0),
    teal: (0, 122, 110),
    added_bg: (214, 236, 214),
    removed_bg: (248, 218, 224),
    changed_bg: (240, 232, 204),
};

/// Catppuccin Mocha: <https://catppuccin.com/palette>.
const CATPPUCCIN_MOCHA: Palette = Palette {
    fg: (205, 214, 244),
    muted: (166, 173, 200),
    faint: (108, 112, 134),
    border: (88, 91, 112),
    raised: (69, 71, 90),
    on_chip: (17, 17, 27),
    accent: (137, 180, 250),
    violet: (203, 166, 247),
    green: (166, 227, 161),
    red: (243, 139, 168),
    yellow: (249, 226, 175),
    orange: (250, 179, 135),
    teal: (148, 226, 213),
    added_bg: (49, 58, 62),
    removed_bg: (60, 45, 63),
    changed_bg: (62, 54, 48),
};

/// Catppuccin Latte, with the yellow darkened so warnings stay legible on a light surface.
const CATPPUCCIN_LATTE: Palette = Palette {
    fg: (76, 79, 105),
    muted: (108, 111, 133),
    faint: (140, 143, 161),
    border: (172, 176, 190),
    raised: (188, 192, 204),
    on_chip: (239, 241, 245),
    accent: (30, 102, 245),
    violet: (136, 57, 239),
    green: (64, 140, 43),
    red: (210, 15, 57),
    yellow: (178, 110, 10),
    orange: (220, 90, 10),
    teal: (23, 146, 153),
    added_bg: (211, 228, 213),
    removed_bg: (234, 205, 215),
    changed_bg: (229, 220, 207),
};

/// Dracula: <https://draculatheme.com/contribute>.
const DRACULA: Palette = Palette {
    fg: (248, 248, 242),
    muted: (180, 184, 208),
    faint: (98, 114, 164),
    border: (84, 88, 112),
    raised: (68, 71, 90),
    on_chip: (40, 42, 54),
    accent: (189, 147, 249),
    violet: (255, 121, 198),
    green: (80, 250, 123),
    red: (255, 85, 85),
    yellow: (241, 250, 140),
    orange: (255, 184, 108),
    teal: (139, 233, 253),
    added_bg: (45, 69, 63),
    removed_bg: (68, 48, 58),
    changed_bg: (66, 60, 50),
};

/// Gruvbox Dark: <https://github.com/morhetz/gruvbox>.
const GRUVBOX_DARK: Palette = Palette {
    fg: (235, 219, 178),
    muted: (168, 153, 132),
    faint: (124, 111, 100),
    border: (80, 73, 69),
    raised: (60, 56, 54),
    on_chip: (40, 40, 40),
    accent: (131, 165, 152),
    violet: (211, 134, 155),
    green: (184, 187, 38),
    red: (251, 73, 52),
    yellow: (250, 189, 47),
    orange: (254, 128, 25),
    teal: (142, 192, 124),
    added_bg: (62, 62, 40),
    removed_bg: (72, 45, 42),
    changed_bg: (74, 62, 38),
};

/// Nord: <https://www.nordtheme.com/docs/colors-and-palettes>.
const NORD: Palette = Palette {
    fg: (216, 222, 233),
    muted: (170, 179, 197),
    faint: (97, 110, 136),
    border: (76, 86, 106),
    raised: (67, 76, 94),
    on_chip: (46, 52, 64),
    accent: (136, 192, 208),
    violet: (180, 142, 173),
    green: (163, 190, 140),
    red: (191, 97, 106),
    yellow: (235, 203, 139),
    orange: (208, 135, 112),
    teal: (143, 188, 187),
    added_bg: (65, 74, 76),
    removed_bg: (69, 59, 71),
    changed_bg: (74, 68, 60),
};

/// Rosé Pine: <https://rosepinetheme.com/palette>. The palette has no green, so the
/// "good" colour is a mint close to its foam.
const ROSE_PINE: Palette = Palette {
    fg: (224, 222, 244),
    muted: (144, 140, 170),
    faint: (110, 106, 134),
    border: (74, 70, 98),
    raised: (54, 50, 78),
    on_chip: (25, 23, 36),
    accent: (196, 167, 231),
    violet: (235, 188, 186),
    green: (148, 208, 176),
    red: (235, 111, 146),
    yellow: (246, 193, 119),
    orange: (234, 154, 151),
    teal: (156, 207, 216),
    added_bg: (42, 49, 56),
    removed_bg: (54, 35, 51),
    changed_bg: (58, 46, 44),
};

/// The palette in use.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    name: ThemeName,
    depth: ColorDepth,
}

impl Theme {
    /// A theme from its configuration name, for the terminal this process runs in.
    pub fn new(name: ThemeName) -> Self {
        Self::with_depth(name, ColorDepth::detect())
    }

    /// A theme for a known colour depth (tests, and callers that already probed the terminal).
    pub fn with_depth(name: ThemeName, depth: ColorDepth) -> Self {
        Self { name, depth }
    }

    /// Whether this is the theme that uses weight and reversal instead of colour.
    pub fn is_high_contrast(self) -> bool {
        self.name == ThemeName::HighContrast
    }

    /// The configuration name of this theme.
    pub fn name(self) -> ThemeName {
        self.name
    }

    /// The same terminal with the next theme in the cycle.
    #[must_use]
    pub fn cycled(self) -> Self {
        Self {
            name: self.name.next(),
            depth: self.depth,
        }
    }

    fn pal(self) -> Palette {
        match self.name {
            ThemeName::Light => LIGHT,
            ThemeName::CatppuccinMocha => CATPPUCCIN_MOCHA,
            ThemeName::CatppuccinLatte => CATPPUCCIN_LATTE,
            ThemeName::Dracula => DRACULA,
            ThemeName::GruvboxDark => GRUVBOX_DARK,
            ThemeName::Nord => NORD,
            ThemeName::RosePine => ROSE_PINE,
            ThemeName::Dark | ThemeName::HighContrast => DARK,
        }
    }

    fn color(self, rgb: Rgb) -> Color {
        match self.depth {
            ColorDepth::TrueColor => Color::Rgb(rgb.0, rgb.1, rgb.2),
            ColorDepth::Indexed => Color::Indexed(nearest_indexed(rgb)),
        }
    }

    fn fg(self, pick: impl Fn(&Palette) -> Rgb) -> Style {
        Style::default().fg(self.color(pick(&self.pal())))
    }

    fn solid(self, pick: impl Fn(&Palette) -> Rgb) -> Style {
        let p = self.pal();
        Style::default()
            .fg(self.color(p.on_chip))
            .bg(self.color(pick(&p)))
            .add_modifier(Modifier::BOLD)
    }

    fn reversed() -> Style {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    }

    // ---- text ----------------------------------------------------------------------------------

    /// Plain text.
    pub fn text(self) -> Style {
        Style::default()
    }

    /// Secondary text.
    pub fn dim(self) -> Style {
        if self.is_high_contrast() {
            Style::default()
        } else {
            self.fg(|p| p.muted)
        }
    }

    /// `style` drawn on the bar: the bar's background under the style's own colours.
    pub(crate) fn on_bar(self, style: Style) -> Style {
        if self.is_high_contrast() {
            Self::reversed()
        } else {
            style
        }
    }

    /// The quietest text: hints, separators, line numbers.
    pub fn faint(self) -> Style {
        if self.is_high_contrast() {
            Style::default()
        } else {
            self.fg(|p| p.faint)
        }
    }

    /// Titles and labels.
    pub fn strong(self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    /// The accent colour, bold: the thing to look at.
    pub fn accent(self) -> Style {
        if self.is_high_contrast() {
            self.strong()
        } else {
            self.fg(|p| p.accent).add_modifier(Modifier::BOLD)
        }
    }

    /// A key name in a hint.
    pub fn key(self) -> Style {
        if self.is_high_contrast() {
            self.strong()
        } else {
            self.fg(|p| p.violet).add_modifier(Modifier::BOLD)
        }
    }

    // ---- surfaces ------------------------------------------------------------------------------

    /// The selected row.
    pub fn selected(self) -> Style {
        if self.is_high_contrast() {
            Self::reversed()
        } else {
            Style::default()
                .bg(self.color(self.pal().raised))
                .add_modifier(Modifier::BOLD)
        }
    }

    /// Lines picked with the selection keys.
    pub fn selection(self) -> Style {
        if self.is_high_contrast() {
            Style::default().add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
        } else {
            Style::default().bg(self.color(self.pal().border))
        }
    }

    /// The bar at the top and the one at the bottom.
    pub fn bar(self) -> Style {
        if self.is_high_contrast() {
            Self::reversed()
        } else {
            Style::default().fg(self.color(self.pal().fg))
        }
    }

    /// Muted text on the bar.
    pub fn bar_dim(self) -> Style {
        if self.is_high_contrast() {
            Self::reversed()
        } else {
            Style::default().fg(self.color(self.pal().muted))
        }
    }

    /// The product mark at the left of the title bar.
    pub fn brand(self) -> Style {
        if self.is_high_contrast() {
            Self::reversed()
        } else {
            self.accent()
        }
    }

    /// The current tab.
    pub fn tab_active(self) -> Style {
        if self.is_high_contrast() {
            Self::reversed()
        } else {
            self.accent().add_modifier(Modifier::UNDERLINED)
        }
    }

    /// A tab that is not current.
    pub fn tab_idle(self) -> Style {
        self.dim()
    }

    /// A panel border at rest.
    pub fn border(self) -> Style {
        if self.is_high_contrast() {
            Style::default()
        } else {
            self.fg(|p| p.border)
        }
    }

    /// A panel border that has the reader's attention, and overlay borders.
    pub fn border_focus(self) -> Style {
        if self.is_high_contrast() {
            self.strong()
        } else {
            self.fg(|p| p.accent)
        }
    }

    /// A small solid label, `fill` choosing its colour.
    pub(crate) fn chip(self, fill: Chip) -> Style {
        if self.is_high_contrast() {
            return Self::reversed();
        }
        match fill {
            Chip::Good => self.solid(|p| p.green),
            Chip::Warn => self.solid(|p| p.yellow),
            Chip::Quiet => {
                let p = self.pal();
                Style::default()
                    .fg(self.color(p.muted))
                    .bg(self.color(p.raised))
            }
        }
    }

    // ---- status colours ------------------------------------------------------------------------

    /// A warning.
    pub fn warn(self) -> Style {
        if self.is_high_contrast() {
            self.strong()
        } else {
            self.fg(|p| p.yellow)
        }
    }

    /// An error.
    pub fn error(self) -> Style {
        if self.is_high_contrast() {
            self.strong()
        } else {
            self.fg(|p| p.red)
        }
    }

    /// Something good (reviewed, passed).
    pub fn good(self) -> Style {
        if self.is_high_contrast() {
            self.strong()
        } else {
            self.fg(|p| p.green)
        }
    }

    /// The colour of a priority label.
    pub fn priority(self, priority: Priority) -> Style {
        if self.is_high_contrast() {
            return if priority.is_high() {
                self.strong()
            } else {
                Style::default()
            };
        }
        match priority {
            Priority::P0 => self.fg(|p| p.red).add_modifier(Modifier::BOLD),
            Priority::P1 => self.fg(|p| p.orange).add_modifier(Modifier::BOLD),
            Priority::P2 => self.fg(|p| p.accent),
            Priority::P3 => self.dim(),
        }
    }

    /// A priority as a solid label such as ` P0 `.
    pub fn priority_chip(self, priority: Priority) -> Style {
        if self.is_high_contrast() {
            return if priority.is_high() {
                Self::reversed()
            } else {
                Style::default()
            };
        }
        match priority {
            Priority::P0 => self.solid(|p| p.red),
            Priority::P1 => self.solid(|p| p.orange),
            Priority::P2 => self.solid(|p| p.accent),
            Priority::P3 => self.chip(Chip::Quiet),
        }
    }

    // ---- code ----------------------------------------------------------------------------------

    /// The style of code that has no syntax span of its own. Unchanged text stays plain; changed
    /// text takes the line's colour, so the gutter marker is never the only cue.
    pub fn mark_text(self, mark: LineMark) -> Style {
        match mark {
            LineMark::Same => Style::default(),
            _ if self.is_high_contrast() => Style::default().add_modifier(Modifier::BOLD),
            _ => Style::default(),
        }
    }

    /// The colour of the gutter marker of a changed line.
    pub fn mark(self, mark: LineMark) -> Style {
        match (self.is_high_contrast(), mark) {
            (_, LineMark::Same) => Style::default(),
            (true, _) => Style::default().add_modifier(Modifier::BOLD),
            (false, LineMark::Added) => self.fg(|p| p.green).add_modifier(Modifier::BOLD),
            (false, LineMark::Removed) => self.fg(|p| p.red).add_modifier(Modifier::BOLD),
            (false, LineMark::Changed) => self.fg(|p| p.yellow).add_modifier(Modifier::BOLD),
        }
    }

    /// The tint behind a changed line. Plain for an unchanged line and in the high-contrast theme.
    pub fn mark_bg(self, mark: LineMark) -> Style {
        if self.is_high_contrast() {
            return Style::default();
        }
        let p = self.pal();
        match mark {
            LineMark::Same => Style::default(),
            LineMark::Added => Style::default().bg(self.color(p.added_bg)),
            LineMark::Removed => Style::default().bg(self.color(p.removed_bg)),
            LineMark::Changed => Style::default().bg(self.color(p.changed_bg)),
        }
    }

    /// Syntax colours.
    pub fn syntax(self, class: HighlightClass) -> Style {
        if self.is_high_contrast() {
            return match class {
                HighlightClass::Keyword => self.strong(),
                HighlightClass::Comment => Style::default().add_modifier(Modifier::ITALIC),
                _ => Style::default(),
            };
        }
        match class {
            HighlightClass::Keyword => self.fg(|p| p.violet),
            HighlightClass::String => self.fg(|p| p.green),
            HighlightClass::Comment => self.fg(|p| p.faint).add_modifier(Modifier::ITALIC),
            HighlightClass::Number => self.fg(|p| p.orange),
            HighlightClass::Type => self.fg(|p| p.teal),
            HighlightClass::Function => self.fg(|p| p.accent),
        }
    }

    /// The colour a gauge fills with for a share between 0 and 100.
    pub fn gauge(self, percent: usize) -> Style {
        match percent {
            100.. => self.good(),
            50..=99 => self.fg_or_plain(|p| p.accent),
            _ => self.fg_or_plain(|p| p.yellow),
        }
    }

    fn fg_or_plain(self, pick: impl Fn(&Palette) -> Rgb) -> Style {
        if self.is_high_contrast() {
            Style::default()
        } else {
            self.fg(pick)
        }
    }
}

/// The colour of a [`Theme::chip`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Chip {
    /// Good news.
    Good,
    /// A warning.
    Warn,
    /// No emphasis.
    Quiet,
}

/// The marker character shown in the gutter for a changed line.
pub(crate) fn mark_symbol(mark: LineMark) -> char {
    match mark {
        LineMark::Same => ' ',
        LineMark::Added => '+',
        LineMark::Removed => '-',
        LineMark::Changed => '~',
    }
}

/// The xterm 256-colour index closest to a colour: the better of the 6×6×6 cube and the grey ramp.
fn nearest_indexed((r, g, b): Rgb) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest_level = |v: u8| -> usize {
        (0..LEVELS.len())
            .min_by_key(|&i| i32::from(LEVELS[i]).abs_diff(i32::from(v)))
            .unwrap_or(0)
    };
    let (ri, gi, bi) = (nearest_level(r), nearest_level(g), nearest_level(b));
    let cube = (LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let cube_index = 16 + 36 * ri + 6 * gi + bi;

    let average = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let grey_step = (average.saturating_sub(8) / 10).min(23);
    let grey_value = u8::try_from(8 + grey_step * 10).unwrap_or(238);
    let grey = (grey_value, grey_value, grey_value);
    let grey_index = 232 + grey_step;

    let distance = |c: Rgb| -> u32 {
        let d = |a: u8, b: u8| u32::from(a.abs_diff(b)).pow(2);
        d(c.0, r) + d(c.1, g) + d(c.2, b)
    };
    let chosen = if distance(grey) < distance(cube) {
        grey_index
    } else {
        u16::try_from(cube_index).unwrap_or(16)
    };
    u8::try_from(chosen).unwrap_or(16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_colours_land_on_the_nearest_cube_or_grey_entry() {
        assert_eq!(nearest_indexed((0, 0, 0)), 16);
        assert_eq!(nearest_indexed((255, 255, 255)), 231);
        assert_eq!(nearest_indexed((255, 0, 0)), 196);
        // A mid grey belongs to the grey ramp, not the cube.
        assert!((232..=255).contains(&nearest_indexed((128, 128, 128))));
    }

    #[test]
    fn high_contrast_uses_no_colour_at_all() {
        let theme = Theme::with_depth(ThemeName::HighContrast, ColorDepth::TrueColor);
        for style in [
            theme.dim(),
            theme.accent(),
            theme.selected(),
            theme.bar(),
            theme.brand(),
            theme.border(),
            theme.border_focus(),
            theme.warn(),
            theme.error(),
            theme.good(),
            theme.priority(Priority::P0),
            theme.priority_chip(Priority::P0),
            theme.mark(LineMark::Added),
            theme.mark_bg(LineMark::Added),
            theme.syntax(HighlightClass::Keyword),
        ] {
            assert_eq!((style.fg, style.bg), (None, None), "{style:?}");
        }
    }

    #[test]
    fn colour_themes_follow_the_terminal_depth() {
        let rich = Theme::with_depth(ThemeName::Dark, ColorDepth::TrueColor);
        let plain = Theme::with_depth(ThemeName::Dark, ColorDepth::Indexed);
        assert!(matches!(rich.accent().fg, Some(Color::Rgb(..))));
        assert!(matches!(plain.accent().fg, Some(Color::Indexed(_))));
    }

    #[test]
    fn a_changed_line_has_a_tint_and_an_unchanged_one_does_not() {
        let theme = Theme::with_depth(ThemeName::Dark, ColorDepth::TrueColor);
        assert!(theme.mark_bg(LineMark::Added).bg.is_some());
        assert!(theme.mark_bg(LineMark::Removed).bg.is_some());
        assert_eq!(theme.mark_bg(LineMark::Same).bg, None);
    }

    #[test]
    fn every_theme_has_a_palette_that_reads_and_a_distinct_look() {
        let mut seen = Vec::new();
        for name in ThemeName::ALL {
            let theme = Theme::with_depth(name, ColorDepth::TrueColor);
            if name == ThemeName::HighContrast {
                continue;
            }
            let look = (
                theme.accent().fg,
                theme.faint().fg,
                theme.selected().bg,
                theme.mark_bg(LineMark::Added).bg,
                theme.mark_bg(LineMark::Removed).bg,
            );
            assert!(
                look.0.is_some() && look.1.is_some() && look.2.is_some(),
                "{name:?}"
            );
            assert_ne!(look.3, look.4, "{name:?}: added and removed look alike");
            assert!(
                theme.bar().fg.is_some(),
                "{name:?}: the bars have text colour"
            );
            assert_eq!(
                theme.bar().bg,
                None,
                "{name:?}: the bars sit on the terminal"
            );
            assert!(!seen.contains(&look), "{name:?} duplicates another theme");
            seen.push(look);
        }
    }

    #[test]
    fn cycling_visits_every_theme_once_and_keeps_the_depth() {
        let start = Theme::with_depth(ThemeName::Dark, ColorDepth::Indexed);
        let mut at = start;
        let mut visited = vec![at.name()];
        for _ in 1..ThemeName::ALL.len() {
            at = at.cycled();
            assert!(!visited.contains(&at.name()));
            visited.push(at.name());
            assert!(matches!(at.accent().fg, Some(Color::Indexed(_)) | None));
        }
        assert_eq!(at.cycled().name(), start.name());
    }
}

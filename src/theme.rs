use ratatui::style::{Color, Modifier, Style};

/// True when ANSI bold should be rendered. Returns false when `NO_COLOR`
/// is set per the spec at https://no-color.org/.
pub fn ansi_bold_supported() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

/// Returns true if colored output should be suppressed.
pub fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

pub struct Theme {
    pub fg: Color,
    pub fg_dim: Color,
    pub accent: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,
    pub header_bg: Color,
}

impl Theme {
    pub fn current() -> Self {
        if no_color() {
            Self {
                fg: Color::Reset,
                fg_dim: Color::Reset,
                accent: Color::Reset,
                good: Color::Reset,
                warn: Color::Reset,
                bad: Color::Reset,
                header_bg: Color::Reset,
            }
        } else {
            Self {
                fg: Color::White,
                fg_dim: Color::Gray,
                accent: Color::Cyan,
                good: Color::Green,
                warn: Color::Yellow,
                bad: Color::Red,
                header_bg: Color::DarkGray,
            }
        }
    }

    pub fn selected_row(&self) -> Style {
        if no_color() {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
                .bg(Color::DarkGray)
                .fg(self.fg)
                .add_modifier(Modifier::BOLD)
        }
    }

    pub fn header(&self) -> Style {
        if no_color() {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.fg)
                .bg(self.header_bg)
                .add_modifier(Modifier::BOLD)
        }
    }
}

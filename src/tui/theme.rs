use ratatui::style::{Color, Modifier, Style};

/// Semantic roles used by the terminal UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Normal,
    Accent,
    Success,
    Warning,
    Selected,
    Muted,
    Error,
}

/// Terminal drawing characters, with a complete ASCII fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Glyphs {
    pub(crate) completed: &'static str,
    pub(crate) remaining: &'static str,
    pub(crate) expanded: &'static str,
    pub(crate) collapsed: &'static str,
    pub(crate) progress_full: &'static str,
    pub(crate) progress_empty: &'static str,
    pub(crate) bullet: &'static str,
    pub(crate) ellipsis: &'static str,
}

impl Glyphs {
    pub(crate) const UNICODE: Self = Self {
        completed: "✓",
        remaining: "○",
        expanded: "▾",
        collapsed: "▸",
        progress_full: "█",
        progress_empty: "░",
        bullet: "·",
        ellipsis: "…",
    };

    pub(crate) const ASCII: Self = Self {
        completed: "x",
        remaining: "o",
        expanded: "v",
        collapsed: ">",
        progress_full: "#",
        progress_empty: "-",
        bullet: ".",
        ellipsis: "...",
    };
}

/// Theme capabilities are explicit so rendering is deterministic in tests.
///
/// A terminal backend may choose these flags after probing its capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Theme {
    colors: bool,
    light_background: bool,
    pub(crate) glyphs: Glyphs,
}

impl Theme {
    #[must_use]
    pub(crate) const fn new(colors: bool, unicode: bool) -> Self {
        Self {
            colors,
            light_background: false,
            glyphs: if unicode {
                Glyphs::UNICODE
            } else {
                Glyphs::ASCII
            },
        }
    }

    #[must_use]
    pub(crate) const fn with_light_background(mut self, light: bool) -> Self {
        self.light_background = light;
        self
    }

    #[must_use]
    pub(crate) fn uses_unicode(&self) -> bool {
        self.glyphs == Glyphs::UNICODE
    }

    #[must_use]
    pub(crate) fn style(&self, role: Role) -> Style {
        if !self.colors {
            return match role {
                Role::Selected => Style::default().add_modifier(Modifier::REVERSED),
                Role::Accent | Role::Success | Role::Warning | Role::Error => {
                    Style::default().add_modifier(Modifier::BOLD)
                }
                Role::Muted => Style::default().add_modifier(Modifier::DIM),
                Role::Normal => Style::default(),
            };
        }

        match role {
            Role::Normal => Style::default().fg(if self.light_background {
                Color::Black
            } else {
                Color::White
            }),
            Role::Accent => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Role::Success => Style::default().fg(Color::Green),
            Role::Warning => Style::default().fg(Color::Yellow),
            Role::Selected => Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Role::Muted => Style::default().fg(Color::DarkGray),
            Role::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(true, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_glyphs_are_seven_bit_clean() {
        let glyphs = Theme::new(false, false).glyphs;
        for glyph in [
            glyphs.completed,
            glyphs.remaining,
            glyphs.expanded,
            glyphs.collapsed,
            glyphs.progress_full,
            glyphs.progress_empty,
            glyphs.bullet,
            glyphs.ellipsis,
        ] {
            assert!(glyph.is_ascii());
        }
    }

    #[test]
    fn no_color_selected_style_still_marks_focus() {
        let style = Theme::new(false, true).style(Role::Selected);
        assert!(style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn normal_text_adapts_to_light_and_dark_backgrounds() {
        assert_eq!(
            Theme::new(true, true).style(Role::Normal).fg,
            Some(Color::White)
        );
        assert_eq!(
            Theme::new(true, true)
                .with_light_background(true)
                .style(Role::Normal)
                .fg,
            Some(Color::Black)
        );
    }
}

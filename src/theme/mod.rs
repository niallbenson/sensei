//! Theming system for Sensei

mod tokyo_night;

pub use tokyo_night::TOKYO_NIGHT;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// A color theme for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,

    // Background colors
    pub bg_primary: Color,
    pub bg_secondary: Color,
    pub bg_tertiary: Color,

    // Foreground colors
    pub fg_primary: Color,
    pub fg_secondary: Color,
    pub fg_muted: Color,

    // Accent colors
    pub accent_primary: Color,
    pub accent_secondary: Color,

    // Semantic colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // Syntax highlighting
    pub syntax_keyword: Color,
    pub syntax_string: Color,
    pub syntax_number: Color,
    pub syntax_comment: Color,
    pub syntax_function: Color,
    pub syntax_type: Color,
    pub syntax_variable: Color,
    pub syntax_operator: Color,

    // UI elements
    pub border: Color,
    pub border_focused: Color,
    pub selection: Color,
    pub cursor: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::tokyo_night()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_tokyo_night() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Tokyo Night");
    }
}

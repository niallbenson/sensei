//! Tokyo Night theme implementation

use ratatui::style::Color;

use super::Theme;

/// Tokyo Night color palette
pub const TOKYO_NIGHT: Theme = Theme {
    name: String::new(), // Will be set properly with const fn when stabilized

    // Background colors
    bg_primary: Color::Rgb(26, 27, 38),   // #1a1b26
    bg_secondary: Color::Rgb(36, 40, 59), // #24283b
    bg_tertiary: Color::Rgb(65, 72, 104), // #414868

    // Foreground colors
    fg_primary: Color::Rgb(169, 177, 214),   // #a9b1d6
    fg_secondary: Color::Rgb(192, 202, 245), // #c0caf5
    fg_muted: Color::Rgb(86, 95, 137),       // #565f89

    // Accent colors
    accent_primary: Color::Rgb(122, 162, 247),   // #7aa2f7
    accent_secondary: Color::Rgb(187, 154, 247), // #bb9af7

    // Semantic colors
    success: Color::Rgb(158, 206, 106), // #9ece6a
    warning: Color::Rgb(224, 175, 104), // #e0af68
    error: Color::Rgb(247, 118, 142),   // #f7768e
    info: Color::Rgb(125, 207, 255),    // #7dcfff

    // Syntax highlighting
    syntax_keyword: Color::Rgb(187, 154, 247),  // #bb9af7
    syntax_string: Color::Rgb(158, 206, 106),   // #9ece6a
    syntax_number: Color::Rgb(255, 158, 100),   // #ff9e64
    syntax_comment: Color::Rgb(86, 95, 137),    // #565f89
    syntax_function: Color::Rgb(122, 162, 247), // #7aa2f7
    syntax_type: Color::Rgb(42, 195, 222),      // #2ac3de
    syntax_variable: Color::Rgb(192, 202, 245), // #c0caf5
    syntax_operator: Color::Rgb(137, 221, 255), // #89ddff

    // UI elements
    border: Color::Rgb(65, 72, 104),           // #414868
    border_focused: Color::Rgb(122, 162, 247), // #7aa2f7
    selection: Color::Rgb(40, 52, 87),         // #283457
    cursor: Color::Rgb(192, 202, 245),         // #c0caf5
};

// Workaround for const String
impl Theme {
    pub fn tokyo_night() -> Self {
        Theme { name: "Tokyo Night".to_string(), ..TOKYO_NIGHT }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokyo_night_has_correct_name() {
        let theme = Theme::tokyo_night();
        assert_eq!(theme.name, "Tokyo Night");
    }

    #[test]
    fn tokyo_night_colors_are_rgb() {
        let theme = Theme::tokyo_night();
        // Verify key colors use RGB format
        assert!(matches!(theme.bg_primary, Color::Rgb(_, _, _)));
        assert!(matches!(theme.accent_primary, Color::Rgb(_, _, _)));
    }
}

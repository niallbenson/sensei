//! Landing screen with animated ensō (zen circle)

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    widgets::Paragraph,
};

use crate::app::state::LandingAnimation;
use crate::theme::Theme;

/// Ensō brush stroke path - (row, col, char) in drawing order
/// Draws clockwise from bottom-left, with varied thickness
const ENSO_PATH: &[(usize, usize, char)] = &[
    // Bottom-left start - thin brush touching down
    (8, 10, '.'),
    (8, 9, '░'),
    (8, 8, '▒'),
    // Going up left side - brush thickens
    (7, 7, '▒'),
    (7, 6, '▓'),
    (7, 5, '█'),
    (6, 5, '█'),
    (6, 4, '▓'),
    (5, 4, '█'),
    (5, 3, '█'),
    (4, 3, '█'),
    (4, 2, '▌'),
    (3, 3, '█'),
    (3, 4, '█'),
    (2, 4, '▓'),
    (2, 5, '█'),
    // Approaching top - full thickness
    (1, 5, '▒'),
    (1, 6, '█'),
    (1, 7, '█'),
    (1, 8, '▀'),
    // Top of circle
    (0, 8, '░'),
    (0, 9, '▒'),
    (0, 10, '▓'),
    (0, 11, '█'),
    (0, 12, '█'),
    (0, 13, '▓'),
    (0, 14, '▒'),
    (0, 15, '░'),
    // Down right side
    (1, 15, '▀'),
    (1, 16, '█'),
    (1, 17, '█'),
    (1, 18, '▒'),
    (2, 18, '█'),
    (2, 19, '▓'),
    (3, 19, '█'),
    (3, 20, '█'),
    (4, 20, '▐'),
    (4, 21, '█'),
    (5, 20, '█'),
    (5, 21, '█'),
    (6, 19, '▓'),
    (6, 20, '█'),
    // Bottom right - brush lifting
    (7, 18, '▒'),
    (7, 19, '█'),
    (7, 20, '▓'),
    (8, 15, '▓'),
    (8, 16, '▒'),
    (8, 17, '░'),
    // Trailing stroke - brush lifting off (the traditional gap)
    (9, 19, '░'),
    (9, 20, '.'),
];

/// The complete ensō for reference (what it looks like when fully drawn)
/// ```text
///         ░▒▓██▓▒░
///      ▒██▀      ▀██▒
///     ▓█            █▓
///    ██              ██
///    █▌              ▐█
///    ██              ██
///     ▓█            █▓
///      ▒█▓        ▓█▒
///         ▒▓█  ▓▒░
///                   ░.
/// ```
const ENSO_ROWS: usize = 10;
const ENSO_COLS: usize = 24;

const TITLE: &str = "SENSEI";
const TAGLINE: &str = "Your AI-powered guide to mastering technical books";
const PROMPT: &str = "Press any key to begin...";

/// Build the ensō string based on animation progress
fn build_enso(progress: f32) -> String {
    // Create empty grid
    let mut grid: Vec<Vec<char>> = vec![vec![' '; ENSO_COLS]; ENSO_ROWS];

    // Calculate how many path segments to draw
    let segments_to_draw = ((ENSO_PATH.len() as f32) * progress) as usize;

    // Draw segments
    for (i, &(row, col, ch)) in ENSO_PATH.iter().enumerate() {
        if i < segments_to_draw && row < ENSO_ROWS && col < ENSO_COLS {
            grid[row][col] = ch;
        }
    }

    // Convert grid to string
    grid.iter().map(|row| row.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
}

/// Draw the landing screen with ensō animation
pub fn draw(frame: &mut Frame, animation: &LandingAnimation, theme: &Theme) {
    let area = frame.area();

    // Fill background
    let bg_style = Style::default().bg(theme.bg_primary);
    frame.render_widget(Paragraph::new("").style(bg_style), area);

    // Build and render the ensō
    let enso_str = build_enso(animation.enso_progress());
    let enso_style = Style::default().fg(theme.accent_primary).bg(theme.bg_primary);

    // Center the ensō vertically in upper portion
    let enso_y = (area.height / 4).min(area.height.saturating_sub(ENSO_ROWS as u16 + 15));
    let enso_area = Rect {
        x: area.x,
        y: enso_y,
        width: area.width,
        height: (ENSO_ROWS as u16).min(area.height.saturating_sub(enso_y)),
    };
    let enso = Paragraph::new(enso_str).style(enso_style).alignment(Alignment::Center);
    frame.render_widget(enso, enso_area);

    // Title "SENSEI" - fade in character by character
    let title_chars = animation.title_chars();
    if title_chars > 0 {
        let visible_title: String = TITLE.chars().take(title_chars).collect();
        // Pad with spaces to maintain centering
        let padding = " ".repeat(6 - title_chars);
        let padded_title = format!("{}{}", visible_title, padding);

        let title_style = Style::default().fg(theme.fg_secondary).bg(theme.bg_primary);

        let title_y = enso_area.y + enso_area.height + 2;
        if title_y < area.height {
            let title_area = Rect {
                x: area.x,
                y: title_y,
                width: area.width,
                height: 1.min(area.height.saturating_sub(title_y)),
            };
            let title =
                Paragraph::new(padded_title).style(title_style).alignment(Alignment::Center);
            frame.render_widget(title, title_area);
        }
    }

    // Tagline
    if animation.show_tagline() {
        let tagline_style = Style::default().fg(theme.fg_muted).bg(theme.bg_primary);

        let tagline_y = enso_area.y + enso_area.height + 5;
        if tagline_y < area.height {
            let tagline_area = Rect {
                x: area.x,
                y: tagline_y,
                width: area.width,
                height: 1.min(area.height.saturating_sub(tagline_y)),
            };
            let tagline = Paragraph::new(TAGLINE).style(tagline_style).alignment(Alignment::Center);
            frame.render_widget(tagline, tagline_area);
        }
    }

    // Blinking prompt
    if animation.complete {
        let blink = (animation.start_time.elapsed().as_millis() / 500) % 2 == 0;
        if blink {
            let prompt_style = Style::default().fg(theme.fg_muted).bg(theme.bg_primary);

            let prompt_y = enso_area.y + enso_area.height + 9;
            if prompt_y < area.height {
                let prompt_area = Rect {
                    x: area.x,
                    y: prompt_y,
                    width: area.width,
                    height: 1.min(area.height.saturating_sub(prompt_y)),
                };
                let prompt =
                    Paragraph::new(PROMPT).style(prompt_style).alignment(Alignment::Center);
                frame.render_widget(prompt, prompt_area);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enso_path_has_segments() {
        assert!(ENSO_PATH.len() > 30);
    }

    #[test]
    fn build_enso_empty_at_zero() {
        let enso = build_enso(0.0);
        // skipcq: RS-W1049 - filter is appropriate here for predicate-based counting
        let non_space: usize = enso.chars().filter(|c| !c.is_whitespace()).count();
        assert_eq!(non_space, 0);
    }

    #[test]
    fn build_enso_full_at_one() {
        let enso = build_enso(1.0);
        // skipcq: RS-W1049 - filter is appropriate here for predicate-based counting
        let non_space: usize = enso.chars().filter(|c| !c.is_whitespace()).count();
        assert_eq!(non_space, ENSO_PATH.len());
    }

    #[test]
    fn build_enso_partial() {
        let enso = build_enso(0.5);
        // skipcq: RS-W1049 - filter is appropriate here for predicate-based counting
        let non_space: usize = enso.chars().filter(|c| !c.is_whitespace()).count();
        assert!(non_space > 0);
        assert!(non_space < ENSO_PATH.len());
    }
}

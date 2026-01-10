//! Command line UI component

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::state::{CommandLineState, CommandMode};
use crate::theme::Theme;

/// Draw the command line at the bottom of the screen
pub fn draw(frame: &mut Frame, area: Rect, state: &CommandLineState, theme: &Theme) {
    let (text, style) = match state.mode {
        CommandMode::Normal => {
            // Show message or empty
            if let Some(ref msg) = state.message {
                let style = if state.is_error {
                    Style::default().fg(theme.error)
                } else {
                    Style::default().fg(theme.fg_muted)
                };
                (msg.clone(), style)
            } else {
                // Show hint when empty
                (
                    String::from("Press : for commands, / for search"),
                    Style::default().fg(theme.fg_muted),
                )
            }
        }
        CommandMode::Command => {
            let text = format!(":{}", state.input);
            (text, Style::default().fg(theme.accent_primary))
        }
        CommandMode::Search => {
            let text = format!("/{}", state.input);
            (text, Style::default().fg(theme.info))
        }
    };

    // Build the line with cursor if in input mode
    let line = if state.is_input_mode() {
        build_line_with_cursor(&text, state.cursor + 1, style, theme) // +1 for prefix
    } else {
        Line::from(Span::styled(text, style))
    };

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Build a line with a visible cursor
fn build_line_with_cursor(
    text: &str,
    cursor_pos: usize,
    base_style: Style,
    theme: &Theme,
) -> Line<'static> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();

    // Text before cursor
    if cursor_pos > 0 {
        let before: String = chars.iter().take(cursor_pos).collect();
        spans.push(Span::styled(before, base_style));
    }

    // Cursor character (or space if at end)
    let cursor_char = chars.get(cursor_pos).copied().unwrap_or(' ');
    let cursor_style =
        Style::default().fg(theme.bg_primary).bg(theme.fg_primary).add_modifier(Modifier::BOLD);
    spans.push(Span::styled(cursor_char.to_string(), cursor_style));

    // Text after cursor
    if cursor_pos + 1 < chars.len() {
        let after: String = chars.iter().skip(cursor_pos + 1).collect();
        spans.push(Span::styled(after, base_style));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_cursor_at_start() {
        let theme = Theme::default();
        let line = build_line_with_cursor(":test", 0, Style::default(), &theme);
        assert_eq!(line.spans.len(), 2); // cursor + rest
    }

    #[test]
    fn build_cursor_at_end() {
        let theme = Theme::default();
        let line = build_line_with_cursor(":test", 5, Style::default(), &theme);
        assert_eq!(line.spans.len(), 2); // before + cursor (space)
    }

    #[test]
    fn build_cursor_in_middle() {
        let theme = Theme::default();
        let line = build_line_with_cursor(":test", 2, Style::default(), &theme);
        assert_eq!(line.spans.len(), 3); // before + cursor + after
    }
}

//! Claude response panel component

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::state::AppState;
use crate::theme::Theme;

/// Draw the Claude response panel as a centered overlay
pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, theme: &Theme) {
    // Don't draw if response panel is not visible
    if !state.claude.is_response_visible() {
        return;
    }

    // Calculate centered overlay area (80% width, 80% height)
    let overlay_area = centered_rect(80, 80, area);

    // Clear the background area
    frame.render_widget(Clear, overlay_area);

    // Create the panel block
    let title =
        if state.claude.streaming { " Claude (streaming...) " } else { " Claude Response " };

    let block = Block::default()
        .title(title)
        .title_bottom(Line::from(" [c] toggle  [j/k] scroll  [Esc] close ").centered())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .style(Style::default().bg(theme.bg_secondary));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    // Get the response text
    let text =
        if state.claude.streaming { &state.claude.stream_buffer } else { &state.claude.response };

    if text.is_empty() {
        let empty = Paragraph::new("No response yet...").style(Style::default().fg(theme.fg_muted));
        frame.render_widget(empty, inner);
        return;
    }

    // Wrap text and create lines
    let width = inner.width.saturating_sub(2) as usize;
    let lines: Vec<Line> = text
        .lines()
        .flat_map(|line| {
            if line.is_empty() { vec![Line::from("")] } else { wrap_line(line, width, theme) }
        })
        .collect();

    // Calculate max scroll
    let visible_lines = inner.height as usize;
    let total_lines = lines.len();
    let max_scroll = total_lines.saturating_sub(visible_lines);

    // Clamp scroll position
    let scroll = (state.claude.response_scroll as usize).min(max_scroll);
    state.claude.response_scroll = scroll as u16;

    // Create scrollable paragraph
    let para = Paragraph::new(lines)
        .style(Style::default().fg(theme.fg_primary))
        .scroll((scroll as u16, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(para, inner);

    // Draw scroll indicator if needed
    if total_lines > visible_lines {
        draw_scroll_indicator(frame, inner, scroll, max_scroll, theme);
    }
}

/// Wrap a single line of text
fn wrap_line(line: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from("")];
    }

    // Simple word wrapping
    let mut result = Vec::new();
    let mut current_line = String::new();

    for word in line.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            result.push(style_line(&current_line, theme));
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() || result.is_empty() {
        result.push(style_line(&current_line, theme));
    }

    result
}

/// Apply styling to a line (handle code blocks, etc.)
fn style_line(text: &str, theme: &Theme) -> Line<'static> {
    // Check for inline code (backticks)
    if text.contains('`') {
        let mut spans = Vec::new();
        let mut in_code = false;
        let mut current = String::new();

        for ch in text.chars() {
            if ch == '`' {
                if !current.is_empty() {
                    let style = if in_code {
                        Style::default().fg(theme.syntax_string).bg(theme.bg_tertiary)
                    } else {
                        Style::default().fg(theme.fg_primary)
                    };
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }
                in_code = !in_code;
            } else {
                current.push(ch);
            }
        }

        if !current.is_empty() {
            let style = if in_code {
                Style::default().fg(theme.syntax_string).bg(theme.bg_tertiary)
            } else {
                Style::default().fg(theme.fg_primary)
            };
            spans.push(Span::styled(current, style));
        }

        Line::from(spans)
    } else if text.starts_with("```") || text.starts_with("   ") || text.starts_with("\t") {
        // Code block line
        Line::from(Span::styled(text.to_string(), Style::default().fg(theme.syntax_function)))
    } else if text.starts_with('#') {
        // Heading
        Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(theme.accent_primary).add_modifier(Modifier::BOLD),
        ))
    } else if text.starts_with("- ") || text.starts_with("* ") || text.starts_with("• ") {
        // List item
        Line::from(Span::styled(text.to_string(), Style::default().fg(theme.fg_primary)))
    } else {
        Line::from(text.to_string())
    }
}

/// Draw scroll indicator on the right side
fn draw_scroll_indicator(
    frame: &mut Frame,
    area: Rect,
    scroll: usize,
    max_scroll: usize,
    theme: &Theme,
) {
    if area.height < 3 || max_scroll == 0 {
        return;
    }

    let track_height = area.height.saturating_sub(2) as usize;
    if track_height == 0 {
        return;
    }

    let thumb_pos = if max_scroll > 0 { (scroll * track_height) / max_scroll } else { 0 };

    // Draw the thumb at the calculated position
    let thumb_y = area.y + 1 + thumb_pos as u16;
    let thumb_x = area.x + area.width - 1;

    if thumb_y < area.y + area.height - 1 {
        let thumb = Paragraph::new("█").style(Style::default().fg(theme.fg_muted));
        frame.render_widget(thumb, Rect::new(thumb_x, thumb_y, 1, 1));
    }
}

/// Create a centered rectangle with the given percentage of width and height
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

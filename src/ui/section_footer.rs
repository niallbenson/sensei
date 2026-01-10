//! Section footer component with Quiz and Complete buttons

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::state::AppState;
use crate::theme::Theme;

/// Height of the section footer in lines
pub const FOOTER_HEIGHT: u16 = 3;

/// Draw the section footer with two buttons
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    if area.height < FOOTER_HEIGHT || area.width < 40 {
        return;
    }

    let footer_focused = state.content.footer_focused;
    let selected_button = state.content.footer_button_index;

    // Create button text with styling
    let quiz_button = create_button(" Take Quiz ", footer_focused && selected_button == 0, theme);

    let next_button = create_button(
        " Complete & Next \u{2192} ", // → arrow
        footer_focused && selected_button == 1,
        theme,
    );

    // Calculate button widths and centering
    let quiz_width = 14; // " Take Quiz " + borders
    let next_width = 22; // " Complete & Next → " + borders
    let gap = 4;
    let total_width = quiz_width + gap + next_width;

    let start_x = if area.width > total_width as u16 {
        area.x + (area.width - total_width as u16) / 2
    } else {
        area.x
    };

    // Draw separator line above buttons
    let separator = Line::from(vec![Span::styled(
        "\u{2500}".repeat(area.width as usize), // ─ horizontal line
        Style::default().fg(theme.border),
    )]);
    frame.render_widget(Paragraph::new(separator), Rect::new(area.x, area.y, area.width, 1));

    // Position for buttons (centered vertically in remaining space)
    let button_y = area.y + 1;

    // Draw quiz button
    let quiz_area = Rect::new(start_x, button_y, quiz_width as u16, 1);
    frame.render_widget(Paragraph::new(quiz_button), quiz_area);

    // Draw next button
    let next_area =
        Rect::new(start_x + quiz_width as u16 + gap as u16, button_y, next_width as u16, 1);
    frame.render_widget(Paragraph::new(next_button), next_area);

    // Draw hint line
    let hint = if footer_focused {
        Line::from(vec![
            Span::styled("[h/l]", Style::default().fg(theme.fg_muted)),
            Span::styled(" switch  ", Style::default().fg(theme.fg_secondary)),
            Span::styled("[Enter]", Style::default().fg(theme.fg_muted)),
            Span::styled(" select  ", Style::default().fg(theme.fg_secondary)),
            Span::styled("[k]", Style::default().fg(theme.fg_muted)),
            Span::styled(" back to content", Style::default().fg(theme.fg_secondary)),
        ])
    } else {
        Line::from(vec![
            Span::styled("[j]", Style::default().fg(theme.fg_muted)),
            Span::styled(
                " at end of content to access buttons",
                Style::default().fg(theme.fg_secondary),
            ),
        ])
    };

    let hint_para = Paragraph::new(hint);
    let hint_area = Rect::new(area.x, area.y + 2, area.width, 1);
    frame.render_widget(hint_para, hint_area);
}

/// Create a styled button
fn create_button<'a>(text: &'a str, focused: bool, theme: &Theme) -> Line<'a> {
    let style = if focused {
        Style::default().fg(theme.bg_primary).bg(theme.accent_primary).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.fg_secondary).bg(theme.bg_tertiary)
    };

    Line::from(vec![Span::styled(text, style)])
}

/// Get the lines that make up the footer (for scroll calculation)
pub fn footer_lines() -> usize {
    FOOTER_HEIGHT as usize
}

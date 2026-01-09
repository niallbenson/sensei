//! Layout utilities and common components

use ratatui::{Frame, layout::Alignment, style::Style, widgets::Paragraph};

use crate::theme::Theme;

/// Draw a placeholder screen (for unimplemented screens)
pub fn draw_placeholder(frame: &mut Frame, message: &str, theme: &Theme) {
    let area = frame.area();

    let style = Style::default().fg(theme.fg_muted).bg(theme.bg_primary);

    let placeholder = Paragraph::new(message).style(style).alignment(Alignment::Center);

    frame.render_widget(placeholder, area);
}

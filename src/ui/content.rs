//! Content block renderer

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::state::AppState;
use crate::book::ContentBlock;
use crate::theme::Theme;

/// Draw the content panel with section content
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme, focused: bool) {
    let border_color = if focused { theme.border_focused } else { theme.border };

    let title = if let Some(book) = &state.book {
        if let Some(section) = book.get_section(state.current_chapter, state.current_section) {
            format!(" {} ", section.title)
        } else {
            " Content ".to_string()
        }
    } else {
        " Content ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg_primary));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // If no book loaded, show welcome message
    let Some(book) = &state.book else {
        draw_welcome(frame, inner, theme);
        return;
    };

    // Render section content
    let Some(section) = book.get_section(state.current_chapter, state.current_section) else {
        let msg = Paragraph::new("Select a section from the curriculum")
            .style(Style::default().fg(theme.fg_muted));
        frame.render_widget(msg, inner);
        return;
    };

    // Render content blocks
    let lines = render_content_blocks(&section.content, theme, inner.width as usize);

    // Handle scroll
    let visible_height = inner.height as usize;
    let start = state.content.scroll_offset.min(lines.len().saturating_sub(1));
    let end = (start + visible_height).min(lines.len());
    let visible_lines: Vec<Line> = lines.into_iter().skip(start).take(end - start).collect();

    let content = Paragraph::new(visible_lines);
    frame.render_widget(content, inner);
}

/// Draw the welcome message when no book is loaded
fn draw_welcome(frame: &mut Frame, area: Rect, theme: &Theme) {
    let welcome = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Welcome to Sensei",
            Style::default().fg(theme.accent_primary).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Your AI-powered guide to mastering technical books",
            Style::default().fg(theme.fg_secondary),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled("Getting Started:", Style::default().fg(theme.fg_primary))),
        Line::from(""),
        Line::from(Span::styled(
            "  1. Add a book: sensei add <path/to/book>",
            Style::default().fg(theme.fg_muted),
        )),
        Line::from(Span::styled(
            "  2. Navigate with j/k (up/down)",
            Style::default().fg(theme.fg_muted),
        )),
        Line::from(Span::styled("  3. Press Enter to select", Style::default().fg(theme.fg_muted))),
        Line::from(Span::styled("  4. Press ? for help", Style::default().fg(theme.fg_muted))),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled("Keyboard Shortcuts:", Style::default().fg(theme.fg_primary))),
        Line::from(""),
        Line::from(Span::styled(
            "  [ or 1  Toggle curriculum panel",
            Style::default().fg(theme.fg_muted),
        )),
        Line::from(Span::styled(
            "  ] or 3  Toggle notes panel",
            Style::default().fg(theme.fg_muted),
        )),
        Line::from(Span::styled(
            "  h/l     Move between panels",
            Style::default().fg(theme.fg_muted),
        )),
        Line::from(Span::styled("  /       Search", Style::default().fg(theme.fg_muted))),
        Line::from(Span::styled("  q       Quit", Style::default().fg(theme.fg_muted))),
    ];

    let content = Paragraph::new(welcome).wrap(Wrap { trim: true });
    frame.render_widget(content, area);
}

/// Render content blocks to styled lines
pub fn render_content_blocks(
    blocks: &[ContentBlock],
    theme: &Theme,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for block in blocks {
        match block {
            ContentBlock::Heading { level, text } => {
                render_heading(&mut lines, *level, text, theme);
            }
            ContentBlock::Paragraph(text) => {
                render_paragraph(&mut lines, text, theme, width);
            }
            ContentBlock::Code(code) => {
                render_code_block(&mut lines, code, theme);
            }
            ContentBlock::UnorderedList(items) => {
                render_unordered_list(&mut lines, items, theme);
            }
            ContentBlock::OrderedList(items) => {
                render_ordered_list(&mut lines, items, theme);
            }
            ContentBlock::Blockquote(text) => {
                render_blockquote(&mut lines, text, theme);
            }
            ContentBlock::HorizontalRule => {
                render_horizontal_rule(&mut lines, theme, width);
            }
            ContentBlock::Image { alt, .. } => {
                render_image(&mut lines, alt, theme);
            }
            ContentBlock::Table(table) => {
                render_table(&mut lines, table, theme);
            }
        }
    }

    lines
}

fn render_heading(lines: &mut Vec<Line<'static>>, level: u8, text: &str, theme: &Theme) {
    let style = match level {
        1 => Style::default()
            .fg(theme.accent_primary)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        2 => Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(theme.fg_secondary),
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(text.to_string(), style)));
    lines.push(Line::from(""));
}

fn render_paragraph(lines: &mut Vec<Line<'static>>, text: &str, theme: &Theme, width: usize) {
    // Word-wrap paragraph
    let wrapped = wrap_text(text, width.saturating_sub(2));
    for line in wrapped {
        lines.push(Line::from(Span::styled(line, Style::default().fg(theme.fg_primary))));
    }
    lines.push(Line::from(""));
}

fn render_code_block(lines: &mut Vec<Line<'static>>, code: &crate::book::CodeBlock, theme: &Theme) {
    // Language label
    if let Some(lang) = &code.language {
        lines.push(Line::from(Span::styled(
            format!("─── {} ───", lang),
            Style::default().fg(theme.fg_muted),
        )));
    }

    // Code content with basic syntax highlighting by line
    for line in code.code.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {}", line),
            Style::default().fg(theme.syntax_keyword).bg(theme.bg_secondary),
        )));
    }
    lines.push(Line::from(""));
}

fn render_unordered_list(lines: &mut Vec<Line<'static>>, items: &[String], theme: &Theme) {
    for item in items {
        lines.push(Line::from(Span::styled(
            format!("  • {}", item),
            Style::default().fg(theme.fg_primary),
        )));
    }
    lines.push(Line::from(""));
}

fn render_ordered_list(lines: &mut Vec<Line<'static>>, items: &[String], theme: &Theme) {
    for (i, item) in items.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            format!("  {}. {}", i + 1, item),
            Style::default().fg(theme.fg_primary),
        )));
    }
    lines.push(Line::from(""));
}

fn render_blockquote(lines: &mut Vec<Line<'static>>, text: &str, theme: &Theme) {
    lines
        .push(Line::from(Span::styled(format!("│ {}", text), Style::default().fg(theme.fg_muted))));
    lines.push(Line::from(""));
}

fn render_horizontal_rule(lines: &mut Vec<Line<'static>>, theme: &Theme, width: usize) {
    let rule_width = width.saturating_sub(4).min(32);
    lines.push(Line::from(Span::styled("─".repeat(rule_width), Style::default().fg(theme.border))));
}

fn render_image(lines: &mut Vec<Line<'static>>, alt: &str, theme: &Theme) {
    lines.push(Line::from(Span::styled(
        format!("[Image: {}]", alt),
        Style::default().fg(theme.fg_muted),
    )));
}

fn render_table(lines: &mut Vec<Line<'static>>, table: &crate::book::Table, theme: &Theme) {
    // Header row
    lines.push(Line::from(Span::styled(
        table.headers.join(" │ "),
        Style::default().fg(theme.fg_primary).add_modifier(Modifier::BOLD),
    )));

    // Separator
    lines.push(Line::from(Span::styled(
        "─".repeat(table.headers.len() * 10),
        Style::default().fg(theme.border),
    )));

    // Data rows
    for row in &table.rows {
        lines.push(Line::from(Span::styled(
            row.join(" │ "),
            Style::default().fg(theme.fg_secondary),
        )));
    }
    lines.push(Line::from(""));
}

/// Simple word-wrapping function
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            result.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_text_short() {
        let result = wrap_text("hello world", 20);
        assert_eq!(result, vec!["hello world"]);
    }

    #[test]
    fn wrap_text_long() {
        let result = wrap_text("this is a longer text that needs wrapping", 20);
        assert!(result.len() > 1);
        for line in &result {
            assert!(line.len() <= 20);
        }
    }

    #[test]
    fn wrap_text_empty() {
        let result = wrap_text("", 20);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn wrap_text_zero_width() {
        let result = wrap_text("hello", 0);
        assert_eq!(result, vec!["hello"]);
    }
}

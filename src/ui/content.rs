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
use crate::syntax;
use crate::theme::Theme;

/// Draw the content panel with section content
pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, theme: &Theme, focused: bool) {
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

    // Reserve 1 column for scrollbar
    let content_width = inner.width.saturating_sub(2) as usize;
    let content_area =
        Rect { x: inner.x, y: inner.y, width: inner.width.saturating_sub(1), height: inner.height };
    let scrollbar_x = inner.x + inner.width.saturating_sub(1);

    // Render content blocks
    let lines = render_content_blocks(&section.content, theme, content_width);
    let total_lines = lines.len();
    let visible_height = inner.height as usize;

    // Update state with content metrics for scroll clamping
    state.content.total_lines = total_lines;
    state.content.visible_height = visible_height;

    // Clamp scroll offset
    state.content.clamp_scroll();
    let scroll_offset = state.content.scroll_offset;
    let end = (scroll_offset + visible_height).min(total_lines);
    let visible_lines: Vec<Line> =
        lines.into_iter().skip(scroll_offset).take(end - scroll_offset).collect();

    let content = Paragraph::new(visible_lines);
    frame.render_widget(content, content_area);

    // Draw scrollbar
    draw_scrollbar(frame, scrollbar_x, inner.y, inner.height, scroll_offset, total_lines, theme);
}

/// Draw a scrollbar indicator
fn draw_scrollbar(
    frame: &mut Frame,
    x: u16,
    y: u16,
    height: u16,
    scroll_offset: usize,
    total_lines: usize,
    theme: &Theme,
) {
    if total_lines == 0 || height == 0 {
        return;
    }

    let height = height as usize;

    // Calculate thumb size and position
    let visible_ratio = (height as f64 / total_lines as f64).min(1.0);
    let thumb_height = ((height as f64 * visible_ratio).ceil() as usize).max(1);

    // Calculate max scroll position to avoid division by zero
    let max_scroll = total_lines.saturating_sub(height / 2);
    let scroll_ratio = if total_lines <= height || max_scroll == 0 {
        0.0
    } else {
        scroll_offset as f64 / max_scroll as f64
    };
    let thumb_top = ((height - thumb_height) as f64 * scroll_ratio).round() as usize;

    // Draw track and thumb
    for i in 0..height {
        let ch = if i >= thumb_top && i < thumb_top + thumb_height {
            "█" // Thumb
        } else {
            "░" // Track
        };
        let style = if i >= thumb_top && i < thumb_top + thumb_height {
            Style::default().fg(theme.accent_secondary)
        } else {
            Style::default().fg(theme.bg_tertiary)
        };

        frame.render_widget(
            Paragraph::new(ch).style(style),
            Rect { x, y: y.saturating_add(i as u16), width: 1, height: 1 },
        );
    }
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
                render_unordered_list(&mut lines, items, theme, width);
            }
            ContentBlock::OrderedList(items) => {
                render_ordered_list(&mut lines, items, theme, width);
            }
            ContentBlock::Blockquote(text) => {
                render_blockquote(&mut lines, text, theme, width);
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
    let (base_style, code_color, prefix) = match level {
        1 => (
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            theme.syntax_keyword,
            "".to_string(),
        ),
        2 => (
            Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD),
            theme.syntax_function,
            "".to_string(),
        ),
        3 => (
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            theme.syntax_keyword,
            "  ".to_string(),
        ),
        4 => (
            Style::default().fg(theme.fg_secondary).add_modifier(Modifier::BOLD),
            theme.syntax_keyword,
            "    ".to_string(),
        ),
        _ => (Style::default().fg(theme.fg_muted), theme.syntax_keyword, "      ".to_string()),
    };

    // Parse heading text to style code parts differently
    let mut spans: Vec<Span<'static>> = Vec::new();
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix, base_style));
    }

    let mut in_code = false;
    let mut current = String::new();

    for c in text.chars() {
        if c == '`' {
            if !current.is_empty() {
                if in_code {
                    // Code text - use code color with bold
                    spans.push(Span::styled(
                        current.clone(),
                        Style::default().fg(code_color).add_modifier(Modifier::BOLD),
                    ));
                } else {
                    // Regular text - use base heading style
                    spans.push(Span::styled(current.clone(), base_style));
                }
                current.clear();
            }
            in_code = !in_code;
        } else {
            current.push(c);
        }
    }

    // Don't forget remaining text
    if !current.is_empty() {
        spans.push(Span::styled(current, base_style));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(spans));
    if level <= 2 {
        lines.push(Line::from(""));
    }
}

fn render_paragraph(lines: &mut Vec<Line<'static>>, text: &str, theme: &Theme, width: usize) {
    // Parse inline formatting and word-wrap
    let spans = parse_inline_formatting(text, theme);
    let wrapped_lines = wrap_spans(spans, width.saturating_sub(2));

    for line in wrapped_lines {
        lines.push(line);
    }
    lines.push(Line::from(""));
}

/// Parse inline markdown formatting into styled spans
fn parse_inline_formatting(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current = String::new();

    while let Some(c) = chars.next() {
        match c {
            '`' => {
                // Inline code
                if !current.is_empty() {
                    spans
                        .push(Span::styled(current.clone(), Style::default().fg(theme.fg_primary)));
                    current.clear();
                }
                let mut code = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '`' {
                        chars.next();
                        break;
                    }
                    code.push(chars.next().unwrap());
                }
                spans.push(Span::styled(
                    code,
                    Style::default().fg(theme.syntax_string).bg(theme.bg_secondary),
                ));
            }
            '*' | '_' => {
                // Check for bold (**) or italic (*)
                let is_double = chars.peek() == Some(&c);
                if is_double {
                    chars.next(); // consume second *
                }

                if !current.is_empty() {
                    spans
                        .push(Span::styled(current.clone(), Style::default().fg(theme.fg_primary)));
                    current.clear();
                }

                let mut content = String::new();
                let mut found_end = false;

                while let Some(next) = chars.next() {
                    if next == c {
                        if is_double && chars.peek() == Some(&c) {
                            chars.next();
                            found_end = true;
                            break;
                        } else if !is_double {
                            found_end = true;
                            break;
                        }
                    }
                    content.push(next);
                }

                if found_end {
                    let style = if is_double {
                        Style::default().fg(theme.fg_primary).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.fg_primary).add_modifier(Modifier::ITALIC)
                    };
                    spans.push(Span::styled(content, style));
                } else {
                    // Not a valid marker, treat as literal
                    current.push(c);
                    if is_double {
                        current.push(c);
                    }
                    current.push_str(&content);
                }
            }
            '[' => {
                // Link: [text](url)
                if !current.is_empty() {
                    spans
                        .push(Span::styled(current.clone(), Style::default().fg(theme.fg_primary)));
                    current.clear();
                }

                let mut link_text = String::new();
                let mut found_bracket = false;
                for next in chars.by_ref() {
                    if next == ']' {
                        found_bracket = true;
                        break;
                    }
                    link_text.push(next);
                }

                if found_bracket && chars.peek() == Some(&'(') {
                    chars.next(); // consume (
                    let mut url = String::new();
                    for next in chars.by_ref() {
                        if next == ')' {
                            break;
                        }
                        url.push(next);
                    }
                    // Show link text in accent color
                    spans.push(Span::styled(
                        link_text,
                        Style::default()
                            .fg(theme.accent_secondary)
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                } else {
                    // Not a valid link, treat as literal
                    current.push('[');
                    current.push_str(&link_text);
                    if found_bracket {
                        current.push(']');
                    }
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        spans.push(Span::styled(current, Style::default().fg(theme.fg_primary)));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}

/// Wrap styled spans into lines while preserving formatting
fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from(spans)];
    }

    let mut lines = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0;

    for span in spans {
        let text = span.content.to_string();
        let style = span.style;

        for word in text.split_inclusive(char::is_whitespace) {
            let word_len = word.chars().count();

            if current_width + word_len > width && current_width > 0 {
                // Start new line
                lines.push(Line::from(current_line.clone()));
                current_line.clear();
                current_width = 0;
            }

            current_line.push(Span::styled(word.to_string(), style));
            current_width += word_len;
        }
    }

    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    if lines.is_empty() {
        lines.push(Line::from(""));
    }

    lines
}

fn render_code_block(lines: &mut Vec<Line<'static>>, code: &crate::book::CodeBlock, theme: &Theme) {
    // Language label with box drawing
    let lang_label = code.language.as_deref().unwrap_or("code");
    lines.push(Line::from(vec![
        Span::styled("┌─ ", Style::default().fg(theme.border)),
        Span::styled(lang_label.to_string(), Style::default().fg(theme.info)),
        Span::styled(" ─".to_string(), Style::default().fg(theme.border)),
    ]));

    // Code content with syntax highlighting
    for line in code.code.lines() {
        let mut line_spans = vec![Span::styled("│ ", Style::default().fg(theme.border))];
        let highlighted_spans = syntax::highlight_line(line, code.language.as_deref(), theme);
        line_spans.extend(highlighted_spans);
        lines.push(Line::from(line_spans));
    }

    // Bottom border
    lines.push(Line::from(Span::styled("└──────", Style::default().fg(theme.border))));
    lines.push(Line::from(""));
}

/// Basic syntax highlighting for a code line (kept for backward compatibility in tests)
#[allow(dead_code)]
fn highlight_code_line(line: &str, language: Option<&str>, theme: &Theme) -> String {
    // This is now just a stub for tests - actual highlighting is done by syntax module
    let _ = (language, theme);
    line.to_string()
}

fn render_unordered_list(
    lines: &mut Vec<Line<'static>>,
    items: &[String],
    theme: &Theme,
    width: usize,
) {
    let bullet = "  • ";
    let indent = "    "; // Same width as bullet for continuation lines
    let content_width = width.saturating_sub(4); // Account for bullet/indent

    for item in items {
        let spans = parse_inline_formatting(item, theme);
        let wrapped = wrap_spans(spans, content_width);

        for (i, line) in wrapped.into_iter().enumerate() {
            let prefix = if i == 0 {
                Span::styled(bullet, Style::default().fg(theme.accent_secondary))
            } else {
                Span::raw(indent)
            };
            let mut line_spans = vec![prefix];
            line_spans.extend(line.spans);
            lines.push(Line::from(line_spans));
        }
    }
    lines.push(Line::from(""));
}

fn render_ordered_list(
    lines: &mut Vec<Line<'static>>,
    items: &[String],
    theme: &Theme,
    width: usize,
) {
    let content_width = width.saturating_sub(6); // Account for "  X. " prefix

    for (i, item) in items.iter().enumerate() {
        let prefix = format!("  {}. ", i + 1);
        let indent = "     "; // Same width for continuation lines
        let spans = parse_inline_formatting(item, theme);
        let wrapped = wrap_spans(spans, content_width);

        for (j, line) in wrapped.into_iter().enumerate() {
            let prefix_span = if j == 0 {
                Span::styled(prefix.clone(), Style::default().fg(theme.accent_secondary))
            } else {
                Span::raw(indent)
            };
            let mut line_spans = vec![prefix_span];
            line_spans.extend(line.spans);
            lines.push(Line::from(line_spans));
        }
    }
    lines.push(Line::from(""));
}

fn render_blockquote(lines: &mut Vec<Line<'static>>, text: &str, theme: &Theme, width: usize) {
    let prefix = "  │ ";
    let content_width = width.saturating_sub(4); // Account for prefix

    let spans = parse_inline_formatting(text, theme);
    // Apply muted style to all spans
    let muted_spans: Vec<Span<'static>> = spans
        .into_iter()
        .map(|s| Span::styled(s.content.to_string(), s.style.fg(theme.fg_muted)))
        .collect();
    let wrapped = wrap_spans(muted_spans, content_width);

    for line in wrapped {
        let mut line_spans = vec![Span::styled(prefix, Style::default().fg(theme.accent_primary))];
        line_spans.extend(line.spans);
        lines.push(Line::from(line_spans));
    }
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
    let num_cols = table.headers.len().max(table.rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if num_cols == 0 {
        return;
    }

    // Fixed column widths based on number of columns
    let col_widths: Vec<usize> = match num_cols {
        1 => vec![70],
        2 => vec![30, 40],
        3 => vec![20, 25, 30],
        4 => vec![10, 18, 28, 14],
        5 => vec![8, 15, 22, 15, 10],
        _ => vec![12; num_cols],
    };

    // Add spacing before table
    lines.push(Line::from(""));

    // Header row with background color
    if !table.headers.is_empty() {
        let header_bg = theme.accent_secondary;
        let mut header_spans: Vec<Span<'static>> = Vec::new();

        for (i, header) in table.headers.iter().enumerate() {
            let width = col_widths.get(i).copied().unwrap_or(15);
            // Strip backticks from headers
            let clean_header = header.replace('`', "");
            let padded = pad_or_truncate(&clean_header, width);

            // Add spacing between columns
            if i > 0 {
                header_spans.push(Span::styled("  ", Style::default().bg(header_bg)));
            }

            header_spans.push(Span::styled(
                padded,
                Style::default()
                    .fg(theme.bg_primary) // Dark text on light background
                    .bg(header_bg)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Pad to fill the row
        header_spans.push(Span::styled("  ", Style::default().bg(header_bg)));
        lines.push(Line::from(header_spans));
    }

    // Data rows with alternating backgrounds
    for (row_idx, row) in table.rows.iter().enumerate() {
        let row_bg = if row_idx % 2 == 0 { theme.bg_secondary } else { theme.bg_primary };

        // Calculate wrapped lines for each cell
        let mut wrapped_cells: Vec<Vec<String>> = Vec::new();
        let mut max_lines = 1;

        for (i, cell) in row.iter().enumerate() {
            let width = col_widths.get(i).copied().unwrap_or(15);
            // Strip backticks from cell content
            let clean_cell = cell.replace('`', "");
            let wrapped = wrap_cell_text(&clean_cell, width);
            max_lines = max_lines.max(wrapped.len());
            wrapped_cells.push(wrapped);
        }

        // Pad missing cells
        while wrapped_cells.len() < num_cols {
            wrapped_cells.push(vec![String::new()]);
        }

        // Render each line of the row
        for line_idx in 0..max_lines {
            let mut row_spans: Vec<Span<'static>> = Vec::new();

            for (col_idx, wrapped) in wrapped_cells.iter().enumerate() {
                let width = col_widths.get(col_idx).copied().unwrap_or(15);
                let cell_line = wrapped.get(line_idx).map(|s| s.as_str()).unwrap_or("");
                let padded = pad_or_truncate(cell_line, width);

                // Add spacing between columns
                if col_idx > 0 {
                    row_spans.push(Span::styled("  ", Style::default().bg(row_bg)));
                }

                // Style based on column (first column often has code-like content)
                let style = if col_idx == 0 {
                    Style::default().fg(theme.syntax_keyword).bg(row_bg)
                } else if col_idx == num_cols - 1 && !cell_line.is_empty() {
                    // Last column often has trait names (code)
                    Style::default().fg(theme.accent_primary).bg(row_bg)
                } else {
                    Style::default().fg(theme.fg_primary).bg(row_bg)
                };

                row_spans.push(Span::styled(padded, style));
            }

            // Pad to fill the row
            row_spans.push(Span::styled("  ", Style::default().bg(row_bg)));
            lines.push(Line::from(row_spans));
        }
    }

    lines.push(Line::from(""));
}

/// Pad string to width or truncate with ellipsis
fn pad_or_truncate(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count > width {
        let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        format!("{:width$}", s, width = width)
    }
}

/// Wrap text to fit within a given width
fn wrap_cell_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        let current_len = current_line.chars().count();

        if current_line.is_empty() {
            // First word on line - might need to truncate if too long
            if word_len > width {
                current_line = word.chars().take(width).collect();
            } else {
                current_line = word.to_string();
            }
        } else if current_len + 1 + word_len <= width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            // Start new line
            result.push(current_line);
            if word_len > width {
                current_line = word.chars().take(width).collect();
            } else {
                current_line = word.to_string();
            }
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

/// Simple word-wrapping function (used in tests)
#[allow(dead_code)]
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::default();

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
        result.push(String::default());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

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

    #[test]
    fn pad_or_truncate_short() {
        let result = pad_or_truncate("hi", 5);
        assert_eq!(result, "hi   ");
    }

    #[test]
    fn pad_or_truncate_exact() {
        let result = pad_or_truncate("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn pad_or_truncate_long() {
        let result = pad_or_truncate("hello world", 5);
        assert_eq!(result, "hell…");
    }

    #[test]
    fn wrap_cell_text_short() {
        let result = wrap_cell_text("short", 10);
        assert_eq!(result, vec!["short"]);
    }

    #[test]
    fn wrap_cell_text_exact() {
        let result = wrap_cell_text("exactly 10", 10);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn wrap_cell_text_long() {
        let result = wrap_cell_text("this is a very long text", 10);
        assert!(result.len() > 1);
    }

    #[test]
    fn wrap_cell_text_empty() {
        let result = wrap_cell_text("", 10);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn parse_inline_code() {
        let theme = Theme::default();
        let spans = parse_inline_formatting("hello `code` world", &theme);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "hello ");
        assert_eq!(spans[1].content, "code");
        assert_eq!(spans[2].content, " world");
    }

    #[test]
    fn parse_inline_bold() {
        let theme = Theme::default();
        let spans = parse_inline_formatting("hello **bold** world", &theme);
        // Should have 3 parts: "hello ", "bold", " world"
        assert!(spans.len() >= 3);
    }

    #[test]
    fn parse_inline_italic() {
        let theme = Theme::default();
        let spans = parse_inline_formatting("hello *italic* world", &theme);
        assert!(spans.len() >= 3);
    }

    #[test]
    fn parse_inline_link() {
        let theme = Theme::default();
        let spans = parse_inline_formatting("click [here](http://example.com) now", &theme);
        // Should contain the link text "here"
        let has_link = spans.iter().any(|s| s.content.contains("here"));
        assert!(has_link);
    }

    #[test]
    fn parse_inline_plain() {
        let theme = Theme::default();
        let spans = parse_inline_formatting("plain text only", &theme);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "plain text only");
    }

    #[test]
    fn render_content_blocks_empty() {
        let theme = Theme::default();
        let lines = render_content_blocks(&[], &theme, 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn render_content_blocks_paragraph() {
        use crate::book::ContentBlock;
        let theme = Theme::default();
        let blocks = vec![ContentBlock::Paragraph("Hello world".into())];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_heading() {
        use crate::book::ContentBlock;
        let theme = Theme::default();
        let blocks = vec![ContentBlock::Heading { level: 1, text: "Title".into() }];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_code() {
        use crate::book::{CodeBlock, ContentBlock};
        let theme = Theme::default();
        let blocks = vec![ContentBlock::Code(CodeBlock {
            language: Some("rust".into()),
            code: "fn main() {}".into(),
            filename: None,
            highlight_lines: Vec::new(),
        })];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_list() {
        use crate::book::ContentBlock;
        let theme = Theme::default();
        let blocks = vec![ContentBlock::UnorderedList(vec!["Item 1".into(), "Item 2".into()])];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_ordered_list() {
        use crate::book::ContentBlock;
        let theme = Theme::default();
        let blocks = vec![ContentBlock::OrderedList(vec!["First".into(), "Second".into()])];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_blockquote() {
        use crate::book::ContentBlock;
        let theme = Theme::default();
        let blocks = vec![ContentBlock::Blockquote("Quote text".into())];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_horizontal_rule() {
        use crate::book::ContentBlock;
        let theme = Theme::default();
        let blocks = vec![ContentBlock::HorizontalRule];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_image() {
        use crate::book::ContentBlock;
        let theme = Theme::default();
        let blocks = vec![ContentBlock::Image {
            alt: "Alt text".into(),
            src: "http://example.com/img.png".into(),
        }];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn render_content_blocks_table() {
        use crate::book::{Alignment, ContentBlock, Table};
        let theme = Theme::default();
        let blocks = vec![ContentBlock::Table(Table {
            headers: vec!["Col1".into(), "Col2".into()],
            rows: vec![vec!["A".into(), "B".into()]],
            alignments: vec![Alignment::Left, Alignment::Left],
        })];
        let lines = render_content_blocks(&blocks, &theme, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn highlight_code_line_rust() {
        let theme = Theme::default();
        let result = highlight_code_line("let x = 5;", Some("rust"), &theme);
        assert!(!result.is_empty());
    }

    #[test]
    fn highlight_code_line_unknown() {
        let theme = Theme::default();
        let result = highlight_code_line("some code", Some("unknown_lang"), &theme);
        assert!(!result.is_empty());
    }

    #[test]
    fn highlight_code_line_none() {
        let theme = Theme::default();
        let result = highlight_code_line("plain code", None, &theme);
        assert!(!result.is_empty());
    }
}

//! Curriculum tree browser component

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use textwrap::{Options, wrap};

use crate::app::state::AppState;
use crate::config::progress::Progress;
use crate::theme::Theme;

/// Status indicators for sections
const STATUS_NOT_STARTED: &str = "○";
const STATUS_IN_PROGRESS: &str = "●";
const STATUS_COMPLETED: &str = "✓";

/// Draw the curriculum tree browser
pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, theme: &Theme, focused: bool) {
    draw_with_progress(frame, area, state, theme, focused, None);
}

/// Draw the curriculum tree browser with optional progress data
pub fn draw_with_progress(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
    focused: bool,
    progress: Option<&Progress>,
) {
    let border_color = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .title(" Curriculum ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg_primary));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Update visible height for scroll calculations
    state.curriculum.visible_height = inner.height as usize;

    // If no book loaded, show message
    let Some(book) = &state.book else {
        let msg = Paragraph::new("No book loaded\n\nAdd a book with:\nsensei add <path>")
            .style(Style::default().fg(theme.fg_muted));
        frame.render_widget(msg, inner);
        return;
    };

    let width = inner.width as usize;

    // Build curriculum tree with text wrapping
    // Track the starting line index for each item so we can scroll to keep selection visible
    let mut lines: Vec<Line> = Vec::new();
    let mut item_line_starts: Vec<usize> = Vec::new(); // Line index where each item starts
    let mut flat_index = 0;

    for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
        let is_expanded = state.curriculum.expanded_chapters.contains(&chapter_idx);
        let expand_icon = if is_expanded { "▼" } else { "▶" };

        // Check if this chapter row is selected
        let is_chapter_selected = flat_index == state.curriculum.selected_index;

        // Chapter prefix and text - only show number if chapter is numbered
        let prefix = match chapter.number {
            Some(num) => format!("{} {}. ", expand_icon, num),
            None => format!("{} ", expand_icon),
        };
        let chapter_style = if is_chapter_selected && focused {
            Style::default()
                .fg(theme.bg_primary)
                .bg(theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg_primary)
        };

        // Track where this chapter starts in the lines list
        item_line_starts.push(lines.len());

        // Wrap the chapter title and apply inline code styling to each line
        // When selected, use a readable color for code; otherwise use accent color
        let code_style = if is_chapter_selected && focused {
            chapter_style.fg(theme.syntax_string) // Readable on selection background
        } else {
            Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD)
        };
        let wrapped_lines = wrap_with_indent(&chapter.title, width, prefix.len());

        let mut in_code = false;
        for (i, line_text) in wrapped_lines.iter().enumerate() {
            let mut line_spans = if i == 0 {
                vec![Span::styled(prefix.clone(), chapter_style)]
            } else {
                let indent = " ".repeat(prefix.len());
                vec![Span::styled(indent, chapter_style)]
            };
            let (spans, new_in_code) = parse_inline_code_spans_with_state(line_text, chapter_style, code_style, in_code);
            line_spans.extend(spans);
            in_code = new_in_code;
            lines.push(Line::from(line_spans));
        }
        flat_index += 1;

        // Sections (if expanded)
        if is_expanded {
            for (section_idx, section) in chapter.sections.iter().enumerate() {
                let is_section_selected = flat_index == state.curriculum.selected_index;

                // Get status from progress if available
                let status = get_section_status(progress, &book.metadata.id, &section.path);

                // Section prefix with indent
                // - For numbered chapters: show "1.1", "1.2", etc. (skip ".0" for chapter intro)
                // - For unnumbered chapters: just show the status indicator
                let section_prefix = match (chapter.number, section.number) {
                    (Some(ch_num), 0) => format!("   {} {}.  ", status, ch_num), // Chapter intro
                    (Some(ch_num), sec_num) => format!("   {} {}.{} ", status, ch_num, sec_num),
                    (None, _) => format!("   {} ", status),
                };

                let section_style = if is_section_selected && focused {
                    Style::default()
                        .fg(theme.bg_primary)
                        .bg(theme.accent_primary)
                        .add_modifier(Modifier::BOLD)
                } else if state.current_chapter == chapter_idx
                    && state.current_section == section_idx
                {
                    // Currently viewed section (but not selected in tree)
                    Style::default().fg(theme.accent_secondary)
                } else {
                    Style::default().fg(theme.fg_secondary)
                };

                // Track where this section starts in the lines list
                item_line_starts.push(lines.len());

                // Wrap the section title and apply inline code styling to each line
                // When selected, use a readable color for code; otherwise use accent color
                let code_style = if is_section_selected && focused {
                    section_style.fg(theme.syntax_string) // Readable on selection background
                } else {
                    Style::default().fg(theme.accent_primary).add_modifier(Modifier::BOLD)
                };
                let wrapped_lines = wrap_with_indent(&section.title, width, section_prefix.len());

                let mut in_code = false;
                for (i, line_text) in wrapped_lines.iter().enumerate() {
                    let mut line_spans = if i == 0 {
                        vec![Span::styled(section_prefix.clone(), section_style)]
                    } else {
                        let indent = " ".repeat(section_prefix.len());
                        vec![Span::styled(indent, section_style)]
                    };
                    let (spans, new_in_code) = parse_inline_code_spans_with_state(line_text, section_style, code_style, in_code);
                    line_spans.extend(spans);
                    in_code = new_in_code;
                    lines.push(Line::from(line_spans));
                }
                flat_index += 1;
            }
        }
    }

    // Handle scroll offset - ensure selected item is visible
    let visible_height = inner.height as usize;

    // Find the line position of the selected item
    if let Some(&selected_line) = item_line_starts.get(state.curriculum.selected_index) {
        // Calculate how many lines this item takes (until next item or end)
        let next_item_line = item_line_starts
            .get(state.curriculum.selected_index + 1)
            .copied()
            .unwrap_or(lines.len());
        let item_height = next_item_line - selected_line;

        // Scroll up if selected item is above visible area
        if selected_line < state.curriculum.scroll_offset {
            state.curriculum.scroll_offset = selected_line;
        }
        // Scroll down if selected item is below visible area
        else if selected_line + item_height > state.curriculum.scroll_offset + visible_height {
            state.curriculum.scroll_offset = (selected_line + item_height).saturating_sub(visible_height);
        }
    }

    let start = state.curriculum.scroll_offset;
    let end = (start + visible_height).min(lines.len());
    let visible_lines: Vec<Line> = lines.into_iter().skip(start).take(end - start).collect();

    let curriculum = Paragraph::new(visible_lines);
    frame.render_widget(curriculum, inner);
}

/// Parse text with backticks and return styled spans (for curriculum display)
/// Also returns whether we ended inside a code block (for multi-line handling)
fn parse_inline_code_spans_with_state(
    text: &str,
    base_style: Style,
    code_style: Style,
    start_in_code: bool,
) -> (Vec<Span<'static>>, bool) {
    let mut spans = Vec::new();
    let mut in_code = start_in_code;
    let mut current = String::new();

    for c in text.chars() {
        if c == '`' {
            if !current.is_empty() {
                let style = if in_code { code_style } else { base_style };
                spans.push(Span::styled(current.clone(), style));
                current.clear();
            }
            in_code = !in_code;
        } else {
            current.push(c);
        }
    }

    // Don't forget remaining text
    if !current.is_empty() {
        let style = if in_code { code_style } else { base_style };
        spans.push(Span::styled(current, style));
    }

    (spans, in_code)
}

/// Wrap text with a given indent for continuation lines
fn wrap_with_indent(text: &str, width: usize, indent: usize) -> Vec<String> {
    if width <= indent {
        return vec![text.to_string()];
    }

    let content_width = width.saturating_sub(indent);
    if content_width == 0 {
        return vec![text.to_string()];
    }

    let options = Options::new(content_width);
    wrap(text, options).into_iter().map(|s| s.to_string()).collect()
}

/// Get the status indicator for a section based on progress
fn get_section_status(
    progress: Option<&Progress>,
    book_id: &str,
    section_path: &str,
) -> &'static str {
    let Some(progress) = progress else {
        return STATUS_NOT_STARTED;
    };

    let Some(book_progress) = progress.books.get(book_id) else {
        return STATUS_NOT_STARTED;
    };

    let Some(section_progress) = book_progress.sections.get(section_path) else {
        return STATUS_NOT_STARTED;
    };

    if section_progress.completed {
        STATUS_COMPLETED
    } else if section_progress.viewed {
        STATUS_IN_PROGRESS
    } else {
        STATUS_NOT_STARTED
    }
}

/// Calculate total visible items in curriculum
pub fn calculate_visible_items(state: &AppState) -> usize {
    let Some(book) = &state.book else { return 0 };

    let mut count = 0;
    for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
        count += 1; // Chapter itself
        if state.curriculum.expanded_chapters.contains(&chapter_idx) {
            count += chapter.sections.len();
        }
    }
    count
}

/// Get the chapter/section at a given flat index
pub fn get_item_at_index(state: &AppState, target_index: usize) -> Option<CurriculumItem> {
    let book = state.book.as_ref()?;

    let mut current_idx = 0;
    for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
        if current_idx == target_index {
            return Some(CurriculumItem::Chapter(chapter_idx));
        }
        current_idx += 1;

        if state.curriculum.expanded_chapters.contains(&chapter_idx) {
            for (section_idx, _section) in chapter.sections.iter().enumerate() {
                if current_idx == target_index {
                    return Some(CurriculumItem::Section(chapter_idx, section_idx));
                }
                current_idx += 1;
            }
        }
    }
    None
}

/// Represents an item in the curriculum tree
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurriculumItem {
    Chapter(usize),
    Section(usize, usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{Book, BookMetadata, BookSource, Chapter, Section};
    use std::path::PathBuf;

    fn create_test_book() -> Book {
        let mut book = Book::new(BookMetadata {
            id: "test".into(),
            title: "Test Book".into(),
            author: None,
            source: BookSource::Markdown(PathBuf::from("/test")),
            language: None,
            description: None,
            cover_image: None,
            added_at: 0,
            last_accessed: None,
        });

        let mut ch1 = Chapter::new("Getting Started", 1, "ch01");
        ch1.sections.push(Section::new("Installation", 1, "ch01/s01"));
        ch1.sections.push(Section::new("Hello World", 2, "ch01/s02"));
        book.chapters.push(ch1);

        let mut ch2 = Chapter::new("Basics", 2, "ch02");
        ch2.sections.push(Section::new("Variables", 1, "ch02/s01"));
        book.chapters.push(ch2);

        book
    }

    #[test]
    fn calculate_items_with_no_book() {
        let state = AppState::default();
        assert_eq!(calculate_visible_items(&state), 0);
    }

    #[test]
    fn calculate_items_collapsed() {
        let state = AppState { book: Some(create_test_book()), ..Default::default() };

        // With no chapters expanded, only chapter headers are visible
        assert_eq!(calculate_visible_items(&state), 2);
    }

    #[test]
    fn calculate_items_expanded() {
        let mut state = AppState { book: Some(create_test_book()), ..Default::default() };
        state.curriculum.expanded_chapters.insert(0);

        // Chapter 1 expanded (2 sections) + Chapter 2 collapsed = 1 + 2 + 1 = 4
        assert_eq!(calculate_visible_items(&state), 4);
    }

    #[test]
    fn get_item_at_index_chapter() {
        let state = AppState { book: Some(create_test_book()), ..Default::default() };

        assert_eq!(get_item_at_index(&state, 0), Some(CurriculumItem::Chapter(0)));
        assert_eq!(get_item_at_index(&state, 1), Some(CurriculumItem::Chapter(1)));
    }

    #[test]
    fn get_item_at_index_section() {
        let mut state = AppState { book: Some(create_test_book()), ..Default::default() };
        state.curriculum.expanded_chapters.insert(0);

        assert_eq!(get_item_at_index(&state, 0), Some(CurriculumItem::Chapter(0)));
        assert_eq!(get_item_at_index(&state, 1), Some(CurriculumItem::Section(0, 0)));
        assert_eq!(get_item_at_index(&state, 2), Some(CurriculumItem::Section(0, 1)));
        assert_eq!(get_item_at_index(&state, 3), Some(CurriculumItem::Chapter(1)));
    }

    #[test]
    fn status_not_started_without_progress() {
        let status = get_section_status(None, "book", "ch01/s01");
        assert_eq!(status, STATUS_NOT_STARTED);
    }
}

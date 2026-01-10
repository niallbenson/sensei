//! Curriculum tree browser component

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

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
            .style(Style::default().fg(theme.fg_muted))
            .wrap(Wrap { trim: true });
        frame.render_widget(msg, inner);
        return;
    };

    // Build curriculum tree
    let mut lines: Vec<Line> = Vec::new();
    let mut flat_index = 0;

    for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
        let is_expanded = state.curriculum.expanded_chapters.contains(&chapter_idx);
        let expand_icon = if is_expanded { "▼" } else { "▶" };

        // Check if this chapter row is selected
        let is_chapter_selected = flat_index == state.curriculum.selected_index;

        // Chapter line
        let chapter_text = format!("{} {}. {}", expand_icon, chapter.number, chapter.title);
        let chapter_style = if is_chapter_selected && focused {
            Style::default()
                .fg(theme.bg_primary)
                .bg(theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg_primary)
        };
        lines.push(Line::from(Span::styled(chapter_text, chapter_style)));
        flat_index += 1;

        // Sections (if expanded)
        if is_expanded {
            for (section_idx, section) in chapter.sections.iter().enumerate() {
                let is_section_selected = flat_index == state.curriculum.selected_index;

                // Get status from progress if available
                let status = get_section_status(progress, &book.metadata.id, &section.path);

                let section_text = format!(
                    "   {} {}.{} {}",
                    status, chapter.number, section.number, section.title
                );

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

                lines.push(Line::from(Span::styled(section_text, section_style)));
                flat_index += 1;
            }
        }
    }

    // Handle scroll offset
    let visible_height = inner.height as usize;
    let start = state.curriculum.scroll_offset;
    let end = (start + visible_height).min(lines.len());
    let visible_lines: Vec<Line> = lines.into_iter().skip(start).take(end - start).collect();

    let curriculum = Paragraph::new(visible_lines);
    frame.render_widget(curriculum, inner);
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

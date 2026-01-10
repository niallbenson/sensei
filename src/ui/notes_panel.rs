//! Notes panel component

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use textwrap::{Options, wrap};

use crate::app::state::AppState;
use crate::notes::{Note, NotesStore};
use crate::theme::Theme;

/// Draw the notes panel
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
    focused: bool,
    notes_store: &NotesStore,
) {
    let border_color = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .title(" Notes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg_primary));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Update visible height for scroll calculations
    state.notes.visible_height = inner.height as usize;

    let width = inner.width as usize;

    // Get current section path from book if loaded
    let (book_id, section_path) = match &state.book {
        Some(book) => {
            let chapter = book.chapters.get(state.current_chapter);
            let section = chapter.and_then(|ch| ch.sections.get(state.current_section));
            match section {
                Some(s) => (book.metadata.id.as_str(), s.path.as_str()),
                None => {
                    draw_empty_message(frame, inner, theme, "No section selected");
                    return;
                }
            }
        }
        None => {
            draw_empty_message(frame, inner, theme, "No book loaded");
            return;
        }
    };

    // Get notes for current section
    let section_notes = notes_store.get_section_level_notes(book_id, section_path);
    let selection_notes = notes_store.get_selection_notes(book_id, section_path);

    // If we're creating a new note, show the input area
    if state.notes.creating {
        draw_note_input(frame, inner, state, theme, width, "New Note");
        return;
    }

    // If we're editing a note, show the edit area
    if let Some(note_id) = &state.notes.editing {
        let note_title = notes_store
            .get_note(note_id)
            .map(|n| if n.is_section_note() { "Edit Note" } else { "Edit Annotation" })
            .unwrap_or("Edit Note");
        draw_note_input(frame, inner, state, theme, width, note_title);
        return;
    }

    // Build notes list
    let mut lines: Vec<Line> = Vec::new();
    let mut note_indices: Vec<&Note> = Vec::new();

    // Section-level notes first
    if !section_notes.is_empty() {
        lines.push(Line::from(Span::styled(
            "─ Section Notes ─",
            Style::default().fg(theme.fg_muted).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for (idx, note) in section_notes.iter().enumerate() {
            let is_selected = note_indices.len() == state.notes.selected_index && focused;
            add_note_lines(&mut lines, note, is_selected, theme, width, idx);
            note_indices.push(note);
        }
    }

    // Text-selection notes (annotations)
    if !selection_notes.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            "─ Annotations ─",
            Style::default().fg(theme.fg_muted).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for (idx, note) in selection_notes.iter().enumerate() {
            let note_list_idx = section_notes.len() + idx;
            let is_selected = note_list_idx == state.notes.selected_index && focused;
            add_note_lines(&mut lines, note, is_selected, theme, width, idx);
            note_indices.push(note);
        }
    }

    if lines.is_empty() {
        draw_empty_message(
            frame,
            inner,
            theme,
            "No notes yet\n\nPress 'n' to add a note\nSelect text with 'v' to annotate",
        );
        return;
    }

    // Handle scroll offset
    let visible_height = inner.height as usize;
    let start = state.notes.scroll_offset;
    let end = (start + visible_height).min(lines.len());
    let visible_lines: Vec<Line> = lines.into_iter().skip(start).take(end - start).collect();

    let notes_widget = Paragraph::new(visible_lines).wrap(Wrap { trim: false });
    frame.render_widget(notes_widget, inner);
}

/// Draw an empty message centered in the area
fn draw_empty_message(frame: &mut Frame, area: Rect, theme: &Theme, msg: &str) {
    let msg_widget =
        Paragraph::new(msg).style(Style::default().fg(theme.fg_muted)).wrap(Wrap { trim: true });
    frame.render_widget(msg_widget, area);
}

/// Draw the note input area for creating/editing
fn draw_note_input(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    theme: &Theme,
    width: usize,
    title: &str,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        format!("─ {} ─", title),
        Style::default().fg(theme.accent_primary).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Input content with cursor
    let input = &state.notes.input;
    let cursor_pos = state.notes.cursor;

    // Wrap input text
    let wrapped = if width > 2 {
        let opts = Options::new(width - 2);
        wrap(input, opts).into_iter().map(|s| s.to_string()).collect::<Vec<_>>()
    } else {
        vec![input.clone()]
    };

    if wrapped.is_empty() || (wrapped.len() == 1 && wrapped[0].is_empty()) {
        // Empty input - show cursor
        lines.push(Line::from(Span::styled(
            "│",
            Style::default().fg(theme.accent_primary).add_modifier(Modifier::SLOW_BLINK),
        )));
    } else {
        // Show input with cursor indicator
        let mut char_count = 0;
        for line_text in &wrapped {
            let line_chars: Vec<char> = line_text.chars().collect();
            let mut spans = Vec::new();

            // Check if cursor is on this line
            if cursor_pos >= char_count && cursor_pos <= char_count + line_chars.len() {
                let cursor_in_line = cursor_pos - char_count;
                let before: String = line_chars[..cursor_in_line].iter().collect();
                let after: String = line_chars[cursor_in_line..].iter().collect();

                if !before.is_empty() {
                    spans.push(Span::styled(before, Style::default().fg(theme.fg_primary)));
                }
                spans.push(Span::styled(
                    "│",
                    Style::default().fg(theme.accent_primary).add_modifier(Modifier::SLOW_BLINK),
                ));
                if !after.is_empty() {
                    spans.push(Span::styled(after, Style::default().fg(theme.fg_primary)));
                }
            } else {
                spans.push(Span::styled(line_text.clone(), Style::default().fg(theme.fg_primary)));
            }

            lines.push(Line::from(spans));
            char_count += line_chars.len();
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter to save, Esc to cancel",
        Style::default().fg(theme.fg_muted),
    )));

    let widget = Paragraph::new(lines);
    frame.render_widget(widget, area);
}

/// Add lines for a single note to the display
fn add_note_lines(
    lines: &mut Vec<Line>,
    note: &Note,
    is_selected: bool,
    theme: &Theme,
    width: usize,
    _idx: usize,
) {
    let base_style = if is_selected {
        Style::default().fg(theme.bg_primary).bg(theme.accent_primary)
    } else {
        Style::default().fg(theme.fg_primary)
    };

    let muted_style = if is_selected {
        Style::default().fg(theme.bg_primary).bg(theme.accent_primary)
    } else {
        Style::default().fg(theme.fg_muted)
    };

    // For selection notes, show the selected text first
    if let Some(selected_text) = note.anchor.selected_text() {
        let quote = format!("\"{}\"", truncate_str(selected_text, width.saturating_sub(4)));
        lines.push(Line::from(Span::styled(quote, muted_style.add_modifier(Modifier::ITALIC))));
    }

    // Note content (wrapped)
    let wrapped = if width > 2 {
        let opts = Options::new(width - 2);
        wrap(&note.content, opts).into_iter().map(|s| s.to_string()).collect::<Vec<_>>()
    } else {
        vec![note.content.clone()]
    };

    for line_text in wrapped {
        lines.push(Line::from(Span::styled(format!("  {}", line_text), base_style)));
    }

    // Timestamp
    let timestamp = format_timestamp(note.created_at);
    lines.push(Line::from(Span::styled(format!("  {}", timestamp), muted_style)));

    lines.push(Line::from(""));
}

/// Truncate a string to a maximum length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Format a Unix timestamp as a relative time or date
fn format_timestamp(timestamp: i64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let note_time = UNIX_EPOCH + Duration::from_secs(timestamp as u64);
    let now = SystemTime::now();

    let elapsed = now.duration_since(note_time).unwrap_or(Duration::ZERO);

    if elapsed.as_secs() < 60 {
        "just now".to_string()
    } else if elapsed.as_secs() < 3600 {
        let mins = elapsed.as_secs() / 60;
        format!("{}m ago", mins)
    } else if elapsed.as_secs() < 86400 {
        let hours = elapsed.as_secs() / 3600;
        format!("{}h ago", hours)
    } else if elapsed.as_secs() < 604800 {
        let days = elapsed.as_secs() / 86400;
        format!("{}d ago", days)
    } else {
        // More than a week, show date
        let days_since_epoch = timestamp / 86400;
        // Simple date calculation (approximate)
        let years = 1970 + (days_since_epoch / 365);
        let remaining_days = days_since_epoch % 365;
        let month = remaining_days / 30 + 1;
        let day = remaining_days % 30 + 1;
        format!("{:04}-{:02}-{:02}", years, month, day)
    }
}

/// Get the note at the current selection index
pub fn get_selected_note<'a>(state: &AppState, notes_store: &'a NotesStore) -> Option<&'a Note> {
    let book = state.book.as_ref()?;
    let chapter = book.chapters.get(state.current_chapter)?;
    let section = chapter.sections.get(state.current_section)?;

    let book_id = &book.metadata.id;
    let section_path = &section.path;

    let section_notes = notes_store.get_section_level_notes(book_id, section_path);
    let selection_notes = notes_store.get_selection_notes(book_id, section_path);

    let idx = state.notes.selected_index;
    if idx < section_notes.len() {
        section_notes.get(idx).copied()
    } else {
        selection_notes.get(idx - section_notes.len()).copied()
    }
}

/// Get total note count for current section
pub fn get_note_count(state: &AppState, notes_store: &NotesStore) -> usize {
    let Some(book) = &state.book else { return 0 };
    let Some(chapter) = book.chapters.get(state.current_chapter) else { return 0 };
    let Some(section) = chapter.sections.get(state.current_section) else { return 0 };

    let section_notes = notes_store.get_section_level_notes(&book.metadata.id, &section.path);
    let selection_notes = notes_store.get_selection_notes(&book.metadata.id, &section.path);

    section_notes.len() + selection_notes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_long() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_str_very_short_max() {
        assert_eq!(truncate_str("hello", 2), "...");
    }

    #[test]
    fn format_timestamp_just_now() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        assert_eq!(format_timestamp(now), "just now");
    }

    #[test]
    fn format_timestamp_minutes_ago() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let five_mins_ago = now - 300;
        assert_eq!(format_timestamp(five_mins_ago), "5m ago");
    }

    #[test]
    fn format_timestamp_hours_ago() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let two_hours_ago = now - 7200;
        assert_eq!(format_timestamp(two_hours_ago), "2h ago");
    }
}

//! Content block renderer

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::state::{AppState, VisualModeState};
use crate::book::ContentBlock;
use crate::notes::{NoteAnchor, NotesStore};
use crate::syntax;
use crate::theme::Theme;

use super::image::ImageCache;
use super::section_footer;

/// Draw the content panel with section content
pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, theme: &Theme, focused: bool) {
    draw_with_notes(frame, area, state, theme, focused, None);
}

/// Draw the content panel with optional notes for underline highlighting
pub fn draw_with_notes(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
    focused: bool,
    notes_store: Option<&NotesStore>,
) {
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

    // Get note anchors for this section
    let note_anchors: Vec<&NoteAnchor> = notes_store
        .map(|store| store.get_note_anchors(&book.metadata.id, &section.path))
        .unwrap_or_default();

    // Get visual mode state for selection highlighting
    let visual_mode = if state.visual_mode.active { Some(&state.visual_mode) } else { None };

    // Get cursor state for cursor rendering
    let cursor_state = if state.content.cursor_mode {
        Some(CursorState {
            cursor_block: state.content.cursor_block,
            cursor_char: state.content.cursor_char,
            cursor_mode: state.content.cursor_mode,
            selection_active: state.visual_mode.active,
        })
    } else {
        None
    };

    // Reserve 1 column for scrollbar
    let content_width = inner.width.saturating_sub(2) as usize;
    let content_area =
        Rect { x: inner.x, y: inner.y, width: inner.width.saturating_sub(1), height: inner.height };
    let scrollbar_x = inner.x + inner.width.saturating_sub(1);

    // Render content blocks with note underlining and selection highlighting
    // (no image heights for non-image rendering path)
    let empty_heights = std::collections::HashMap::new();
    let (lines, block_offsets) = render_content_blocks_with_offsets(
        &section.content,
        theme,
        content_width,
        &note_anchors,
        visual_mode,
        cursor_state.as_ref(),
        &empty_heights,
    );
    let total_lines = lines.len();
    let visible_height = inner.height as usize;

    // Update state with content metrics for scroll clamping
    state.content.total_lines = total_lines;
    state.content.visible_height = visible_height;
    state.content.block_line_offsets = block_offsets;
    state.content.content_width = content_width;
    state.content.content_area =
        (content_area.x, content_area.y, content_area.width, content_area.height);

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

/// Draw the content panel with images and notes support
pub fn draw_with_images(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
    focused: bool,
    notes_store: Option<&NotesStore>,
    image_cache: &mut ImageCache,
) {
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

    // Get note anchors for this section
    let note_anchors: Vec<&NoteAnchor> = notes_store
        .map(|store| store.get_note_anchors(&book.metadata.id, &section.path))
        .unwrap_or_default();

    // Get visual mode state for selection highlighting
    let visual_mode = if state.visual_mode.active { Some(&state.visual_mode) } else { None };

    // Get cursor state for cursor rendering
    let cursor_state = if state.content.cursor_mode {
        Some(CursorState {
            cursor_block: state.content.cursor_block,
            cursor_char: state.content.cursor_char,
            cursor_mode: state.content.cursor_mode,
            selection_active: state.visual_mode.active,
        })
    } else {
        None
    };

    // Reserve 1 column for scrollbar
    let content_width = inner.width.saturating_sub(2) as usize;
    let content_area =
        Rect { x: inner.x, y: inner.y, width: inner.width.saturating_sub(1), height: inner.height };
    let scrollbar_x = inner.x + inner.width.saturating_sub(1);

    // Collect image info for later rendering, calculating dynamic heights
    let mut image_info: Vec<ImageRenderInfo> = Vec::new();
    let mut image_heights: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for (block_idx, content_block) in section.content.iter().enumerate() {
        if let ContentBlock::Image { src, .. } = content_block {
            // Calculate dynamic height based on image aspect ratio
            let height = image_cache
                .recommended_rows(src, content_width as u16)
                .unwrap_or(IMAGE_RESERVED_HEIGHT);
            image_info.push(ImageRenderInfo { block_index: block_idx, src: src.clone(), height });
            image_heights.insert(block_idx, height);
        }
    }

    // Render content blocks with note underlining and selection highlighting
    let (mut lines, block_offsets) = render_content_blocks_with_offsets(
        &section.content,
        theme,
        content_width,
        &note_anchors,
        visual_mode,
        cursor_state.as_ref(),
        &image_heights,
    );

    // Add blank lines before footer for spacing
    let footer_start_line = lines.len();
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Add footer height to total lines (footer renders separately but affects scroll)
    let footer_height = section_footer::FOOTER_HEIGHT as usize;
    let total_lines = lines.len() + footer_height;
    let visible_height = inner.height as usize;

    // Update state with content metrics for scroll clamping
    state.content.total_lines = total_lines;
    state.content.visible_height = visible_height;
    state.content.block_line_offsets = block_offsets.clone();
    state.content.content_width = content_width;
    state.content.content_area =
        (content_area.x, content_area.y, content_area.width, content_area.height);

    // Clamp scroll offset
    state.content.clamp_scroll();
    let scroll_offset = state.content.scroll_offset;
    let end = (scroll_offset + visible_height).min(total_lines);
    let visible_lines: Vec<Line> =
        lines.into_iter().skip(scroll_offset).take(end - scroll_offset).collect();

    let content = Paragraph::new(visible_lines);
    frame.render_widget(content, content_area);

    // Render images at their positions
    for img_info in image_info {
        // Get the line offset for this image block
        if let Some(&line_offset) = block_offsets.get(img_info.block_index) {
            let image_height = img_info.height;
            let image_end = line_offset + image_height;
            let viewport_end = scroll_offset + visible_height;

            // Check if any part of the image is visible
            if image_end > scroll_offset && line_offset < viewport_end {
                if line_offset >= scroll_offset {
                    // Image starts within viewport - may clip at bottom
                    let relative_y = (line_offset - scroll_offset) as u16;
                    let available_height = inner.height.saturating_sub(relative_y);

                    if available_height >= 1 {
                        let render_height = available_height.min(image_height as u16);
                        let image_area = Rect {
                            x: inner.x + 2,
                            y: inner.y + relative_y,
                            width: inner.width.saturating_sub(4),
                            height: render_height,
                        };

                        image_cache.render_cropped(
                            frame,
                            image_area,
                            &img_info.src,
                            image_height as u16,
                        );
                    }
                } else {
                    // Image starts above viewport - show bottom portion (clip at top)
                    let lines_above = scroll_offset - line_offset;
                    let visible_lines = image_height.saturating_sub(lines_above);

                    if visible_lines >= 1 {
                        let render_height = (visible_lines as u16).min(inner.height);
                        let image_area = Rect {
                            x: inner.x + 2,
                            y: inner.y,
                            width: inner.width.saturating_sub(4),
                            height: render_height,
                        };

                        image_cache.render_cropped_bottom(
                            frame,
                            image_area,
                            &img_info.src,
                            image_height as u16,
                        );
                    }
                }
            }
        }
    }

    // Render section footer if visible
    // Footer starts after all content lines (including spacing)
    let footer_line_start = footer_start_line + 2; // After the two blank lines
    let footer_line_end = footer_line_start + footer_height;
    let viewport_end = scroll_offset + visible_height;

    // Check if footer is visible in viewport
    if footer_line_end > scroll_offset && footer_line_start < viewport_end {
        // Calculate footer position within viewport
        let footer_y = if footer_line_start >= scroll_offset {
            // Footer starts within viewport
            inner.y + (footer_line_start - scroll_offset) as u16
        } else {
            // Footer starts above viewport (clipped at top)
            inner.y
        };

        // Calculate available height for footer
        let available_height = inner.height.saturating_sub(footer_y - inner.y);
        if available_height >= section_footer::FOOTER_HEIGHT {
            let footer_area = Rect {
                x: inner.x,
                y: footer_y,
                width: content_area.width,
                height: section_footer::FOOTER_HEIGHT,
            };
            section_footer::draw(frame, footer_area, state, theme);
        }
    }

    // Draw scrollbar
    draw_scrollbar(frame, scrollbar_x, inner.y, inner.height, scroll_offset, total_lines, theme);
}

/// Information about an image to render
struct ImageRenderInfo {
    block_index: usize,
    src: String,
    /// Dynamic height in rows based on image aspect ratio
    height: usize,
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
    render_content_blocks_with_notes(blocks, theme, width, &[])
}

/// Render content blocks with note underlines
pub fn render_content_blocks_with_notes(
    blocks: &[ContentBlock],
    theme: &Theme,
    width: usize,
    note_anchors: &[&NoteAnchor],
) -> Vec<Line<'static>> {
    render_content_blocks_with_visual_mode(blocks, theme, width, note_anchors, None, None)
}

/// Render content blocks with note underlines and visual mode selection
/// Cursor and selection state for rendering
pub struct CursorState {
    /// Cursor block index
    pub cursor_block: usize,
    /// Cursor character offset
    pub cursor_char: usize,
    /// Whether cursor mode is active
    pub cursor_mode: bool,
    /// Whether selection mode is active (for different cursor color)
    pub selection_active: bool,
}

pub fn render_content_blocks_with_visual_mode(
    blocks: &[ContentBlock],
    theme: &Theme,
    width: usize,
    note_anchors: &[&NoteAnchor],
    visual_mode: Option<&VisualModeState>,
    cursor_state: Option<&CursorState>,
) -> Vec<Line<'static>> {
    let empty_heights = std::collections::HashMap::new();
    let (lines, _offsets) = render_content_blocks_with_offsets(
        blocks,
        theme,
        width,
        note_anchors,
        visual_mode,
        cursor_state,
        &empty_heights,
    );
    lines
}

/// Render content blocks and track starting line offset for each block
pub fn render_content_blocks_with_offsets(
    blocks: &[ContentBlock],
    theme: &Theme,
    width: usize,
    note_anchors: &[&NoteAnchor],
    visual_mode: Option<&VisualModeState>,
    cursor_state: Option<&CursorState>,
    image_heights: &std::collections::HashMap<usize, usize>,
) -> (Vec<Line<'static>>, Vec<usize>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut block_offsets: Vec<usize> = Vec::with_capacity(blocks.len());

    for (block_index, block) in blocks.iter().enumerate() {
        // Record the starting line for this block
        block_offsets.push(lines.len());
        // Find note anchors for this block
        let block_anchors: Vec<(usize, usize)> = note_anchors
            .iter()
            .filter_map(|anchor| {
                if anchor.block_index() == Some(block_index) { anchor.char_range() } else { None }
            })
            .collect();

        // Check for visual mode selection in this block
        let selection_range = visual_mode.and_then(|vm| {
            if vm.active {
                if let Some(cs) = cursor_state {
                    let (sb, sc, eb, ec) = vm.selection_range(cs.cursor_block, cs.cursor_char);
                    // Check if this block is within selection
                    if block_index >= sb && block_index <= eb {
                        let start = if block_index == sb { sc } else { 0 };
                        // Add 1 to make selection INCLUSIVE of cursor position (Vim visual mode behavior)
                        let end = if block_index == eb { ec + 1 } else { usize::MAX };
                        Some((start, end))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        });

        // Check if cursor is in this block (for cursor-only mode, no selection)
        let cursor_pos = cursor_state.and_then(|cs| {
            if cs.cursor_mode && !visual_mode.is_some_and(|vm| vm.active) {
                if block_index == cs.cursor_block { Some(cs.cursor_char) } else { None }
            } else {
                None
            }
        });

        match block {
            ContentBlock::Heading { level, text } => {
                if let Some((start, end)) = selection_range {
                    render_heading_with_selection(&mut lines, *level, text, theme, start, end);
                } else if let Some(pos) = cursor_pos {
                    // Show cursor without selection
                    render_heading_with_cursor(&mut lines, *level, text, theme, pos);
                } else {
                    render_heading(&mut lines, *level, text, theme);
                }
            }
            ContentBlock::Paragraph(text) => {
                if let Some((start, end)) = selection_range {
                    render_paragraph_with_selection(&mut lines, text, theme, width, start, end);
                } else if let Some(pos) = cursor_pos {
                    // Show cursor without selection
                    render_paragraph_with_cursor(&mut lines, text, theme, width, pos);
                } else if block_anchors.is_empty() {
                    render_paragraph(&mut lines, text, theme, width);
                } else {
                    render_paragraph_with_underlines(
                        &mut lines,
                        text,
                        theme,
                        width,
                        &block_anchors,
                    );
                }
            }
            ContentBlock::Code(code) => {
                if let Some((start, end)) = selection_range {
                    render_code_block_with_selection(&mut lines, code, theme, width, start, end);
                } else if let Some(pos) = cursor_pos {
                    render_code_block_with_cursor(&mut lines, code, theme, width, pos);
                } else {
                    render_code_block(&mut lines, code, theme, width);
                }
            }
            ContentBlock::UnorderedList(items) => {
                if let Some((start, end)) = selection_range {
                    render_unordered_list_with_selection(
                        &mut lines, items, theme, width, start, end,
                    );
                } else if let Some(pos) = cursor_pos {
                    render_unordered_list_with_cursor(&mut lines, items, theme, width, pos);
                } else {
                    render_unordered_list(&mut lines, items, theme, width);
                }
            }
            ContentBlock::OrderedList(items) => {
                if let Some((start, end)) = selection_range {
                    render_ordered_list_with_selection(&mut lines, items, theme, width, start, end);
                } else if let Some(pos) = cursor_pos {
                    render_ordered_list_with_cursor(&mut lines, items, theme, width, pos);
                } else {
                    render_ordered_list(&mut lines, items, theme, width);
                }
            }
            ContentBlock::Blockquote(text) => {
                if let Some((start, end)) = selection_range {
                    render_blockquote_with_selection(&mut lines, text, theme, width, start, end);
                } else if let Some(pos) = cursor_pos {
                    render_blockquote_with_cursor(&mut lines, text, theme, width, pos);
                } else {
                    render_blockquote(&mut lines, text, theme, width);
                }
            }
            ContentBlock::HorizontalRule => {
                render_horizontal_rule(&mut lines, theme, width);
            }
            ContentBlock::Image { alt, .. } => {
                let height =
                    image_heights.get(&block_index).copied().unwrap_or(IMAGE_RESERVED_HEIGHT);
                render_image(&mut lines, alt, theme, height);
            }
            ContentBlock::Table(table) => {
                render_table(&mut lines, table, theme);
            }
        }
    }

    (lines, block_offsets)
}

fn render_heading(lines: &mut Vec<Line<'static>>, level: u8, text: &str, theme: &Theme) {
    let (base_style, code_color, prefix) = match level {
        1 => (
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            theme.syntax_keyword,
            "  ".to_string(), // Left padding for consistency
        ),
        2 => (
            Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD),
            theme.syntax_function,
            "  ".to_string(), // Left padding for consistency
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
        // Use plain style for padding to avoid underline extending into margin
        spans.push(Span::raw(prefix));
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
    // Add spacing after all headings (including captions which are level 5)
    lines.push(Line::from(""));
}

/// Render heading with selection highlighting
fn render_heading_with_selection(
    lines: &mut Vec<Line<'static>>,
    level: u8,
    text: &str,
    theme: &Theme,
    start: usize,
    end: usize,
) {
    let (base_style, _, prefix) = match level {
        1 => (
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            theme.syntax_keyword,
            "  ".to_string(), // Left padding for consistency
        ),
        2 => (
            Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD),
            theme.syntax_function,
            "  ".to_string(), // Left padding for consistency
        ),
        3 => (
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            theme.syntax_keyword,
            "  ".to_string(),
        ),
        _ => (Style::default().fg(theme.fg_secondary), theme.syntax_keyword, "    ".to_string()),
    };

    let selection_style = Style::default().fg(theme.bg_primary).bg(theme.accent_primary);

    let mut spans: Vec<Span<'static>> = Vec::new();
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix, base_style));
    }

    // Apply selection highlighting
    let chars: Vec<char> = text.chars().collect();
    let end = end.min(chars.len());

    if start > 0 {
        let before: String = chars[..start].iter().collect();
        spans.push(Span::styled(before, base_style));
    }
    if start < end {
        let selected: String = chars[start..end].iter().collect();
        spans.push(Span::styled(selected, selection_style));
    }
    if end < chars.len() {
        let after: String = chars[end..].iter().collect();
        spans.push(Span::styled(after, base_style));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(spans));
    lines.push(Line::from(""));
}

/// Render a heading with cursor (no selection)
fn render_heading_with_cursor(
    lines: &mut Vec<Line<'static>>,
    level: u8,
    text: &str,
    theme: &Theme,
    cursor_pos: usize,
) {
    let (base_style, _, prefix) = match level {
        1 => (
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            theme.syntax_keyword,
            "  ".to_string(), // Left padding for consistency
        ),
        2 => (
            Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD),
            theme.syntax_function,
            "  ".to_string(), // Left padding for consistency
        ),
        3 => (
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            theme.syntax_keyword,
            "  ".to_string(),
        ),
        _ => (Style::default().fg(theme.fg_secondary), theme.syntax_keyword, "    ".to_string()),
    };

    // Cursor style - invert colors for visibility
    let cursor_style = Style::default().fg(theme.bg_primary).bg(theme.accent_primary);

    let mut spans: Vec<Span<'static>> = Vec::new();
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix, base_style));
    }

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    if len == 0 {
        spans.push(Span::styled(" ", cursor_style));
    } else {
        let cursor_pos = cursor_pos.min(len.saturating_sub(1));

        if cursor_pos > 0 {
            let before: String = chars[..cursor_pos].iter().collect();
            spans.push(Span::styled(before, base_style));
        }

        let cursor_char: String = chars[cursor_pos..cursor_pos + 1].iter().collect();
        spans.push(Span::styled(cursor_char, cursor_style));

        if cursor_pos + 1 < len {
            let after: String = chars[cursor_pos + 1..].iter().collect();
            spans.push(Span::styled(after, base_style));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(spans));
    lines.push(Line::from(""));
}

fn render_paragraph(lines: &mut Vec<Line<'static>>, text: &str, theme: &Theme, width: usize) {
    // Parse inline formatting and word-wrap
    let padding = "  "; // Left padding for paragraph text
    let spans = parse_inline_formatting(text, theme);
    let wrapped_lines = wrap_spans(spans, width.saturating_sub(4)); // Account for padding

    for line in wrapped_lines {
        let mut padded_spans = vec![Span::raw(padding)];
        padded_spans.extend(line.spans);
        lines.push(Line::from(padded_spans));
    }
    lines.push(Line::from(""));
}

/// Render a paragraph with selection highlighting
fn render_paragraph_with_selection(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    theme: &Theme,
    width: usize,
    start: usize,
    end: usize,
) {
    let padding = "  "; // Left padding for paragraph text
    let spans = parse_text_with_selection(text, theme, start, end);
    let wrapped_lines = wrap_spans(spans, width.saturating_sub(4)); // Account for padding

    for line in wrapped_lines {
        let mut padded_spans = vec![Span::raw(padding)];
        padded_spans.extend(line.spans);
        lines.push(Line::from(padded_spans));
    }
    lines.push(Line::from(""));
}

/// Render a paragraph with a cursor (no selection)
fn render_paragraph_with_cursor(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    theme: &Theme,
    width: usize,
    cursor_pos: usize,
) {
    let padding = "  "; // Left padding for paragraph text
    let spans = parse_text_with_cursor(text, theme, cursor_pos);
    let wrapped_lines = wrap_spans(spans, width.saturating_sub(4)); // Account for padding

    for line in wrapped_lines {
        let mut padded_spans = vec![Span::raw(padding)];
        padded_spans.extend(line.spans);
        lines.push(Line::from(padded_spans));
    }
    lines.push(Line::from(""));
}

/// Parse text and apply cursor styling (background highlight on cursor character)
/// Preserves inline formatting (backticks for code, bold, italic)
fn parse_text_with_cursor(text: &str, theme: &Theme, cursor_pos: usize) -> Vec<Span<'static>> {
    let base_style = Style::default().fg(theme.fg_primary);
    let code_style = Style::default()
        .fg(theme.syntax_string)
        .bg(theme.bg_secondary)
        .add_modifier(Modifier::BOLD);
    // Cursor style - highlight the character at cursor position with background color
    let cursor_style = Style::default().fg(theme.bg_primary).bg(theme.accent_primary);

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    if len == 0 {
        // Empty text - show a space with cursor highlight
        return vec![Span::styled(" ", cursor_style)];
    }

    let cursor_pos = cursor_pos.min(len.saturating_sub(1));

    // Parse inline formatting and track which ranges are code
    let mut code_ranges: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < len {
        if chars[i] == '`' {
            let start = i + 1;
            let mut end = start;
            while end < len && chars[end] != '`' {
                end += 1;
            }
            if end < len {
                code_ranges.push((start, end));
                i = end + 1;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    let is_in_code =
        |pos: usize| -> bool { code_ranges.iter().any(|(start, end)| pos >= *start && pos < *end) };
    let is_backtick = |pos: usize| -> bool {
        // Check if this position is a backtick that starts or ends a code range
        if chars[pos] != '`' {
            return false;
        }
        code_ranges.iter().any(|(start, end)| pos == start.saturating_sub(1) || pos == *end)
    };

    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_in_code = false;
    let skip_backticks = true; // Skip backticks in output

    for (i, c) in chars.iter().enumerate() {
        let in_code = is_in_code(i);
        let is_tick = is_backtick(i);

        // Skip backtick delimiters
        if is_tick && skip_backticks {
            // Flush current segment before skipping
            if !current.is_empty() {
                let style = if current_in_code { code_style } else { base_style };
                spans.push(Span::styled(current.clone(), style));
                current.clear();
            }
            current_in_code = in_code;
            continue;
        }

        if i == cursor_pos {
            // Flush current segment
            if !current.is_empty() {
                let style = if current_in_code { code_style } else { base_style };
                spans.push(Span::styled(current.clone(), style));
                current.clear();
            }
            // Add cursor character
            spans.push(Span::styled(c.to_string(), cursor_style));
            current_in_code = in_code;
        } else {
            // Check if formatting changed
            if in_code != current_in_code && !current.is_empty() {
                let style = if current_in_code { code_style } else { base_style };
                spans.push(Span::styled(current.clone(), style));
                current.clear();
            }
            current.push(*c);
            current_in_code = in_code;
        }
    }

    // Flush remaining
    if !current.is_empty() {
        let style = if current_in_code { code_style } else { base_style };
        spans.push(Span::styled(current, style));
    }

    if spans.is_empty() {
        spans.push(Span::styled(" ", cursor_style));
    }

    spans
}

/// Parse text and apply selection highlighting with visible cursor
/// Preserves inline formatting (backticks for code, bold, italic)
fn parse_text_with_selection(
    text: &str,
    theme: &Theme,
    start: usize,
    end: usize,
) -> Vec<Span<'static>> {
    let base_style = Style::default().fg(theme.fg_primary);
    let code_style = Style::default()
        .fg(theme.syntax_string)
        .bg(theme.bg_secondary)
        .add_modifier(Modifier::BOLD);
    let selection_style = Style::default().fg(theme.bg_primary).bg(theme.accent_secondary);
    let code_selection_style = Style::default().fg(theme.bg_primary).bg(theme.accent_secondary);

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    if len == 0 {
        return vec![Span::styled(" ", selection_style)];
    }

    let start = start.min(len);
    let end = end.min(len);

    // Parse inline formatting and track which ranges are code
    let mut code_ranges: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < len {
        if chars[i] == '`' {
            let code_start = i + 1;
            let mut code_end = code_start;
            while code_end < len && chars[code_end] != '`' {
                code_end += 1;
            }
            if code_end < len {
                code_ranges.push((code_start, code_end));
                i = code_end + 1;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    let is_in_code =
        |pos: usize| -> bool { code_ranges.iter().any(|(s, e)| pos >= *s && pos < *e) };
    let is_backtick = |pos: usize| -> bool {
        if chars[pos] != '`' {
            return false;
        }
        code_ranges.iter().any(|(s, e)| pos == s.saturating_sub(1) || pos == *e)
    };

    let is_in_selection = |pos: usize| -> bool { pos >= start && pos < end };

    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_in_code = false;
    let mut current_selected = false;

    for (i, c) in chars.iter().enumerate() {
        let in_code = is_in_code(i);
        let is_tick = is_backtick(i);
        let selected = is_in_selection(i);

        // Skip backtick delimiters
        if is_tick {
            // Flush current segment before skipping
            if !current.is_empty() {
                let style = match (current_in_code, current_selected) {
                    (true, true) => code_selection_style,
                    (true, false) => code_style,
                    (false, true) => selection_style,
                    (false, false) => base_style,
                };
                spans.push(Span::styled(current.clone(), style));
                current.clear();
            }
            current_in_code = in_code;
            current_selected = selected;
            continue;
        }

        // Check if formatting or selection state changed
        if (in_code != current_in_code || selected != current_selected) && !current.is_empty() {
            let style = match (current_in_code, current_selected) {
                (true, true) => code_selection_style,
                (true, false) => code_style,
                (false, true) => selection_style,
                (false, false) => base_style,
            };
            spans.push(Span::styled(current.clone(), style));
            current.clear();
        }

        current.push(*c);
        current_in_code = in_code;
        current_selected = selected;
    }

    // Flush remaining
    if !current.is_empty() {
        let style = match (current_in_code, current_selected) {
            (true, true) => code_selection_style,
            (true, false) => code_style,
            (false, true) => selection_style,
            (false, false) => base_style,
        };
        spans.push(Span::styled(current, style));
    }

    if spans.is_empty() {
        spans.push(Span::styled(" ", selection_style));
    }

    spans
}

/// Render a paragraph with underlined note ranges
fn render_paragraph_with_underlines(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    theme: &Theme,
    width: usize,
    underline_ranges: &[(usize, usize)],
) {
    // First, apply underlines to the raw text spans, then parse inline formatting
    let padding = "  "; // Left padding for paragraph text
    let spans = parse_inline_formatting_with_underlines(text, theme, underline_ranges);
    let wrapped_lines = wrap_spans(spans, width.saturating_sub(4)); // Account for padding

    for line in wrapped_lines {
        let mut padded_spans = vec![Span::raw(padding)];
        padded_spans.extend(line.spans);
        lines.push(Line::from(padded_spans));
    }
    lines.push(Line::from(""));
}

/// Parse inline markdown formatting with underlines for note ranges
fn parse_inline_formatting_with_underlines(
    text: &str,
    theme: &Theme,
    underline_ranges: &[(usize, usize)],
) -> Vec<Span<'static>> {
    // Check if a character index falls within any underline range
    let should_underline = |char_idx: usize| -> bool {
        underline_ranges.iter().any(|(start, end)| char_idx >= *start && char_idx < *end)
    };

    // Use accent color for underlined noted text to make it visually distinct
    let underline_color = theme.accent_secondary;

    let mut spans = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current = String::new();
    let mut current_underlined = false;
    let mut char_idx = 0;

    while let Some(c) = chars.next() {
        let is_underlined = should_underline(char_idx);

        match c {
            '`' => {
                // Flush current text
                if !current.is_empty() {
                    let style = if current_underlined {
                        Style::default().fg(underline_color).add_modifier(Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(theme.fg_primary)
                    };
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }

                // Inline code
                let mut code = String::new();
                let code_start_idx = char_idx + 1;
                let mut code_char_idx = code_start_idx;
                while let Some(&next) = chars.peek() {
                    if next == '`' {
                        chars.next();
                        char_idx += 1;
                        break;
                    }
                    code.push(chars.next().unwrap());
                    char_idx += 1;
                    code_char_idx += 1;
                }

                // Check if any part of code should be underlined
                let code_underlined = (code_start_idx..code_char_idx).any(&should_underline);
                let style = if code_underlined {
                    Style::default()
                        .fg(underline_color)
                        .bg(theme.bg_secondary)
                        .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(theme.syntax_string)
                        .bg(theme.bg_secondary)
                        .add_modifier(Modifier::BOLD)
                };
                spans.push(Span::styled(code, style));
                current_underlined = is_underlined;
            }
            '*' | '_' => {
                // Simplified: for bold/italic, just add the characters
                // Full parsing would require tracking nested formatting
                if current_underlined != is_underlined && !current.is_empty() {
                    let style = if current_underlined {
                        Style::default().fg(underline_color).add_modifier(Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(theme.fg_primary)
                    };
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }
                current.push(c);
                current_underlined = is_underlined;
            }
            '[' => {
                // Simplified: just add the character
                if current_underlined != is_underlined && !current.is_empty() {
                    let style = if current_underlined {
                        Style::default().fg(underline_color).add_modifier(Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(theme.fg_primary)
                    };
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }
                current.push(c);
                current_underlined = is_underlined;
            }
            _ => {
                // Regular character - check if underline state changed
                if current_underlined != is_underlined && !current.is_empty() {
                    let style = if current_underlined {
                        Style::default().fg(underline_color).add_modifier(Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(theme.fg_primary)
                    };
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }
                current.push(c);
                current_underlined = is_underlined;
            }
        }
        char_idx += 1;
    }

    // Flush remaining text
    if !current.is_empty() {
        let style = if current_underlined {
            Style::default().fg(underline_color).add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().fg(theme.fg_primary)
        };
        spans.push(Span::styled(current, style));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
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
                    Style::default()
                        .fg(theme.syntax_string)
                        .bg(theme.bg_secondary)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            '*' => {
                // Check for bold (**) or italic (*)
                let is_double = chars.peek() == Some(&'*');
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
                    if next == '*' {
                        if is_double && chars.peek() == Some(&'*') {
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
                    current.push('*');
                    if is_double {
                        current.push('*');
                    }
                    current.push_str(&content);
                }
            }
            '_' => {
                // Underscores only count as formatting at word boundaries
                // Check if previous char was whitespace or start of text
                let prev_is_boundary = current.is_empty()
                    || current.chars().last().map(|c| c.is_whitespace()).unwrap_or(true);

                if !prev_is_boundary {
                    // Mid-word underscore, treat as literal
                    current.push('_');
                    continue;
                }

                // Check for bold (__) or italic (_)
                let is_double = chars.peek() == Some(&'_');
                if is_double {
                    chars.next(); // consume second _
                }

                if !current.is_empty() {
                    spans
                        .push(Span::styled(current.clone(), Style::default().fg(theme.fg_primary)));
                    current.clear();
                }

                let mut content = String::new();
                let mut found_end = false;

                while let Some(next) = chars.next() {
                    if next == '_' {
                        // Check if this underscore is at a word boundary (followed by whitespace or end)
                        let next_is_boundary = chars
                            .peek()
                            .map(|c| {
                                c.is_whitespace()
                                    || *c == '.'
                                    || *c == ','
                                    || *c == '!'
                                    || *c == '?'
                                    || *c == ':'
                                    || *c == ';'
                            })
                            .unwrap_or(true);

                        if is_double && chars.peek() == Some(&'_') && next_is_boundary {
                            chars.next();
                            found_end = true;
                            break;
                        } else if !is_double && next_is_boundary {
                            found_end = true;
                            break;
                        } else {
                            // Mid-word underscore in the content, include it
                            content.push(next);
                        }
                    } else {
                        content.push(next);
                    }
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
                    current.push('_');
                    if is_double {
                        current.push('_');
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

/// Clean up language label by removing attributes like "ignore", "noplayground", etc.
fn clean_language_label(lang: Option<&str>) -> String {
    match lang {
        None => "code".to_string(),
        Some(lang) => {
            // Split by comma and take just the language name
            let parts: Vec<&str> = lang.split(',').collect();
            let base_lang = parts[0].trim();

            // Also remove attributes after space
            let base_lang = base_lang.split_whitespace().next().unwrap_or(base_lang);

            if base_lang.is_empty() { "code".to_string() } else { base_lang.to_string() }
        }
    }
}

fn render_code_block(
    lines: &mut Vec<Line<'static>>,
    code: &crate::book::CodeBlock,
    theme: &Theme,
    width: usize,
) {
    let bg_style = Style::default().bg(theme.bg_secondary);
    let border_style = Style::default().fg(theme.border).bg(theme.bg_secondary);

    // Clean up language label
    let lang_label = clean_language_label(code.language.as_deref());

    // Calculate the max line width for consistent shading
    let code_width = code.code.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let block_width = width.saturating_sub(4).max(code_width + 2);

    // Language label header with background - full width
    let header_padding = block_width.saturating_sub(lang_label.len() + 4);
    lines.push(Line::from(vec![
        Span::styled("┌─ ", border_style),
        Span::styled(lang_label, Style::default().fg(theme.info).bg(theme.bg_secondary)),
        Span::styled(format!(" {}", "─".repeat(header_padding)), border_style),
    ]));

    // Code content with syntax highlighting and full-width background
    for line in code.code.lines() {
        let mut line_spans = vec![Span::styled("│ ", border_style)];
        let highlighted_spans = syntax::highlight_line(line, code.language.as_deref(), theme);

        // Add background to each span
        let highlighted_with_bg: Vec<Span<'static>> = highlighted_spans
            .into_iter()
            .map(|span| Span::styled(span.content.to_string(), span.style.bg(theme.bg_secondary)))
            .collect();

        let line_char_count: usize =
            highlighted_with_bg.iter().map(|s| s.content.chars().count()).sum();

        line_spans.extend(highlighted_with_bg);

        // Pad to fill the block width
        let padding_needed = block_width.saturating_sub(line_char_count + 2);
        if padding_needed > 0 {
            line_spans.push(Span::styled(" ".repeat(padding_needed), bg_style));
        }

        lines.push(Line::from(line_spans));
    }

    // Handle empty code blocks
    if code.code.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(" ".repeat(block_width.saturating_sub(2)), bg_style),
        ]));
    }

    // Bottom border with background - full width (subtract 1 because └ takes one char)
    lines.push(Line::from(vec![Span::styled(
        format!("└{}", "─".repeat(block_width.saturating_sub(1))),
        border_style,
    )]));
    lines.push(Line::from(""));
}

/// Render code block with cursor
fn render_code_block_with_cursor(
    lines: &mut Vec<Line<'static>>,
    code: &crate::book::CodeBlock,
    theme: &Theme,
    width: usize,
    cursor_pos: usize,
) {
    let bg_style = Style::default().bg(theme.bg_secondary);
    let border_style = Style::default().fg(theme.border).bg(theme.bg_secondary);
    let cursor_style = Style::default().fg(theme.bg_primary).bg(theme.accent_primary);

    let lang_label = clean_language_label(code.language.as_deref());

    let code_width = code.code.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let block_width = width.saturating_sub(4).max(code_width + 2);

    // Header - full width
    let header_padding = block_width.saturating_sub(lang_label.len() + 4);
    lines.push(Line::from(vec![
        Span::styled("┌─ ", border_style),
        Span::styled(lang_label, Style::default().fg(theme.info).bg(theme.bg_secondary)),
        Span::styled(format!(" {}", "─".repeat(header_padding)), border_style),
    ]));

    // Track character position across lines
    let mut char_offset = 0;

    for line in code.code.lines() {
        let line_len = line.chars().count();
        let line_end = char_offset + line_len;

        let mut line_spans = vec![Span::styled("│ ", border_style)];

        // Check if cursor is in this line (including newline position at end)
        if cursor_pos >= char_offset && cursor_pos <= line_end {
            let local_pos = cursor_pos - char_offset;
            let chars: Vec<char> = line.chars().collect();

            // Before cursor
            if local_pos > 0 {
                let before: String = chars[..local_pos].iter().collect();
                let before_spans = syntax::highlight_line(&before, code.language.as_deref(), theme);
                for span in before_spans {
                    line_spans.push(Span::styled(
                        span.content.to_string(),
                        span.style.bg(theme.bg_secondary),
                    ));
                }
            }

            // Cursor character (or space for empty line / newline position)
            if local_pos < chars.len() {
                let cursor_char: String = chars[local_pos..local_pos + 1].iter().collect();
                line_spans.push(Span::styled(cursor_char, cursor_style));
            } else {
                // Cursor at end of line (newline position) or on empty line - show cursor as space
                line_spans.push(Span::styled(" ", cursor_style));
            }

            // After cursor
            if local_pos + 1 < chars.len() {
                let after: String = chars[local_pos + 1..].iter().collect();
                let after_spans = syntax::highlight_line(&after, code.language.as_deref(), theme);
                for span in after_spans {
                    line_spans.push(Span::styled(
                        span.content.to_string(),
                        span.style.bg(theme.bg_secondary),
                    ));
                }
            }
        } else {
            // Normal line without cursor
            let highlighted_spans = syntax::highlight_line(line, code.language.as_deref(), theme);
            for span in highlighted_spans {
                line_spans.push(Span::styled(
                    span.content.to_string(),
                    span.style.bg(theme.bg_secondary),
                ));
            }
        }

        // Pad to fill width
        let line_char_count: usize =
            line_spans.iter().skip(1).map(|s| s.content.chars().count()).sum();
        let padding_needed = block_width.saturating_sub(line_char_count + 2);
        if padding_needed > 0 {
            line_spans.push(Span::styled(" ".repeat(padding_needed), bg_style));
        }

        lines.push(Line::from(line_spans));

        // +1 for newline character
        char_offset = line_end + 1;
    }

    // Bottom border - full width (subtract 1 because └ takes one char)
    lines.push(Line::from(vec![Span::styled(
        format!("└{}", "─".repeat(block_width.saturating_sub(1))),
        border_style,
    )]));
    lines.push(Line::from(""));
}

/// Render code block with selection
fn render_code_block_with_selection(
    lines: &mut Vec<Line<'static>>,
    code: &crate::book::CodeBlock,
    theme: &Theme,
    width: usize,
    start: usize,
    end: usize,
) {
    let bg_style = Style::default().bg(theme.bg_secondary);
    let border_style = Style::default().fg(theme.border).bg(theme.bg_secondary);
    let selection_style = Style::default().fg(theme.bg_primary).bg(theme.accent_secondary);

    let lang_label = clean_language_label(code.language.as_deref());

    let code_width = code.code.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let block_width = width.saturating_sub(4).max(code_width + 2);

    // Header - full width
    let header_padding = block_width.saturating_sub(lang_label.len() + 4);
    lines.push(Line::from(vec![
        Span::styled("┌─ ", border_style),
        Span::styled(lang_label, Style::default().fg(theme.info).bg(theme.bg_secondary)),
        Span::styled(format!(" {}", "─".repeat(header_padding)), border_style),
    ]));

    let mut char_offset = 0;

    for line in code.code.lines() {
        let line_len = line.chars().count();
        let line_end = char_offset + line_len;

        let mut line_spans = vec![Span::styled("│ ", border_style)];

        // Check if selection overlaps this line (include newline position)
        // For empty lines, we need start <= line_end (which equals char_offset)
        let line_in_selection = if line_len == 0 {
            // Empty line: check if selection includes this position
            start <= char_offset && end > char_offset
        } else {
            start < line_end && end > char_offset
        };

        if line_in_selection {
            let sel_start = start.saturating_sub(char_offset).min(line_len);
            let sel_end = (end - char_offset).min(line_len);
            let chars: Vec<char> = line.chars().collect();

            // Before selection
            if sel_start > 0 {
                let before: String = chars[..sel_start].iter().collect();
                let before_spans = syntax::highlight_line(&before, code.language.as_deref(), theme);
                for span in before_spans {
                    line_spans.push(Span::styled(
                        span.content.to_string(),
                        span.style.bg(theme.bg_secondary),
                    ));
                }
            }

            // Selected text (or space for empty line)
            if sel_start < sel_end {
                let selected: String = chars[sel_start..sel_end].iter().collect();
                line_spans.push(Span::styled(selected, selection_style));
            } else if line_len == 0 {
                // Empty line within selection - show highlighted space
                line_spans.push(Span::styled(" ", selection_style));
            }

            // After selection
            if sel_end < line_len {
                let after: String = chars[sel_end..].iter().collect();
                let after_spans = syntax::highlight_line(&after, code.language.as_deref(), theme);
                for span in after_spans {
                    line_spans.push(Span::styled(
                        span.content.to_string(),
                        span.style.bg(theme.bg_secondary),
                    ));
                }
            }
        } else {
            // Normal line without selection
            let highlighted_spans = syntax::highlight_line(line, code.language.as_deref(), theme);
            for span in highlighted_spans {
                line_spans.push(Span::styled(
                    span.content.to_string(),
                    span.style.bg(theme.bg_secondary),
                ));
            }
        }

        // Pad to fill width
        let line_char_count: usize =
            line_spans.iter().skip(1).map(|s| s.content.chars().count()).sum();
        let padding_needed = block_width.saturating_sub(line_char_count + 2);
        if padding_needed > 0 {
            line_spans.push(Span::styled(" ".repeat(padding_needed), bg_style));
        }

        lines.push(Line::from(line_spans));

        char_offset = line_end + 1;
    }

    // Bottom border - full width (subtract 1 because └ takes one char)
    lines.push(Line::from(vec![Span::styled(
        format!("└{}", "─".repeat(block_width.saturating_sub(1))),
        border_style,
    )]));
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

/// Render unordered list with cursor
fn render_unordered_list_with_cursor(
    lines: &mut Vec<Line<'static>>,
    items: &[String],
    theme: &Theme,
    width: usize,
    cursor_pos: usize,
) {
    let bullet = "  • ";
    let indent = "    ";
    let content_width = width.saturating_sub(4);

    // Combined text for cursor positioning (items joined by newlines)
    let combined = items.join("\n");
    let chars: Vec<char> = combined.chars().collect();
    let cursor_pos = cursor_pos.min(chars.len().saturating_sub(1));

    // Track which character we're at in combined text
    let mut char_offset = 0;

    for item in items {
        let item_len = item.chars().count();
        let item_end = char_offset + item_len;

        // Check if cursor is within this item
        let item_spans = if cursor_pos >= char_offset && cursor_pos < item_end {
            // Cursor is in this item
            let local_pos = cursor_pos - char_offset;
            parse_text_with_cursor(item, theme, local_pos)
        } else {
            parse_inline_formatting(item, theme)
        };

        let wrapped = wrap_spans(item_spans, content_width);

        for (j, line) in wrapped.into_iter().enumerate() {
            let prefix_span = if j == 0 {
                Span::styled(bullet, Style::default().fg(theme.accent_secondary))
            } else {
                Span::raw(indent)
            };
            let mut line_spans = vec![prefix_span];
            line_spans.extend(line.spans);
            lines.push(Line::from(line_spans));
        }

        // Move past item + newline separator
        char_offset = item_end + 1;
    }

    lines.push(Line::from(""));
}

/// Render unordered list with selection
fn render_unordered_list_with_selection(
    lines: &mut Vec<Line<'static>>,
    items: &[String],
    theme: &Theme,
    width: usize,
    start: usize,
    end: usize,
) {
    let bullet = "  • ";
    let indent = "    ";
    let content_width = width.saturating_sub(4);

    // Combined text for selection positioning
    let combined = items.join("\n");
    let chars: Vec<char> = combined.chars().collect();
    let start = start.min(chars.len());
    let end = end.min(chars.len());

    let mut char_offset = 0;

    for item in items {
        let item_len = item.chars().count();
        let item_end = char_offset + item_len;

        // Calculate selection range within this item
        let item_start = start.saturating_sub(char_offset);
        let item_end_sel = end.saturating_sub(char_offset);

        let item_spans = if start < item_end && end > char_offset {
            // Selection overlaps this item
            parse_text_with_selection(
                item,
                theme,
                item_start.min(item_len),
                item_end_sel.min(item_len),
            )
        } else {
            parse_inline_formatting(item, theme)
        };

        let wrapped = wrap_spans(item_spans, content_width);

        for (j, line) in wrapped.into_iter().enumerate() {
            let prefix_span = if j == 0 {
                Span::styled(bullet, Style::default().fg(theme.accent_secondary))
            } else {
                Span::raw(indent)
            };
            let mut line_spans = vec![prefix_span];
            line_spans.extend(line.spans);
            lines.push(Line::from(line_spans));
        }

        char_offset = item_end + 1;
    }

    lines.push(Line::from(""));
}

/// Render ordered list with cursor
fn render_ordered_list_with_cursor(
    lines: &mut Vec<Line<'static>>,
    items: &[String],
    theme: &Theme,
    width: usize,
    cursor_pos: usize,
) {
    let indent = "     ";
    let content_width = width.saturating_sub(6);

    // Combined text for cursor positioning
    let combined = items.join("\n");
    let chars: Vec<char> = combined.chars().collect();
    let cursor_pos = cursor_pos.min(chars.len().saturating_sub(1));

    let mut char_offset = 0;

    for (i, item) in items.iter().enumerate() {
        let prefix = format!("  {}. ", i + 1);
        let item_len = item.chars().count();
        let item_end = char_offset + item_len;

        let item_spans = if cursor_pos >= char_offset && cursor_pos < item_end {
            let local_pos = cursor_pos - char_offset;
            parse_text_with_cursor(item, theme, local_pos)
        } else {
            parse_inline_formatting(item, theme)
        };

        let wrapped = wrap_spans(item_spans, content_width);

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

        char_offset = item_end + 1;
    }

    lines.push(Line::from(""));
}

/// Render ordered list with selection
fn render_ordered_list_with_selection(
    lines: &mut Vec<Line<'static>>,
    items: &[String],
    theme: &Theme,
    width: usize,
    start: usize,
    end: usize,
) {
    let indent = "     ";
    let content_width = width.saturating_sub(6);

    let combined = items.join("\n");
    let chars: Vec<char> = combined.chars().collect();
    let start = start.min(chars.len());
    let end = end.min(chars.len());

    let mut char_offset = 0;

    for (i, item) in items.iter().enumerate() {
        let prefix = format!("  {}. ", i + 1);
        let item_len = item.chars().count();
        let item_end = char_offset + item_len;

        let item_start = start.saturating_sub(char_offset);
        let item_end_sel = end.saturating_sub(char_offset);

        let item_spans = if start < item_end && end > char_offset {
            parse_text_with_selection(
                item,
                theme,
                item_start.min(item_len),
                item_end_sel.min(item_len),
            )
        } else {
            parse_inline_formatting(item, theme)
        };

        let wrapped = wrap_spans(item_spans, content_width);

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

        char_offset = item_end + 1;
    }

    lines.push(Line::from(""));
}

fn render_blockquote(lines: &mut Vec<Line<'static>>, text: &str, theme: &Theme, width: usize) {
    let prefix = "  │ ";
    let content_width = width.saturating_sub(4); // Account for prefix

    // Split by double newlines (paragraph breaks) and single newlines
    // Process each line/paragraph separately to preserve structure
    for paragraph in text.split("\n\n") {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            continue;
        }

        // Process each line within the paragraph
        for line_text in paragraph.split('\n') {
            let line_text = line_text.trim();
            if line_text.is_empty() {
                // Empty line within blockquote - add blank line with prefix
                lines.push(Line::from(Span::styled(
                    prefix,
                    Style::default().fg(theme.accent_primary),
                )));
                continue;
            }

            let spans = parse_inline_formatting(line_text, theme);
            // Apply muted style to all spans
            let muted_spans: Vec<Span<'static>> = spans
                .into_iter()
                .map(|s| Span::styled(s.content.to_string(), s.style.fg(theme.fg_muted)))
                .collect();
            let wrapped = wrap_spans(muted_spans, content_width);

            for wrapped_line in wrapped {
                let mut line_spans =
                    vec![Span::styled(prefix, Style::default().fg(theme.accent_primary))];
                line_spans.extend(wrapped_line.spans);
                lines.push(Line::from(line_spans));
            }
        }

        // Add blank line between paragraphs
        lines.push(Line::from(Span::styled(prefix, Style::default().fg(theme.accent_primary))));
    }

    // Final empty line after blockquote
    lines.push(Line::from(""));
}

/// Render blockquote with selection highlighting
fn render_blockquote_with_selection(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    theme: &Theme,
    width: usize,
    start: usize,
    end: usize,
) {
    let prefix = "  │ ";
    let content_width = width.saturating_sub(4);
    let selection_style = Style::default().fg(theme.bg_primary).bg(theme.accent_primary);
    let muted_style = Style::default().fg(theme.fg_muted);

    let chars: Vec<char> = text.chars().collect();
    let end = end.min(chars.len());

    // Simplified: render with selection highlight
    let mut spans = Vec::new();

    if start > 0 && !chars.is_empty() {
        let before: String = chars[..start.min(chars.len())].iter().collect();
        spans.push(Span::styled(before, muted_style));
    }

    if start < end && start < chars.len() {
        let selected: String = chars[start..end].iter().collect();
        spans.push(Span::styled(selected, selection_style));
    }

    if end < chars.len() {
        let after: String = chars[end..].iter().collect();
        spans.push(Span::styled(after, muted_style));
    }

    if spans.is_empty() {
        spans.push(Span::styled("", muted_style));
    }

    let wrapped = wrap_spans(spans, content_width);

    for wrapped_line in wrapped {
        let mut line_spans = vec![Span::styled(prefix, Style::default().fg(theme.accent_primary))];
        line_spans.extend(wrapped_line.spans);
        lines.push(Line::from(line_spans));
    }

    lines.push(Line::from(""));
}

/// Render a blockquote with cursor (no selection)
fn render_blockquote_with_cursor(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    theme: &Theme,
    width: usize,
    cursor_pos: usize,
) {
    let prefix = "  │ ";
    let content_width = width.saturating_sub(4);
    // Cursor style - invert colors for visibility
    let cursor_style = Style::default().fg(theme.bg_primary).bg(theme.accent_primary);
    let muted_style = Style::default().fg(theme.fg_muted);

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    let mut spans = Vec::new();

    if len == 0 {
        spans.push(Span::styled(" ", cursor_style));
    } else {
        let cursor_pos = cursor_pos.min(len.saturating_sub(1));

        if cursor_pos > 0 {
            let before: String = chars[..cursor_pos].iter().collect();
            spans.push(Span::styled(before, muted_style));
        }

        let cursor_char: String = chars[cursor_pos..cursor_pos + 1].iter().collect();
        spans.push(Span::styled(cursor_char, cursor_style));

        if cursor_pos + 1 < len {
            let after: String = chars[cursor_pos + 1..].iter().collect();
            spans.push(Span::styled(after, muted_style));
        }
    }

    let wrapped = wrap_spans(spans, content_width);

    for wrapped_line in wrapped {
        let mut line_spans = vec![Span::styled(prefix, Style::default().fg(theme.accent_primary))];
        line_spans.extend(wrapped_line.spans);
        lines.push(Line::from(line_spans));
    }

    lines.push(Line::from(""));
}

fn render_horizontal_rule(lines: &mut Vec<Line<'static>>, theme: &Theme, width: usize) {
    let rule_width = width.saturating_sub(4).min(32);
    lines.push(Line::from(Span::styled("─".repeat(rule_width), Style::default().fg(theme.border))));
}

/// Default height in lines to reserve for images when dimensions unknown
/// Most diagrams are readable at 15-20 rows
const IMAGE_RESERVED_HEIGHT: usize = 15;

fn render_image(lines: &mut Vec<Line<'static>>, _alt: &str, _theme: &Theme, height: usize) {
    // Reserve empty lines for the image to be rendered into
    // The actual image will be rendered on top of this space
    // No trailing space - the caption heading provides its own leading space
    for _ in 0..height {
        lines.push(Line::from(""));
    }
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

        // Add left padding (no background, creates gap between border and table)
        header_spans.push(Span::raw("  "));

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

            // Add left padding (no background, creates gap between border and table)
            row_spans.push(Span::raw("  "));

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
    fn parse_inline_underscore_in_word() {
        // Underscores within words should NOT be treated as italic markers
        let theme = Theme::default();
        let spans = parse_inline_formatting("hello_cargo is a directory", &theme);
        // Should preserve the underscore in the word
        let combined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(combined.contains("hello_cargo"), "Expected 'hello_cargo' but got: {}", combined);
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

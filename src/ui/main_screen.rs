//! Main reading screen with three-panel layout

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{command_line, content, curriculum};
use crate::app::state::{AppState, Panel};
use crate::config::progress::Progress;
use crate::theme::Theme;

/// Minimum width for the curriculum panel
const CURRICULUM_MIN_WIDTH: u16 = 20;

/// Draw the main reading screen
pub fn draw(frame: &mut Frame, state: &mut AppState, theme: &Theme, progress: &Progress) {
    let area = frame.area();

    // Split vertically: main area and command line
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let main_area = vertical_chunks[0];
    let command_area = vertical_chunks[1];

    // Calculate panel layout for main area
    let chunks = create_layout(main_area, state);

    // Draw each visible panel
    let mut panel_index = 0;

    if state.panel_visibility.curriculum {
        draw_curriculum_panel(
            frame,
            chunks[panel_index],
            state,
            theme,
            state.focused_panel == Panel::Curriculum,
            progress,
        );
        panel_index += 1;
    }

    draw_content_panel(
        frame,
        chunks[panel_index],
        state,
        theme,
        state.focused_panel == Panel::Content,
    );
    panel_index += 1;

    if state.panel_visibility.notes {
        draw_notes_panel(
            frame,
            chunks[panel_index],
            state,
            theme,
            state.focused_panel == Panel::Notes,
        );
    }

    // Draw command line at bottom
    command_line::draw(frame, command_area, &state.command_line, theme);
}

/// Create the layout constraints based on visible panels
fn create_layout(area: Rect, state: &AppState) -> Vec<Rect> {
    let mut constraints = Vec::new();

    // Curriculum panel (left): 20% width, min 20 cols
    if state.panel_visibility.curriculum {
        let curriculum_width = (area.width / 5).max(CURRICULUM_MIN_WIDTH);
        constraints.push(Constraint::Length(curriculum_width));
    }

    // Content panel (center): flexible
    constraints.push(Constraint::Min(30));

    // Notes panel (right): 25% width
    if state.panel_visibility.notes {
        let notes_width = area.width / 4;
        constraints.push(Constraint::Length(notes_width));
    }

    Layout::default().direction(Direction::Horizontal).constraints(constraints).split(area).to_vec()
}

/// Draw the curriculum (left) panel
fn draw_curriculum_panel(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
    focused: bool,
    progress: &Progress,
) {
    curriculum::draw_with_progress(frame, area, state, theme, focused, Some(progress));
}

/// Draw the content (center) panel
fn draw_content_panel(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
    focused: bool,
) {
    content::draw(frame, area, state, theme, focused);
}

/// Draw the notes (right) panel
fn draw_notes_panel(
    frame: &mut Frame,
    area: Rect,
    _state: &AppState,
    theme: &Theme,
    focused: bool,
) {
    let border_color = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .title(" Notes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg_primary));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Placeholder for notes
    let msg = Paragraph::new("Notes coming soon...\n\nPress ] to toggle this panel")
        .style(Style::default().fg(theme.fg_muted))
        .wrap(Wrap { trim: true });
    frame.render_widget(msg, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::PanelVisibility;

    #[test]
    fn layout_with_all_panels() {
        let area = Rect::new(0, 0, 120, 40);
        let state = AppState {
            panel_visibility: PanelVisibility { curriculum: true, notes: true },
            ..Default::default()
        };

        let chunks = create_layout(area, &state);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn layout_with_curriculum_only() {
        let area = Rect::new(0, 0, 100, 40);
        let state = AppState {
            panel_visibility: PanelVisibility { curriculum: true, notes: false },
            ..Default::default()
        };

        let chunks = create_layout(area, &state);
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn layout_with_content_only() {
        let area = Rect::new(0, 0, 80, 40);
        let state = AppState {
            panel_visibility: PanelVisibility { curriculum: false, notes: false },
            ..Default::default()
        };

        let chunks = create_layout(area, &state);
        assert_eq!(chunks.len(), 1);
    }
}

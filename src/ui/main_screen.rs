//! Main reading screen with three-panel layout

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use super::{
    claude_panel, command_line, content, curriculum, image::ImageCache, notes_panel, quiz_panel,
};
use crate::app::state::{AppState, Panel};
use crate::config::progress::Progress;
use crate::notes::NotesStore;
use crate::theme::Theme;

/// Minimum width for the curriculum panel
const CURRICULUM_MIN_WIDTH: u16 = 20;

/// Draw the main reading screen
pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    theme: &Theme,
    progress: &Progress,
    notes_store: &NotesStore,
    image_cache: &mut ImageCache,
) {
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
        notes_store,
        image_cache,
    );
    panel_index += 1;

    if state.panel_visibility.notes {
        draw_notes_panel(
            frame,
            chunks[panel_index],
            state,
            theme,
            state.focused_panel == Panel::Notes,
            notes_store,
        );
    }

    // Draw command line at bottom
    command_line::draw(frame, command_area, &state.command_line, theme);

    // Draw Claude response panel as overlay (if visible)
    claude_panel::draw(frame, area, state, theme);

    // Draw quiz panel as overlay (if active)
    quiz_panel::draw(frame, area, state, theme);
}

/// Create the layout constraints based on visible panels
fn create_layout(area: Rect, state: &AppState) -> Vec<Rect> {
    let mut constraints = Vec::new();

    // Curriculum panel (left): configurable width, min 20 cols
    if state.panel_visibility.curriculum {
        let curriculum_width = (area.width as u32
            * state.panel_visibility.curriculum_width_percent as u32
            / 100) as u16;
        constraints.push(Constraint::Length(curriculum_width.max(CURRICULUM_MIN_WIDTH)));
    }

    // Content panel (center): flexible
    constraints.push(Constraint::Min(30));

    // Notes panel (right): configurable width
    if state.panel_visibility.notes {
        let notes_width =
            (area.width as u32 * state.panel_visibility.notes_width_percent as u32 / 100) as u16;
        constraints.push(Constraint::Length(notes_width.max(CURRICULUM_MIN_WIDTH)));
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
    notes_store: &NotesStore,
    image_cache: &mut ImageCache,
) {
    content::draw_with_images(frame, area, state, theme, focused, Some(notes_store), image_cache);
}

/// Draw the notes (right) panel
fn draw_notes_panel(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
    focused: bool,
    notes_store: &NotesStore,
) {
    notes_panel::draw(frame, area, state, theme, focused, notes_store);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::PanelVisibility;

    #[test]
    fn layout_with_all_panels() {
        let area = Rect::new(0, 0, 120, 40);
        let state = AppState {
            panel_visibility: PanelVisibility {
                curriculum: true,
                notes: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let chunks = create_layout(area, &state);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn layout_with_curriculum_only() {
        let area = Rect::new(0, 0, 100, 40);
        let state = AppState {
            panel_visibility: PanelVisibility {
                curriculum: true,
                notes: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let chunks = create_layout(area, &state);
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn layout_with_content_only() {
        let area = Rect::new(0, 0, 80, 40);
        let state = AppState {
            panel_visibility: PanelVisibility {
                curriculum: false,
                notes: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let chunks = create_layout(area, &state);
        assert_eq!(chunks.len(), 1);
    }
}

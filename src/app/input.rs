//! Event handling utilities

use crossterm::event::{KeyCode, KeyModifiers};

/// Vim-style key mapping (basic, without modifiers)
pub fn vim_key_to_action(key: KeyCode) -> Option<Action> {
    match key {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Char('h') | KeyCode::Left => Some(Action::Left),
        KeyCode::Char('l') | KeyCode::Right => Some(Action::Right),
        KeyCode::Char('g') | KeyCode::Home => Some(Action::Top),
        KeyCode::Char('G') | KeyCode::End => Some(Action::Bottom),
        KeyCode::Char('d') | KeyCode::PageDown => Some(Action::PageDown),
        KeyCode::Char('u') | KeyCode::PageUp => Some(Action::PageUp),
        KeyCode::Enter => Some(Action::Select),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Char('/') => Some(Action::Search),
        KeyCode::Char('n') => Some(Action::NextMatch),
        KeyCode::Char('N') => Some(Action::PrevMatch),
        KeyCode::Char('v') => Some(Action::VisualMode),
        KeyCode::Char('?') => Some(Action::Help),
        // Note: 'q' intentionally not mapped - use :q command to quit
        // Panel toggles
        KeyCode::Char('[') | KeyCode::Char('1') => Some(Action::ToggleCurriculum),
        KeyCode::Char(']') | KeyCode::Char('3') => Some(Action::ToggleNotes),
        // Mark complete
        KeyCode::Char('m') => Some(Action::MarkComplete),
        _ => None,
    }
}

/// Key mapping with modifiers (for Ctrl combinations)
pub fn key_with_modifier_to_action(key: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
    if modifiers.contains(KeyModifiers::CONTROL) {
        match key {
            KeyCode::Char('d') => Some(Action::HalfPageDown),
            KeyCode::Char('u') => Some(Action::HalfPageUp),
            KeyCode::Char('f') => Some(Action::PageDown),
            KeyCode::Char('b') => Some(Action::PageUp),
            _ => None,
        }
    } else {
        vim_key_to_action(key)
    }
}

/// Actions that can be taken in the app
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Navigation
    Up,
    Down,
    Left,
    Right,
    Top,
    Bottom,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,

    // Selection
    Select,
    Back,

    // Search
    Search,
    NextMatch,
    PrevMatch,

    // Panel management
    ToggleCurriculum,
    ToggleNotes,

    // Progress
    MarkComplete,

    // Modes
    VisualMode,
    Help,
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vim_j_maps_to_down() {
        assert_eq!(vim_key_to_action(KeyCode::Char('j')), Some(Action::Down));
    }

    #[test]
    fn vim_k_maps_to_up() {
        assert_eq!(vim_key_to_action(KeyCode::Char('k')), Some(Action::Up));
    }

    #[test]
    fn unknown_key_returns_none() {
        assert_eq!(vim_key_to_action(KeyCode::Char('x')), None);
    }

    #[test]
    fn bracket_toggles_curriculum() {
        assert_eq!(vim_key_to_action(KeyCode::Char('[')), Some(Action::ToggleCurriculum));
        assert_eq!(vim_key_to_action(KeyCode::Char('1')), Some(Action::ToggleCurriculum));
    }

    #[test]
    fn bracket_toggles_notes() {
        assert_eq!(vim_key_to_action(KeyCode::Char(']')), Some(Action::ToggleNotes));
        assert_eq!(vim_key_to_action(KeyCode::Char('3')), Some(Action::ToggleNotes));
    }

    #[test]
    fn ctrl_d_half_page_down() {
        assert_eq!(
            key_with_modifier_to_action(KeyCode::Char('d'), KeyModifiers::CONTROL),
            Some(Action::HalfPageDown)
        );
    }

    #[test]
    fn ctrl_u_half_page_up() {
        assert_eq!(
            key_with_modifier_to_action(KeyCode::Char('u'), KeyModifiers::CONTROL),
            Some(Action::HalfPageUp)
        );
    }

    #[test]
    fn no_modifier_uses_vim_keys() {
        assert_eq!(
            key_with_modifier_to_action(KeyCode::Char('j'), KeyModifiers::NONE),
            Some(Action::Down)
        );
    }
}

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

    #[test]
    fn vim_h_maps_to_left() {
        assert_eq!(vim_key_to_action(KeyCode::Char('h')), Some(Action::Left));
        assert_eq!(vim_key_to_action(KeyCode::Left), Some(Action::Left));
    }

    #[test]
    fn vim_l_maps_to_right() {
        assert_eq!(vim_key_to_action(KeyCode::Char('l')), Some(Action::Right));
        assert_eq!(vim_key_to_action(KeyCode::Right), Some(Action::Right));
    }

    #[test]
    fn vim_g_maps_to_top() {
        assert_eq!(vim_key_to_action(KeyCode::Char('g')), Some(Action::Top));
        assert_eq!(vim_key_to_action(KeyCode::Home), Some(Action::Top));
    }

    #[test]
    fn vim_shift_g_maps_to_bottom() {
        assert_eq!(vim_key_to_action(KeyCode::Char('G')), Some(Action::Bottom));
        assert_eq!(vim_key_to_action(KeyCode::End), Some(Action::Bottom));
    }

    #[test]
    fn vim_d_maps_to_page_down() {
        assert_eq!(vim_key_to_action(KeyCode::Char('d')), Some(Action::PageDown));
        assert_eq!(vim_key_to_action(KeyCode::PageDown), Some(Action::PageDown));
    }

    #[test]
    fn vim_u_maps_to_page_up() {
        assert_eq!(vim_key_to_action(KeyCode::Char('u')), Some(Action::PageUp));
        assert_eq!(vim_key_to_action(KeyCode::PageUp), Some(Action::PageUp));
    }

    #[test]
    fn enter_maps_to_select() {
        assert_eq!(vim_key_to_action(KeyCode::Enter), Some(Action::Select));
    }

    #[test]
    fn esc_maps_to_back() {
        assert_eq!(vim_key_to_action(KeyCode::Esc), Some(Action::Back));
    }

    #[test]
    fn slash_maps_to_search() {
        assert_eq!(vim_key_to_action(KeyCode::Char('/')), Some(Action::Search));
    }

    #[test]
    fn n_maps_to_next_match() {
        assert_eq!(vim_key_to_action(KeyCode::Char('n')), Some(Action::NextMatch));
    }

    #[test]
    fn shift_n_maps_to_prev_match() {
        assert_eq!(vim_key_to_action(KeyCode::Char('N')), Some(Action::PrevMatch));
    }

    #[test]
    fn v_maps_to_visual_mode() {
        assert_eq!(vim_key_to_action(KeyCode::Char('v')), Some(Action::VisualMode));
    }

    #[test]
    fn question_maps_to_help() {
        assert_eq!(vim_key_to_action(KeyCode::Char('?')), Some(Action::Help));
    }

    #[test]
    fn m_maps_to_mark_complete() {
        assert_eq!(vim_key_to_action(KeyCode::Char('m')), Some(Action::MarkComplete));
    }

    #[test]
    fn ctrl_f_maps_to_page_down() {
        assert_eq!(
            key_with_modifier_to_action(KeyCode::Char('f'), KeyModifiers::CONTROL),
            Some(Action::PageDown)
        );
    }

    #[test]
    fn ctrl_b_maps_to_page_up() {
        assert_eq!(
            key_with_modifier_to_action(KeyCode::Char('b'), KeyModifiers::CONTROL),
            Some(Action::PageUp)
        );
    }

    #[test]
    fn ctrl_unknown_returns_none() {
        assert_eq!(key_with_modifier_to_action(KeyCode::Char('x'), KeyModifiers::CONTROL), None);
    }

    #[test]
    fn arrow_keys_work() {
        assert_eq!(vim_key_to_action(KeyCode::Down), Some(Action::Down));
        assert_eq!(vim_key_to_action(KeyCode::Up), Some(Action::Up));
    }
}

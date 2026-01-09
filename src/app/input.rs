//! Event handling utilities

use crossterm::event::KeyCode;

/// Vim-style key mapping
pub fn vim_key_to_action(key: KeyCode) -> Option<Action> {
    match key {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Char('h') | KeyCode::Left => Some(Action::Left),
        KeyCode::Char('l') | KeyCode::Right => Some(Action::Right),
        KeyCode::Char('g') => Some(Action::Top),
        KeyCode::Char('G') => Some(Action::Bottom),
        KeyCode::Char('d') => Some(Action::PageDown),
        KeyCode::Char('u') => Some(Action::PageUp),
        KeyCode::Enter => Some(Action::Select),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Char('/') => Some(Action::Search),
        KeyCode::Char('n') => Some(Action::NextMatch),
        KeyCode::Char('N') => Some(Action::PrevMatch),
        KeyCode::Char('v') => Some(Action::VisualMode),
        KeyCode::Char('?') => Some(Action::Help),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}

/// Actions that can be taken in the app
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Top,
    Bottom,
    PageUp,
    PageDown,
    Select,
    Back,
    Search,
    NextMatch,
    PrevMatch,
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
}

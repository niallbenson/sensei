//! UI rendering components

pub mod command_line;
pub mod content;
pub mod curriculum;
pub mod landing;
pub mod layout;
pub mod main_screen;

use ratatui::Frame;

use crate::app::state::{AppState, Screen};
use crate::config::Config;
use crate::config::progress::Progress;

/// Main draw function
pub fn draw(frame: &mut Frame, state: &mut AppState, config: &Config, progress: &Progress) {
    let theme = config.active_theme();

    match &state.screen {
        Screen::Landing => {
            landing::draw(frame, &state.landing_animation, &theme);
        }
        Screen::Main => {
            main_screen::draw(frame, state, &theme, progress);
        }
        Screen::Quiz => {
            layout::draw_placeholder(frame, "Quiz - Coming Soon\n\nPress Esc to return", &theme);
        }
        Screen::Notes => {
            layout::draw_placeholder(frame, "Notes - Coming Soon\n\nPress Esc to return", &theme);
        }
        Screen::Help => {
            layout::draw_placeholder(frame, "Help - Coming Soon\n\nPress Esc to return", &theme);
        }
    }
}

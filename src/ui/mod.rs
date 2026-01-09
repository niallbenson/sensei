//! UI rendering components

pub mod landing;
pub mod layout;

use ratatui::Frame;

use crate::app::state::{AppState, Screen};
use crate::config::Config;

/// Main draw function
pub fn draw(frame: &mut Frame, state: &AppState, config: &Config) {
    let theme = config.active_theme();

    match &state.screen {
        Screen::Landing => {
            landing::draw(frame, &state.landing_animation, &theme);
        }
        Screen::Main => {
            // TODO: Implement main screen
            layout::draw_placeholder(frame, "Main Screen - Coming Soon", &theme);
        }
        Screen::Quiz => {
            layout::draw_placeholder(frame, "Quiz - Coming Soon", &theme);
        }
        Screen::Notes => {
            layout::draw_placeholder(frame, "Notes - Coming Soon", &theme);
        }
        Screen::Help => {
            layout::draw_placeholder(frame, "Help - Coming Soon", &theme);
        }
    }
}

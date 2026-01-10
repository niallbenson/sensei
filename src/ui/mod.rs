//! UI rendering components

pub mod command_line;
pub mod content;
pub mod curriculum;
pub mod image;
pub mod landing;
pub mod layout;
pub mod main_screen;
pub mod notes_panel;

use ratatui::Frame;

use crate::app::state::{AppState, Screen};
use crate::config::Config;
use crate::config::progress::Progress;
use crate::notes::NotesStore;

use self::image::ImageCache;

/// Main draw function
pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    config: &Config,
    progress: &Progress,
    notes_store: &NotesStore,
    image_cache: &mut ImageCache,
) {
    let theme = config.active_theme();

    match &state.screen {
        Screen::Landing => {
            landing::draw(frame, &state.landing_animation, &theme);
        }
        Screen::Main => {
            main_screen::draw(frame, state, &theme, progress, notes_store, image_cache);
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

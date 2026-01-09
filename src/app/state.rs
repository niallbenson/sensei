//! Application state definitions

use std::time::Instant;

/// Which screen is currently displayed
#[derive(Debug, Clone, Default)]
pub enum Screen {
    #[default]
    Landing,
    Main,
    Quiz,
    Notes,
    Help,
}

/// State for the landing animation
#[derive(Debug, Clone)]
pub struct LandingAnimation {
    /// When the animation started
    pub start_time: Instant,

    /// Current character index being drawn
    pub current_char: usize,

    /// Whether animation is complete
    pub complete: bool,
}

impl Default for LandingAnimation {
    fn default() -> Self {
        Self { start_time: Instant::now(), current_char: 0, complete: false }
    }
}

impl LandingAnimation {
    /// Advance the animation
    pub fn tick(&mut self) {
        let elapsed = self.start_time.elapsed().as_millis() as usize;
        // Draw one character every 20ms
        self.current_char = elapsed / 20;
    }
}

/// Full application state
#[derive(Debug, Default)]
pub struct AppState {
    /// Current screen
    pub screen: Screen,

    /// Landing animation state
    pub landing_animation: LandingAnimation,

    /// Currently selected book (if any)
    pub current_book: Option<String>,

    /// Currently selected chapter index
    pub current_chapter: usize,

    /// Currently selected section index
    pub current_section: usize,

    /// Scroll offset in content view
    pub scroll_offset: usize,
}

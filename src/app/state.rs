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

    /// Current animation frame (50ms per frame)
    pub current_frame: usize,

    /// Whether animation is complete (ready for input)
    pub complete: bool,
}

impl Default for LandingAnimation {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            current_frame: 0,
            complete: false,
        }
    }
}

impl LandingAnimation {
    /// Frame timing constants
    pub const MS_PER_FRAME: u128 = 50;
    pub const ENSO_END_FRAME: usize = 30;
    pub const PAUSE_END_FRAME: usize = 40;
    pub const TEXT_END_FRAME: usize = 60;
    pub const TAGLINE_END_FRAME: usize = 70;

    /// Advance the animation based on elapsed time
    pub fn tick(&mut self) {
        let elapsed_ms = self.start_time.elapsed().as_millis();
        self.current_frame = (elapsed_ms / Self::MS_PER_FRAME) as usize;
        self.complete = self.current_frame >= Self::TAGLINE_END_FRAME;
    }

    /// How much of the ensō should be drawn (0.0 to 1.0)
    pub fn enso_progress(&self) -> f32 {
        if self.current_frame >= Self::ENSO_END_FRAME {
            1.0
        } else {
            self.current_frame as f32 / Self::ENSO_END_FRAME as f32
        }
    }

    /// How many characters of "SENSEI" to show
    pub fn title_chars(&self) -> usize {
        if self.current_frame < Self::PAUSE_END_FRAME {
            0
        } else if self.current_frame >= Self::TEXT_END_FRAME {
            6
        } else {
            // 20 frames for 6 chars = ~3.3 frames per char
            let text_frame = self.current_frame - Self::PAUSE_END_FRAME;
            ((text_frame as f32 / 20.0) * 6.0).min(6.0) as usize
        }
    }

    /// Whether to show the tagline
    pub fn show_tagline(&self) -> bool {
        self.current_frame >= Self::TEXT_END_FRAME
    }

    /// Tagline opacity (0.0 to 1.0)
    pub fn tagline_opacity(&self) -> f32 {
        if self.current_frame < Self::TEXT_END_FRAME {
            0.0
        } else if self.current_frame >= Self::TAGLINE_END_FRAME {
            1.0
        } else {
            (self.current_frame - Self::TEXT_END_FRAME) as f32 / 10.0
        }
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

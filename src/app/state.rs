//! Application state definitions

use std::collections::HashSet;
use std::time::Instant;

use crate::book::Book;

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

/// Which panel is currently focused
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Panel {
    Curriculum,
    #[default]
    Content,
    Notes,
}

/// Panel visibility settings
#[derive(Debug, Clone)]
pub struct PanelVisibility {
    /// Show the curriculum (left) panel
    pub curriculum: bool,
    /// Show the notes (right) panel
    pub notes: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self { curriculum: true, notes: false }
    }
}

/// State for the curriculum tree browser
#[derive(Debug, Clone, Default)]
pub struct CurriculumState {
    /// Currently selected item index (flat index in tree)
    pub selected_index: usize,
    /// Which chapter indices are expanded
    pub expanded_chapters: HashSet<usize>,
    /// Scroll offset for long curricula
    pub scroll_offset: usize,
}

/// State for content rendering
#[derive(Debug, Clone, Default)]
pub struct ContentState {
    /// Current scroll position (lines from top)
    pub scroll_offset: usize,
    /// Total rendered lines (updated on render)
    pub total_lines: usize,
    /// Line indices that match current search
    pub search_matches: Vec<usize>,
    /// Currently highlighted match index
    pub current_match: Option<usize>,
}

/// State for search mode
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,
    /// Current search query
    pub query: String,
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
        Self { start_time: Instant::now(), current_frame: 0, complete: false }
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

    /// Currently loaded book (if any)
    pub book: Option<Book>,

    /// Currently selected chapter index
    pub current_chapter: usize,

    /// Currently selected section index
    pub current_section: usize,

    /// Panel visibility settings
    pub panel_visibility: PanelVisibility,

    /// Currently focused panel
    pub focused_panel: Panel,

    /// Curriculum browser state
    pub curriculum: CurriculumState,

    /// Content rendering state
    pub content: ContentState,

    /// Search state
    pub search: SearchState,
}

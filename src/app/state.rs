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
    /// Visible height in items (updated on render)
    pub visible_height: usize,
}

impl CurriculumState {
    /// Ensure the selected item is visible by adjusting scroll offset
    pub fn ensure_selection_visible(&mut self) {
        // Don't scroll past the selection (top)
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
        // Don't let selection go below visible area (bottom)
        let visible = self.visible_height.saturating_sub(2);
        if visible > 0 && self.selected_index >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_index.saturating_sub(visible) + 1;
        }
    }
}

/// State for content rendering
#[derive(Debug, Clone, Default)]
pub struct ContentState {
    /// Current scroll position (lines from top)
    pub scroll_offset: usize,
    /// Total rendered lines (updated on render)
    pub total_lines: usize,
    /// Visible height in lines (updated on render)
    pub visible_height: usize,
    /// Line indices that match current search
    pub search_matches: Vec<usize>,
    /// Currently highlighted match index
    pub current_match: Option<usize>,
}

impl ContentState {
    /// Get the maximum allowed scroll offset
    pub fn max_scroll(&self) -> usize {
        self.total_lines.saturating_sub(self.visible_height / 2)
    }

    /// Clamp scroll offset to valid range
    pub fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
    }
}

/// State for search mode
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,
    /// Current search query
    pub query: String,
}

/// Command line mode
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CommandMode {
    /// Normal mode - command line hidden or showing status
    #[default]
    Normal,
    /// Command mode - accepting : commands
    Command,
    /// Search mode - accepting / search queries
    Search,
}

/// State for the command line input
#[derive(Debug, Clone, Default)]
pub struct CommandLineState {
    /// Current mode
    pub mode: CommandMode,
    /// Input buffer
    pub input: String,
    /// Cursor position in input
    pub cursor: usize,
    /// Status/error message to display (when not in input mode)
    pub message: Option<String>,
    /// Whether message is an error
    pub is_error: bool,
    /// Command history
    pub history: Vec<String>,
    /// Current history index when navigating
    pub history_index: Option<usize>,
}

impl CommandLineState {
    /// Start command mode
    pub fn enter_command_mode(&mut self) {
        self.mode = CommandMode::Command;
        self.input.clear();
        self.cursor = 0;
        self.message = None;
        self.history_index = None;
    }

    /// Start search mode
    pub fn enter_search_mode(&mut self) {
        self.mode = CommandMode::Search;
        self.input.clear();
        self.cursor = 0;
        self.message = None;
        self.history_index = None;
    }

    /// Exit input mode
    pub fn exit_input_mode(&mut self) {
        self.mode = CommandMode::Normal;
        self.input.clear();
        self.cursor = 0;
    }

    /// Set a status message
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = Some(msg.into());
        self.is_error = false;
    }

    /// Set an error message
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.message = Some(msg.into());
        self.is_error = true;
    }

    /// Clear the message
    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Convert character index to byte index
    fn char_to_byte_index(&self, char_idx: usize) -> usize {
        self.input.char_indices().nth(char_idx).map(|(i, _)| i).unwrap_or(self.input.len())
    }

    /// Get the number of characters in input
    fn char_count(&self) -> usize {
        self.input.chars().count()
    }

    /// Insert a character at cursor (cursor is character index)
    pub fn insert_char(&mut self, c: char) {
        let byte_idx = self.char_to_byte_index(self.cursor);
        self.input.insert(byte_idx, c);
        self.cursor += 1;
    }

    /// Delete character before cursor
    pub fn delete_char(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let byte_idx = self.char_to_byte_index(self.cursor);
            self.input.remove(byte_idx);
        }
    }

    /// Delete character at cursor
    pub fn delete_char_forward(&mut self) {
        if self.cursor < self.char_count() {
            let byte_idx = self.char_to_byte_index(self.cursor);
            self.input.remove(byte_idx);
        }
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        if self.cursor < self.char_count() {
            self.cursor += 1;
        }
    }

    /// Move cursor to start
    pub fn move_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn move_end(&mut self) {
        self.cursor = self.char_count();
    }

    /// Get the current input with prefix
    pub fn display_text(&self) -> String {
        match self.mode {
            CommandMode::Normal => self.message.clone().unwrap_or_default(),
            CommandMode::Command => format!(":{}", self.input),
            CommandMode::Search => format!("/{}", self.input),
        }
    }

    /// Check if we're in input mode
    pub fn is_input_mode(&self) -> bool {
        matches!(self.mode, CommandMode::Command | CommandMode::Search)
    }

    /// Maximum number of history entries to keep
    const MAX_HISTORY: usize = 1000;

    /// Add to history
    pub fn add_to_history(&mut self, cmd: String) {
        if !cmd.is_empty() && self.history.last() != Some(&cmd) {
            if self.history.len() >= Self::MAX_HISTORY {
                self.history.remove(0);
            }
            self.history.push(cmd);
        }
    }

    /// Navigate history up
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.history_index = Some(self.history.len() - 1);
            }
            Some(i) if i > 0 => {
                self.history_index = Some(i - 1);
            }
            _ => {}
        }
        if let Some(i) = self.history_index {
            self.input = self.history[i].clone();
            self.cursor = self.input.len();
        }
    }

    /// Navigate history down
    pub fn history_down(&mut self) {
        if let Some(i) = self.history_index {
            if i + 1 < self.history.len() {
                self.history_index = Some(i + 1);
                self.input = self.history[i + 1].clone();
                self.cursor = self.input.len();
            } else {
                self.history_index = None;
                self.input.clear();
                self.cursor = 0;
            }
        }
    }
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

    /// Command line state
    pub command_line: CommandLineState,
}

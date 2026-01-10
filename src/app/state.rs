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
    /// Curriculum panel width as percentage (10-50)
    pub curriculum_width_percent: u16,
    /// Notes panel width as percentage (10-50)
    pub notes_width_percent: u16,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            curriculum: true,
            notes: false,
            curriculum_width_percent: 20,
            notes_width_percent: 25,
        }
    }
}

impl PanelVisibility {
    /// Increase curriculum panel width
    pub fn increase_curriculum_width(&mut self) {
        self.curriculum_width_percent = (self.curriculum_width_percent + 5).min(50);
    }

    /// Decrease curriculum panel width
    pub fn decrease_curriculum_width(&mut self) {
        self.curriculum_width_percent = (self.curriculum_width_percent.saturating_sub(5)).max(10);
    }

    /// Increase notes panel width
    pub fn increase_notes_width(&mut self) {
        self.notes_width_percent = (self.notes_width_percent + 5).min(50);
    }

    /// Decrease notes panel width
    pub fn decrease_notes_width(&mut self) {
        self.notes_width_percent = (self.notes_width_percent.saturating_sub(5)).max(10);
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
    /// Cursor block index (which content block the cursor is in)
    pub cursor_block: usize,
    /// Cursor character offset within the block
    pub cursor_char: usize,
    /// Whether cursor mode is active (showing cursor in content)
    pub cursor_mode: bool,
    /// Frame counter for cursor blinking (toggles every N frames)
    pub cursor_blink_frame: usize,
    /// Starting line number for each content block (computed during render)
    pub block_line_offsets: Vec<usize>,
    /// Whether the section footer buttons are focused
    pub footer_focused: bool,
    /// Which footer button is selected (0 = Quiz, 1 = Next)
    pub footer_button_index: usize,
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

    /// Get the starting line number for a given block index
    pub fn get_block_line(&self, block_index: usize) -> usize {
        self.block_line_offsets.get(block_index).copied().unwrap_or(0)
    }

    /// Ensure the cursor block is visible by scrolling if needed
    pub fn ensure_block_visible(&mut self, block_index: usize) {
        let block_line = self.get_block_line(block_index);

        // If block is above visible area, scroll to show it near top
        if block_line < self.scroll_offset {
            self.scroll_offset = block_line.saturating_sub(2);
        }
        // If block is below visible area, scroll to show it
        else if block_line >= self.scroll_offset + self.visible_height.saturating_sub(3) {
            self.scroll_offset = block_line.saturating_sub(self.visible_height / 3);
        }

        self.clamp_scroll();
    }

    /// Enter cursor mode at the top of visible content
    pub fn enter_cursor_mode(&mut self, first_visible_block: usize) {
        self.cursor_mode = true;
        self.cursor_block = first_visible_block;
        self.cursor_char = 0;
        self.cursor_blink_frame = 0;
    }

    /// Exit cursor mode
    pub fn exit_cursor_mode(&mut self) {
        self.cursor_mode = false;
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.cursor_char > 0 {
            self.cursor_char -= 1;
        }
    }

    /// Move cursor right (needs max chars for boundary)
    pub fn cursor_right(&mut self, max_chars: usize) {
        if self.cursor_char < max_chars {
            self.cursor_char += 1;
        }
    }

    /// Move cursor up to previous block
    pub fn cursor_up(&mut self, min_block: usize) {
        if self.cursor_block > min_block {
            self.cursor_block -= 1;
            // Char position will be clamped by caller
        }
    }

    /// Move cursor down to next block
    pub fn cursor_down(&mut self, max_block: usize) {
        if self.cursor_block < max_block {
            self.cursor_block += 1;
            // Char position will be clamped by caller
        }
    }

    /// Move cursor to start of next word
    pub fn cursor_word_forward(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        if self.cursor_char >= len {
            return;
        }

        let mut pos = self.cursor_char;

        // Skip current word (non-whitespace)
        while pos < len && !chars[pos].is_whitespace() {
            pos += 1;
        }

        // Skip whitespace
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }

        self.cursor_char = pos;
    }

    /// Move cursor to start of previous word
    pub fn cursor_word_backward(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        if self.cursor_char == 0 {
            return;
        }

        let mut pos = self.cursor_char.saturating_sub(1);

        // Skip whitespace before current position
        while pos > 0 && chars[pos].is_whitespace() {
            pos -= 1;
        }

        // Skip current word back to its start
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }

        self.cursor_char = pos;
    }

    /// Move cursor to end of current/next word
    pub fn cursor_word_end(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        if self.cursor_char >= len {
            return;
        }

        let mut pos = self.cursor_char;

        // If at a word char, move forward one
        if pos < len && !chars[pos].is_whitespace() {
            pos += 1;
        }

        // Skip whitespace
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }

        // Skip to end of word
        while pos < len && !chars[pos].is_whitespace() {
            pos += 1;
        }

        // Back up one to be AT the last char of the word
        if pos > self.cursor_char {
            self.cursor_char = pos.saturating_sub(1).max(self.cursor_char);
        }
    }

    /// Move cursor down one line within the current block
    pub fn cursor_line_down(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        if len == 0 {
            return;
        }

        // Clamp cursor position to valid range
        let cursor_pos = self.cursor_char.min(len.saturating_sub(1));

        // Find the next newline character after current cursor position
        let mut newline_pos = cursor_pos;
        while newline_pos < len && chars[newline_pos] != '\n' {
            newline_pos += 1;
        }

        // If we didn't find a newline, check if there's any content after current position
        // that we might have missed (e.g., if cursor is at end of block but not on last line)
        if newline_pos >= len {
            // No newline found - we're on the last line
            // But as a fallback, if there are characters after cursor, try to find them
            if cursor_pos < len.saturating_sub(1) {
                // There's content after cursor - search from the very start for lines
                // This handles edge cases where cursor position might be misaligned
                let mut search_pos = cursor_pos + 1;
                while search_pos < len {
                    if chars[search_pos] == '\n' && search_pos + 1 < len {
                        // Found a newline with content after it
                        self.cursor_char = search_pos + 1;
                        return;
                    }
                    search_pos += 1;
                }
            }
            return;
        }

        // The next line starts right after the newline
        let next_line_start = newline_pos + 1;

        // If next_line_start is at or past the end, there's no actual content after
        if next_line_start >= len {
            return;
        }

        // Find the end of the next line
        let mut next_line_end = next_line_start;
        while next_line_end < len && chars[next_line_end] != '\n' {
            next_line_end += 1;
        }

        // Move to same column on next line, or end of line if shorter
        let next_line_len = next_line_end - next_line_start;

        if next_line_len == 0 {
            // Empty line
            self.cursor_char = next_line_start;
        } else {
            // Go to END of line - this ensures visual selection includes the full line
            // when navigating down with Ctrl+J
            self.cursor_char = next_line_end.saturating_sub(1);
        }
    }

    /// Move cursor up one line within the current block
    pub fn cursor_line_up(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() || self.cursor_char == 0 {
            return;
        }

        // Find current line boundaries and column
        let (line_start, _line_end, col) = self.find_line_info(&chars);

        // If we're on the first line, stay there
        if line_start == 0 {
            return;
        }

        // Find the previous line (line_start - 1 is the newline, so go before that)
        let prev_line_end = line_start - 1; // Position of newline character
        let mut prev_line_start = prev_line_end;
        while prev_line_start > 0 && chars[prev_line_start - 1] != '\n' {
            prev_line_start -= 1;
        }

        // Move to same column on previous line, or end of line if shorter
        let prev_line_len = prev_line_end - prev_line_start;
        let target_col = col.min(prev_line_len.saturating_sub(1));
        self.cursor_char = prev_line_start + target_col;

        // Handle empty lines
        if prev_line_len == 0 {
            self.cursor_char = prev_line_start;
        }
    }

    /// Helper: Find the start, end, and column position of the current line
    fn find_line_info(&self, chars: &[char]) -> (usize, usize, usize) {
        let len = chars.len();
        let cursor_pos = self.cursor_char.min(len.saturating_sub(1));

        // Find line start
        let mut line_start = cursor_pos;
        while line_start > 0 && chars[line_start - 1] != '\n' {
            line_start -= 1;
        }

        // Find line end
        let mut line_end = cursor_pos;
        while line_end < len && chars[line_end] != '\n' {
            line_end += 1;
        }

        // Column is position within line
        let col = cursor_pos - line_start;

        (line_start, line_end, col)
    }

    /// Tick the blink frame counter
    pub fn tick_blink(&mut self) {
        self.cursor_blink_frame = self.cursor_blink_frame.wrapping_add(1);
    }

    /// Whether cursor should be visible (for blinking effect)
    pub fn cursor_visible(&self) -> bool {
        // Blink every ~500ms at 60fps = every 30 frames
        (self.cursor_blink_frame / 15) % 2 == 0
    }

    /// Enter footer focus mode
    pub fn enter_footer(&mut self) {
        self.footer_focused = true;
        self.footer_button_index = 0; // Start on Quiz button
    }

    /// Exit footer focus mode
    pub fn exit_footer(&mut self) {
        self.footer_focused = false;
    }

    /// Move to previous footer button
    pub fn footer_prev(&mut self) {
        if self.footer_button_index > 0 {
            self.footer_button_index -= 1;
        }
    }

    /// Move to next footer button
    pub fn footer_next(&mut self) {
        if self.footer_button_index < 1 {
            self.footer_button_index += 1;
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

/// A single quiz question
#[derive(Debug, Clone, Default)]
pub struct QuizQuestion {
    /// The question text
    pub question: String,
    /// Answer options (typically 4)
    pub options: Vec<String>,
    /// Index of the correct answer (0-3)
    pub correct_index: usize,
}

/// State for section quiz
#[derive(Debug, Clone, Default)]
pub struct QuizState {
    /// Whether quiz overlay is visible
    pub active: bool,
    /// Generated questions
    pub questions: Vec<QuizQuestion>,
    /// Current question index (0-4)
    pub current_question: usize,
    /// User's answers (None = not answered yet)
    pub answers: Vec<Option<usize>>,
    /// Currently selected answer option (0-3)
    pub selected_option: usize,
    /// Quiz completed (showing results)
    pub completed: bool,
    /// Loading state (waiting for Claude to generate questions)
    pub loading: bool,
    /// Error message if generation failed
    pub error: Option<String>,
    /// Section path this quiz is for
    pub section_path: Option<String>,
}

impl QuizState {
    /// Reset quiz state for a new quiz
    pub fn start_loading(&mut self, section_path: &str) {
        self.active = true;
        self.loading = true;
        self.completed = false;
        self.questions.clear();
        self.answers.clear();
        self.current_question = 0;
        self.selected_option = 0;
        self.error = None;
        self.section_path = Some(section_path.to_string());
    }

    /// Set questions after Claude generates them
    pub fn set_questions(&mut self, questions: Vec<QuizQuestion>) {
        self.questions = questions;
        self.answers = vec![None; self.questions.len()];
        self.loading = false;
        self.current_question = 0;
        self.selected_option = 0;
    }

    /// Set error state
    pub fn set_error(&mut self, message: &str) {
        self.error = Some(message.to_string());
        self.loading = false;
    }

    /// Select previous answer option
    pub fn select_prev(&mut self) {
        if self.selected_option > 0 {
            self.selected_option -= 1;
        }
    }

    /// Select next answer option
    pub fn select_next(&mut self) {
        let max_options = self.questions.get(self.current_question).map(|q| q.options.len()).unwrap_or(4);
        if self.selected_option + 1 < max_options {
            self.selected_option += 1;
        }
    }

    /// Confirm current answer and move to next question
    pub fn confirm_answer(&mut self) {
        if self.current_question < self.questions.len() {
            self.answers[self.current_question] = Some(self.selected_option);

            if self.current_question + 1 < self.questions.len() {
                self.current_question += 1;
                self.selected_option = 0;
            } else {
                self.completed = true;
            }
        }
    }

    /// Calculate score (number correct)
    pub fn score(&self) -> (usize, usize) {
        let correct = self.questions.iter().enumerate()
            .filter(|(i, q)| self.answers.get(*i).copied().flatten() == Some(q.correct_index))
            .count();
        (correct, self.questions.len())
    }

    /// Check if quiz was passed (100% required)
    pub fn passed(&self) -> bool {
        let (correct, total) = self.score();
        total > 0 && correct == total
    }

    /// Reset for retry
    pub fn retry(&mut self) {
        self.answers = vec![None; self.questions.len()];
        self.current_question = 0;
        self.selected_option = 0;
        self.completed = false;
    }

    /// Close the quiz
    pub fn close(&mut self) {
        self.active = false;
        self.loading = false;
        self.completed = false;
        self.questions.clear();
        self.answers.clear();
        self.error = None;
        self.section_path = None;
    }
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

/// State for the notes panel
#[derive(Debug, Clone, Default)]
pub struct NotesState {
    /// Currently selected note index in the panel
    pub selected_index: usize,
    /// Scroll offset for the notes list
    pub scroll_offset: usize,
    /// Visible height in lines (updated on render)
    pub visible_height: usize,
    /// ID of note being edited (if any)
    pub editing: Option<String>,
    /// Whether we're in note creation mode
    pub creating: bool,
    /// Input buffer for creating/editing notes
    pub input: String,
    /// Cursor position in input
    pub cursor: usize,
}

impl NotesState {
    /// Start creating a new note
    pub fn start_creating(&mut self) {
        self.creating = true;
        self.editing = None;
        self.input.clear();
        self.cursor = 0;
    }

    /// Start editing an existing note
    pub fn start_editing(&mut self, note_id: &str, content: &str) {
        self.editing = Some(note_id.to_string());
        self.creating = false;
        self.input = content.to_string();
        self.cursor = content.len();
    }

    /// Cancel editing/creating
    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.creating = false;
        self.input.clear();
        self.cursor = 0;
    }

    /// Check if in edit mode (creating or editing)
    pub fn is_editing(&self) -> bool {
        self.creating || self.editing.is_some()
    }

    /// Insert a character at cursor position
    pub fn insert_char(&mut self, c: char) {
        let byte_pos =
            self.input.char_indices().nth(self.cursor).map_or(self.input.len(), |(pos, _)| pos);
        self.input.insert(byte_pos, c);
        self.cursor += 1;
    }

    /// Delete character before cursor
    pub fn delete_char(&mut self) {
        if self.cursor > 0 {
            let byte_pos =
                self.input.char_indices().nth(self.cursor - 1).map(|(pos, _)| pos).unwrap_or(0);
            let next_byte_pos =
                self.input.char_indices().nth(self.cursor).map_or(self.input.len(), |(pos, _)| pos);
            self.input.replace_range(byte_pos..next_byte_pos, "");
            self.cursor -= 1;
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
        let char_count = self.input.chars().count();
        if self.cursor < char_count {
            self.cursor += 1;
        }
    }
}

/// State for visual mode (text selection)
/// The anchor is where selection started; cursor position comes from ContentState
#[derive(Debug, Clone, Default)]
pub struct VisualModeState {
    /// Whether visual mode (selection) is active
    pub active: bool,
    /// Block index where selection started (anchor)
    pub anchor_block: usize,
    /// Character offset within anchor block
    pub anchor_char: usize,
}

impl VisualModeState {
    /// Enter visual mode, setting anchor at current cursor position
    pub fn enter(&mut self, block_index: usize, char_offset: usize) {
        self.active = true;
        self.anchor_block = block_index;
        self.anchor_char = char_offset;
    }

    /// Exit visual mode
    pub fn exit(&mut self) {
        self.active = false;
    }

    /// Get the selection range given the current cursor position
    /// Returns (start_block, start_char, end_block, end_char) where start <= end
    pub fn selection_range(
        &self,
        cursor_block: usize,
        cursor_char: usize,
    ) -> (usize, usize, usize, usize) {
        // Compare positions to determine order
        if self.anchor_block < cursor_block
            || (self.anchor_block == cursor_block && self.anchor_char <= cursor_char)
        {
            (self.anchor_block, self.anchor_char, cursor_block, cursor_char)
        } else {
            (cursor_block, cursor_char, self.anchor_block, self.anchor_char)
        }
    }

    /// Check if a position is within the selection
    pub fn is_selected(
        &self,
        block_index: usize,
        char_index: usize,
        cursor_block: usize,
        cursor_char: usize,
    ) -> bool {
        if !self.active {
            return false;
        }

        let (start_block, start_char, end_block, end_char) =
            self.selection_range(cursor_block, cursor_char);

        if block_index < start_block || block_index > end_block {
            return false;
        }

        if block_index == start_block && block_index == end_block {
            // Selection within single block
            char_index >= start_char && char_index < end_char
        } else if block_index == start_block {
            // Start block - from start_char to end
            char_index >= start_char
        } else if block_index == end_block {
            // End block - from start to end_char
            char_index < end_char
        } else {
            // Middle block - fully selected
            true
        }
    }
}

/// Setup wizard step for Claude configuration
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SetupStep {
    /// Welcome message explaining the feature
    #[default]
    Welcome,
    /// User enters their API key
    EnterApiKey,
    /// User selects preferred model
    SelectModel,
    /// Setup complete, test connection
    Complete,
}

/// State for Claude AI integration
#[derive(Debug, Clone, Default)]
pub struct ClaudeState {
    /// Whether Claude is currently streaming a response
    pub streaming: bool,
    /// Accumulated text from streaming response
    pub stream_buffer: String,
    /// Completed response for display
    pub response: String,
    /// Whether to show the response panel
    pub show_response: bool,
    /// Scroll position in response panel
    pub response_scroll: u16,
    /// Error message from last operation (if any)
    pub error: Option<String>,
    /// Selected model (Haiku or Sonnet)
    pub model: crate::claude::ClaudeModel,
    /// Whether API key setup is needed
    pub needs_setup: bool,
    /// Whether setup wizard is currently active
    pub setup_active: bool,
    /// Current step in setup wizard
    pub setup_step: SetupStep,
    /// Temporary API key during setup (before validation)
    pub setup_api_key: String,
    /// Pending note info: question asked
    pub pending_question: Option<String>,
    /// Pending note info: book ID
    pub pending_book_id: Option<String>,
    /// Pending note info: section path
    pub pending_section_path: Option<String>,
    /// Pending note info: selected text (for anchor)
    pub pending_selection: Option<String>,
    /// Pending note info: selection block index
    pub pending_selection_block: Option<usize>,
    /// Pending note info: selection start char
    pub pending_selection_char: Option<usize>,
}

impl ClaudeState {
    /// Check if Claude integration is available
    pub fn is_available(&self) -> bool {
        !self.needs_setup
    }

    /// Start the setup wizard
    pub fn start_setup(&mut self) {
        self.setup_active = true;
        self.setup_step = SetupStep::Welcome;
        self.setup_api_key.clear();
        self.error = None;
    }

    /// Cancel and exit setup
    pub fn cancel_setup(&mut self) {
        self.setup_active = false;
        self.setup_api_key.clear();
    }

    /// Advance to next setup step
    pub fn next_setup_step(&mut self) {
        self.setup_step = match self.setup_step {
            SetupStep::Welcome => SetupStep::EnterApiKey,
            SetupStep::EnterApiKey => SetupStep::SelectModel,
            SetupStep::SelectModel => SetupStep::Complete,
            SetupStep::Complete => {
                self.setup_active = false;
                self.needs_setup = false;
                SetupStep::Complete
            }
        };
    }

    /// Go back to previous setup step
    pub fn prev_setup_step(&mut self) {
        self.setup_step = match self.setup_step {
            SetupStep::Welcome => SetupStep::Welcome,
            SetupStep::EnterApiKey => SetupStep::Welcome,
            SetupStep::SelectModel => SetupStep::EnterApiKey,
            SetupStep::Complete => SetupStep::SelectModel,
        };
    }

    /// Set error message
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Clear error message
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Clear streaming state
    pub fn clear_streaming(&mut self) {
        self.streaming = false;
        self.stream_buffer.clear();
    }

    /// Finalize the response (called when streaming completes)
    pub fn finalize_response(&mut self) {
        self.response = std::mem::take(&mut self.stream_buffer);
        self.streaming = false;
        self.show_response = true;
        self.response_scroll = 0;
    }

    /// Toggle response panel visibility
    pub fn toggle_response(&mut self) {
        self.show_response = !self.show_response;
    }

    /// Hide response panel
    pub fn hide_response(&mut self) {
        self.show_response = false;
    }

    /// Scroll response up
    pub fn scroll_response_up(&mut self, amount: u16) {
        self.response_scroll = self.response_scroll.saturating_sub(amount);
    }

    /// Scroll response down
    pub fn scroll_response_down(&mut self, amount: u16, max_scroll: u16) {
        self.response_scroll = self.response_scroll.saturating_add(amount).min(max_scroll);
    }

    /// Check if response panel is visible
    pub fn is_response_visible(&self) -> bool {
        self.show_response && !self.response.is_empty()
    }

    /// Set pending note info for saving Q&A as a note
    pub fn set_pending_note(
        &mut self,
        question: &str,
        book_id: &str,
        section_path: &str,
        selection: Option<&str>,
        selection_block: Option<usize>,
        selection_char: Option<usize>,
    ) {
        self.pending_question = Some(question.to_string());
        self.pending_book_id = Some(book_id.to_string());
        self.pending_section_path = Some(section_path.to_string());
        self.pending_selection = selection.map(|s| s.to_string());
        self.pending_selection_block = selection_block;
        self.pending_selection_char = selection_char;
    }

    /// Clear pending note info
    pub fn clear_pending_note(&mut self) {
        self.pending_question = None;
        self.pending_book_id = None;
        self.pending_section_path = None;
        self.pending_selection = None;
        self.pending_selection_block = None;
        self.pending_selection_char = None;
    }

    /// Check if there's pending note info
    pub fn has_pending_note(&self) -> bool {
        self.pending_question.is_some() && self.pending_book_id.is_some()
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

    /// Notes panel state
    pub notes: NotesState,

    /// Visual mode state (text selection)
    pub visual_mode: VisualModeState,

    /// Claude AI integration state
    pub claude: ClaudeState,

    /// Quiz state
    pub quiz: QuizState,
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn command_line_enter_command_mode() {
        let mut state = CommandLineState::default();
        state.input = "old".into();
        state.cursor = 3;
        state.enter_command_mode();
        assert!(matches!(state.mode, CommandMode::Command));
        assert!(state.input.is_empty());
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn command_line_enter_search_mode() {
        let mut state = CommandLineState::default();
        state.enter_search_mode();
        assert!(matches!(state.mode, CommandMode::Search));
    }

    #[test]
    fn command_line_exit_input_mode() {
        let mut state = CommandLineState::default();
        state.enter_command_mode();
        state.input = "test".into();
        state.exit_input_mode();
        assert!(matches!(state.mode, CommandMode::Normal));
        assert!(state.input.is_empty());
    }

    #[test]
    fn command_line_set_message() {
        let mut state = CommandLineState::default();
        state.set_message("hello");
        assert_eq!(state.message, Some("hello".into()));
        assert!(!state.is_error);
    }

    #[test]
    fn command_line_set_error() {
        let mut state = CommandLineState::default();
        state.set_error("error!");
        assert_eq!(state.message, Some("error!".into()));
        assert!(state.is_error);
    }

    #[test]
    fn command_line_clear_message() {
        let mut state = CommandLineState::default();
        state.set_message("hello");
        state.clear_message();
        assert!(state.message.is_none());
    }

    #[test]
    fn command_line_insert_char() {
        let mut state = CommandLineState::default();
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        assert_eq!(state.input, "abc");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn command_line_insert_char_unicode() {
        let mut state = CommandLineState::default();
        state.insert_char('日');
        state.insert_char('本');
        assert_eq!(state.input, "日本");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn command_line_delete_char() {
        let mut state = CommandLineState::default();
        state.input = "abc".into();
        state.cursor = 3;
        state.delete_char();
        assert_eq!(state.input, "ab");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn command_line_delete_char_at_start() {
        let mut state = CommandLineState::default();
        state.input = "abc".into();
        state.cursor = 0;
        state.delete_char();
        assert_eq!(state.input, "abc"); // No change
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn command_line_delete_char_forward() {
        let mut state = CommandLineState::default();
        state.input = "abc".into();
        state.cursor = 1;
        state.delete_char_forward();
        assert_eq!(state.input, "ac");
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn command_line_delete_char_forward_at_end() {
        let mut state = CommandLineState::default();
        state.input = "abc".into();
        state.cursor = 3;
        state.delete_char_forward();
        assert_eq!(state.input, "abc"); // No change
    }

    #[test]
    fn command_line_move_left() {
        let mut state = CommandLineState::default();
        state.input = "abc".into();
        state.cursor = 2;
        state.move_left();
        assert_eq!(state.cursor, 1);
        state.move_left();
        assert_eq!(state.cursor, 0);
        state.move_left(); // Should not go below 0
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn command_line_move_right() {
        let mut state = CommandLineState::default();
        state.input = "abc".into();
        state.cursor = 1;
        state.move_right();
        assert_eq!(state.cursor, 2);
        state.move_right();
        assert_eq!(state.cursor, 3);
        state.move_right(); // Should not go beyond length
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn command_line_move_start_end() {
        let mut state = CommandLineState::default();
        state.input = "hello".into();
        state.cursor = 2;
        state.move_end();
        assert_eq!(state.cursor, 5);
        state.move_start();
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn command_line_display_text() {
        let mut state = CommandLineState::default();
        state.set_message("status");
        assert_eq!(state.display_text(), "status");

        state.enter_command_mode();
        state.input = "quit".into();
        assert_eq!(state.display_text(), ":quit");

        state.enter_search_mode();
        state.input = "pattern".into();
        assert_eq!(state.display_text(), "/pattern");
    }

    #[test]
    fn command_line_is_input_mode() {
        let mut state = CommandLineState::default();
        assert!(!state.is_input_mode());
        state.enter_command_mode();
        assert!(state.is_input_mode());
        state.enter_search_mode();
        assert!(state.is_input_mode());
        state.exit_input_mode();
        assert!(!state.is_input_mode());
    }

    #[test]
    fn command_line_add_to_history() {
        let mut state = CommandLineState::default();
        state.add_to_history("cmd1".into());
        state.add_to_history("cmd2".into());
        assert_eq!(state.history, vec!["cmd1", "cmd2"]);
    }

    #[test]
    fn command_line_history_no_duplicates() {
        let mut state = CommandLineState::default();
        state.add_to_history("cmd".into());
        state.add_to_history("cmd".into());
        assert_eq!(state.history.len(), 1);
    }

    #[test]
    fn command_line_history_no_empty() {
        let mut state = CommandLineState::default();
        state.add_to_history("".into());
        assert!(state.history.is_empty());
    }

    #[test]
    fn command_line_history_navigation() {
        let mut state = CommandLineState::default();
        state.add_to_history("first".into());
        state.add_to_history("second".into());
        state.add_to_history("third".into());

        state.history_up();
        assert_eq!(state.input, "third");
        state.history_up();
        assert_eq!(state.input, "second");
        state.history_up();
        assert_eq!(state.input, "first");
        state.history_up(); // Should stay at first
        assert_eq!(state.input, "first");

        state.history_down();
        assert_eq!(state.input, "second");
        state.history_down();
        assert_eq!(state.input, "third");
        state.history_down(); // Should clear
        assert!(state.input.is_empty());
    }

    #[test]
    fn command_line_history_up_empty() {
        let mut state = CommandLineState::default();
        state.history_up(); // Should not panic
        assert!(state.input.is_empty());
    }

    #[test]
    fn curriculum_state_ensure_selection_visible() {
        let mut state = CurriculumState::default();
        state.visible_height = 10;
        state.selected_index = 15;
        state.ensure_selection_visible();
        // Selection should be visible (scroll_offset should adjust)
        assert!(state.scroll_offset <= state.selected_index);
    }

    #[test]
    fn curriculum_state_ensure_visible_scroll_up() {
        let mut state = CurriculumState::default();
        state.visible_height = 10;
        state.scroll_offset = 5;
        state.selected_index = 2; // Above visible area
        state.ensure_selection_visible();
        assert_eq!(state.scroll_offset, 2);
    }

    #[test]
    fn content_state_default() {
        let state = ContentState::default();
        assert_eq!(state.scroll_offset, 0);
        assert_eq!(state.total_lines, 0);
    }

    #[test]
    fn search_state_default() {
        let state = SearchState::default();
        assert!(state.query.is_empty());
        assert!(!state.active);
    }

    #[test]
    fn panel_visibility_default() {
        let vis = PanelVisibility::default();
        assert!(vis.curriculum);
        assert!(!vis.notes);
    }

    #[test]
    fn panel_default_is_content() {
        let panel = Panel::default();
        assert!(matches!(panel, Panel::Content));
    }

    #[test]
    fn screen_default_is_landing() {
        let screen = Screen::default();
        assert!(matches!(screen, Screen::Landing));
    }

    #[test]
    fn panel_resize_curriculum_increase() {
        let mut vis = PanelVisibility::default();
        vis.curriculum_width_percent = 20;
        vis.increase_curriculum_width();
        assert_eq!(vis.curriculum_width_percent, 25); // Step size is 5
    }

    #[test]
    fn panel_resize_curriculum_decrease() {
        let mut vis = PanelVisibility::default();
        vis.curriculum_width_percent = 20;
        vis.decrease_curriculum_width();
        assert_eq!(vis.curriculum_width_percent, 15); // Step size is 5
    }

    #[test]
    fn panel_resize_curriculum_caps_at_max() {
        let mut vis = PanelVisibility::default();
        vis.curriculum_width_percent = 49;
        vis.increase_curriculum_width();
        assert_eq!(vis.curriculum_width_percent, 50); // Should cap at 50
        vis.increase_curriculum_width();
        assert_eq!(vis.curriculum_width_percent, 50); // Should not exceed 50
    }

    #[test]
    fn panel_resize_curriculum_floors_at_min() {
        let mut vis = PanelVisibility::default();
        vis.curriculum_width_percent = 11;
        vis.decrease_curriculum_width();
        assert_eq!(vis.curriculum_width_percent, 10); // Should floor at 10
        vis.decrease_curriculum_width();
        assert_eq!(vis.curriculum_width_percent, 10); // Should not go below 10
    }

    #[test]
    fn panel_resize_notes_increase() {
        let mut vis = PanelVisibility::default();
        vis.notes_width_percent = 20;
        vis.increase_notes_width();
        assert_eq!(vis.notes_width_percent, 25); // Step size is 5
    }

    #[test]
    fn panel_resize_notes_caps_at_max() {
        let mut vis = PanelVisibility::default();
        vis.notes_width_percent = 49;
        vis.increase_notes_width();
        assert_eq!(vis.notes_width_percent, 50);
        vis.increase_notes_width();
        assert_eq!(vis.notes_width_percent, 50);
    }

    #[test]
    fn panel_resize_notes_floors_at_min() {
        let mut vis = PanelVisibility::default();
        vis.notes_width_percent = 11;
        vis.decrease_notes_width();
        assert_eq!(vis.notes_width_percent, 10);
        vis.decrease_notes_width();
        assert_eq!(vis.notes_width_percent, 10);
    }

    #[test]
    fn content_state_max_scroll() {
        let mut state = ContentState::default();
        state.total_lines = 100;
        state.visible_height = 20;
        // max_scroll = total_lines - visible_height/2 = 100 - 10 = 90
        assert_eq!(state.max_scroll(), 90);
    }

    #[test]
    fn content_state_clamp_scroll() {
        let mut state = ContentState::default();
        state.total_lines = 100;
        state.visible_height = 20;
        state.scroll_offset = 200; // Way beyond max
        state.clamp_scroll();
        assert_eq!(state.scroll_offset, 90); // Should clamp to max_scroll
    }

    #[test]
    fn content_state_clamp_scroll_zero_lines() {
        let mut state = ContentState::default();
        state.total_lines = 0;
        state.visible_height = 20;
        state.scroll_offset = 10;
        state.clamp_scroll();
        assert_eq!(state.scroll_offset, 0);
    }

    // NotesState tests

    #[test]
    fn notes_state_default() {
        let state = NotesState::default();
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);
        assert!(state.editing.is_none());
        assert!(!state.creating);
        assert!(state.input.is_empty());
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn notes_state_start_creating() {
        let mut state = NotesState::default();
        state.input = "old content".into();
        state.cursor = 5;
        state.editing = Some("note123".into());

        state.start_creating();

        assert!(state.creating);
        assert!(state.editing.is_none());
        assert!(state.input.is_empty());
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn notes_state_start_editing() {
        let mut state = NotesState::default();
        state.creating = true;

        state.start_editing("note456", "existing content");

        assert!(!state.creating);
        assert_eq!(state.editing, Some("note456".into()));
        assert_eq!(state.input, "existing content");
        assert_eq!(state.cursor, 16); // Length of "existing content"
    }

    #[test]
    fn notes_state_cancel_edit() {
        let mut state = NotesState::default();
        state.creating = true;
        state.editing = Some("note789".into());
        state.input = "some text".into();
        state.cursor = 4;

        state.cancel_edit();

        assert!(!state.creating);
        assert!(state.editing.is_none());
        assert!(state.input.is_empty());
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn notes_state_is_editing() {
        let mut state = NotesState::default();
        assert!(!state.is_editing());

        state.creating = true;
        assert!(state.is_editing());

        state.creating = false;
        state.editing = Some("note123".into());
        assert!(state.is_editing());

        state.editing = None;
        assert!(!state.is_editing());
    }

    #[test]
    fn notes_state_insert_char() {
        let mut state = NotesState::default();
        state.insert_char('h');
        state.insert_char('i');
        assert_eq!(state.input, "hi");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn notes_state_insert_char_unicode() {
        let mut state = NotesState::default();
        state.insert_char('日');
        state.insert_char('本');
        state.insert_char('語');
        assert_eq!(state.input, "日本語");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn notes_state_insert_char_middle() {
        let mut state = NotesState::default();
        state.input = "hlo".into();
        state.cursor = 1;
        state.insert_char('e');
        state.insert_char('l');
        assert_eq!(state.input, "hello");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn notes_state_delete_char() {
        let mut state = NotesState::default();
        state.input = "hello".into();
        state.cursor = 5;
        state.delete_char();
        assert_eq!(state.input, "hell");
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn notes_state_delete_char_at_start() {
        let mut state = NotesState::default();
        state.input = "hello".into();
        state.cursor = 0;
        state.delete_char();
        assert_eq!(state.input, "hello"); // No change
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn notes_state_delete_char_unicode() {
        let mut state = NotesState::default();
        state.input = "日本語".into();
        state.cursor = 3;
        state.delete_char();
        assert_eq!(state.input, "日本");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn notes_state_move_left() {
        let mut state = NotesState::default();
        state.input = "test".into();
        state.cursor = 3;
        state.move_left();
        assert_eq!(state.cursor, 2);
        state.move_left();
        state.move_left();
        assert_eq!(state.cursor, 0);
        state.move_left(); // Should not go below 0
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn notes_state_move_right() {
        let mut state = NotesState::default();
        state.input = "test".into();
        state.cursor = 1;
        state.move_right();
        assert_eq!(state.cursor, 2);
        state.move_right();
        state.move_right();
        assert_eq!(state.cursor, 4);
        state.move_right(); // Should not exceed length
        assert_eq!(state.cursor, 4);
    }

    // VisualModeState tests

    #[test]
    fn visual_mode_default() {
        let state = VisualModeState::default();
        assert!(!state.active);
        assert_eq!(state.anchor_block, 0);
        assert_eq!(state.anchor_char, 0);
    }

    #[test]
    fn visual_mode_enter_exit() {
        let mut state = VisualModeState::default();
        state.enter(2, 10);
        assert!(state.active);
        assert_eq!(state.anchor_block, 2);
        assert_eq!(state.anchor_char, 10);

        state.exit();
        assert!(!state.active);
    }

    #[test]
    fn visual_mode_selection_range_forward() {
        let mut state = VisualModeState::default();
        state.enter(1, 5);

        // Cursor at block 2, char 10
        let (sb, sc, eb, ec) = state.selection_range(2, 10);
        assert_eq!((sb, sc, eb, ec), (1, 5, 2, 10));
    }

    #[test]
    fn visual_mode_selection_range_backward() {
        let mut state = VisualModeState::default();
        state.enter(3, 15);

        // Cursor at block 1, char 5 (before anchor)
        let (sb, sc, eb, ec) = state.selection_range(1, 5);
        assert_eq!((sb, sc, eb, ec), (1, 5, 3, 15));
    }

    #[test]
    fn visual_mode_is_selected_single_block() {
        let mut state = VisualModeState::default();
        state.enter(1, 5);

        // Cursor at block 1, char 10
        let cursor_block = 1;
        let cursor_char = 10;

        assert!(!state.is_selected(0, 5, cursor_block, cursor_char)); // Wrong block
        assert!(!state.is_selected(1, 4, cursor_block, cursor_char)); // Before selection
        assert!(state.is_selected(1, 5, cursor_block, cursor_char)); // Start
        assert!(state.is_selected(1, 7, cursor_block, cursor_char)); // Middle
        assert!(state.is_selected(1, 9, cursor_block, cursor_char)); // Last char
        assert!(!state.is_selected(1, 10, cursor_block, cursor_char)); // After selection (exclusive end)
    }

    #[test]
    fn visual_mode_is_selected_multi_block() {
        let mut state = VisualModeState::default();
        state.enter(1, 5);

        // Cursor at block 3, char 10
        let cursor_block = 3;
        let cursor_char = 10;

        // Block 0 - not selected
        assert!(!state.is_selected(0, 0, cursor_block, cursor_char));

        // Block 1 - from char 5 onwards
        assert!(!state.is_selected(1, 4, cursor_block, cursor_char));
        assert!(state.is_selected(1, 5, cursor_block, cursor_char));
        assert!(state.is_selected(1, 100, cursor_block, cursor_char)); // Any char after start

        // Block 2 - fully selected
        assert!(state.is_selected(2, 0, cursor_block, cursor_char));
        assert!(state.is_selected(2, 50, cursor_block, cursor_char));

        // Block 3 - up to char 10
        assert!(state.is_selected(3, 0, cursor_block, cursor_char));
        assert!(state.is_selected(3, 9, cursor_block, cursor_char));
        assert!(!state.is_selected(3, 10, cursor_block, cursor_char));

        // Block 4 - not selected
        assert!(!state.is_selected(4, 0, cursor_block, cursor_char));
    }

    #[test]
    fn visual_mode_not_active_not_selected() {
        let state = VisualModeState::default();
        assert!(!state.is_selected(0, 0, 0, 0));
        assert!(!state.is_selected(1, 5, 1, 5));
    }

    // ContentState cursor tests

    #[test]
    fn content_cursor_movement() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);
        assert!(state.cursor_mode);
        assert_eq!(state.cursor_block, 0);
        assert_eq!(state.cursor_char, 0);

        // Move right
        state.cursor_right(10);
        assert_eq!(state.cursor_char, 1);

        // Move left
        state.cursor_left();
        assert_eq!(state.cursor_char, 0);

        // Can't go past start
        state.cursor_left();
        assert_eq!(state.cursor_char, 0);

        // Move down
        state.cursor_down(5);
        assert_eq!(state.cursor_block, 1);

        // Move up
        state.cursor_up(0);
        assert_eq!(state.cursor_block, 0);

        // Can't go above min
        state.cursor_up(0);
        assert_eq!(state.cursor_block, 0);
    }

    #[test]
    fn content_cursor_word_motions() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        let text = "hello world test";

        // Word forward
        state.cursor_word_forward(text);
        assert_eq!(state.cursor_char, 6);

        state.cursor_word_forward(text);
        assert_eq!(state.cursor_char, 12);

        // Word backward
        state.cursor_word_backward(text);
        assert_eq!(state.cursor_char, 6);

        state.cursor_word_backward(text);
        assert_eq!(state.cursor_char, 0);
    }

    #[test]
    fn content_cursor_blink() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        assert!(state.cursor_visible()); // Initially visible

        // Tick through frames
        for _ in 0..15 {
            state.tick_blink();
        }
        assert!(!state.cursor_visible()); // Now hidden

        for _ in 0..15 {
            state.tick_blink();
        }
        assert!(state.cursor_visible()); // Visible again
    }

    #[test]
    fn content_cursor_line_navigation() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Three lines of text
        let text = "line1\nline2\nline3";

        // Start at beginning (line 1)
        state.cursor_char = 0;

        // Move down to line 2 (goes to END of line)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 10); // END of line2

        // Move down to line 3 (goes to END of line)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 16); // END of line3

        // Can't go past last line
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 16); // Still at end of line3

        // Move back up to line 2
        state.cursor_line_up(text);
        assert_eq!(state.cursor_char, 10); // End of line2 (maintains column)

        // Move up to line 1 (maintains column 4)
        state.cursor_line_up(text);
        assert_eq!(state.cursor_char, 4); // Column 4 on line1

        // Can't go above first line
        state.cursor_line_up(text);
        assert_eq!(state.cursor_char, 4); // Still on line1
    }

    #[test]
    fn content_cursor_line_navigation_with_column() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Lines of different lengths
        let text = "short\nmedium len\nend";

        // Start at column 3 of line 1
        state.cursor_char = 3;

        // Move down - now goes to END of line (for visual selection)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 15); // END of "medium len"

        // Move down - goes to END of line 3
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 19); // END of "end"
    }

    #[test]
    fn content_cursor_line_navigation_empty_lines() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Text with empty line in middle
        let text = "first\n\nthird";

        state.cursor_char = 0;

        // Move to empty line
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 6); // Position of empty line

        // Move to third line (last line - goes to END)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 11); // END of "third" (last line)

        // Can't go past last line
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 11);
    }

    #[test]
    fn content_cursor_line_navigation_code_block() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Mimics actual code block with 4 lines (line 2 is empty)
        // Newlines at positions: 30, 31, 70
        // Line 1: 0-29, Line 2 (empty): 31, Line 3: 32-69, Line 4: 71-103
        let text = "println!(\"Guess the number!\");\n\nprintln!(\"Please input your guess.\");\nprintln!(\"You guessed: {guess}\");";

        state.cursor_char = 0; // Start at line 1

        // Move to line 2 (empty) - position 31 (the second newline)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 31, "Should be at empty line 2");

        // Move to line 3 - goes to END of line
        state.cursor_line_down(text);
        // Line 3 is "println!("Please input your guess.");" at positions 32-68, \n at 69
        assert_eq!(state.cursor_char, 68, "Should be at END of line 3");

        // Move to line 4 - goes to END of line
        state.cursor_line_down(text);
        // Line 4 is "println!("You guessed: {guess}");" which is 33 chars at positions 70-102
        assert_eq!(state.cursor_char, 102, "Should be at END of line 4");

        // Verify we're on line 4 by checking we can't go further
        let pos_on_line4 = state.cursor_char;
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, pos_on_line4, "Should stay on last line");
    }

    #[test]
    fn content_cursor_line_navigation_from_end_of_line() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Same structure as the problematic code block
        // Newlines at positions: 30, 31, 69
        let text = "println!(\"Guess the number!\");\n\nprintln!(\"Please input your guess.\");\nprintln!(\"You guessed: {guess}\");";

        // Start at the END of line 3 (position 68, just before the newline at 69)
        state.cursor_char = 68;

        // Move to line 4
        state.cursor_line_down(text);

        // Should be on line 4 now (starts at 70)
        assert!(state.cursor_char >= 70, "Should be on line 4, got {}", state.cursor_char);
    }

    #[test]
    fn content_cursor_line_navigation_from_newline() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Same structure
        let text = "println!(\"Guess the number!\");\n\nprintln!(\"Please input your guess.\");\nprintln!(\"You guessed: {guess}\");";

        // Start AT the newline at end of line 3 (position 69)
        state.cursor_char = 69;

        // Move to line 4
        state.cursor_line_down(text);

        // Should be on line 4 now
        assert!(state.cursor_char >= 70, "Should be on line 4, got {}", state.cursor_char);
    }

    #[test]
    fn content_cursor_line_navigation_with_trailing_newline() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Text WITH trailing newline (like some parsed code blocks might have)
        let text = "line1\nline2\nline3\n";

        state.cursor_char = 0;

        // Move to line 2 (goes to END of line)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 10, "Should be at END of line 2");

        // Move to line 3 (goes to END of line)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 16, "Should be at END of line 3");

        // Try to move past line 3 - should stay (trailing newline = no more content)
        state.cursor_line_down(text);
        assert_eq!(state.cursor_char, 16, "Should stay at END of line 3");
    }

    #[test]
    fn content_cursor_line_navigation_exact_book_content() {
        let mut state = ContentState::default();
        state.enter_cursor_mode(0);

        // Exact content from the Rust book code block (with 4-space indentation)
        let text = "    println!(\"Guess the number!\");\n\n    println!(\"Please input your guess.\");\n    println!(\"You guessed: {guess}\");";

        // Verify structure
        assert_eq!(text.chars().filter(|c| *c == '\n').count(), 3, "Should have 3 newlines");
        let text_len = text.chars().count();

        state.cursor_char = 0; // Start at line 1

        // Move to line 2 (empty line)
        state.cursor_line_down(text);
        let line2_pos = state.cursor_char;
        assert!(text.chars().nth(line2_pos - 1) == Some('\n'), "Should be right after newline");

        // Move to line 3
        state.cursor_line_down(text);
        let line3_pos = state.cursor_char;
        assert!(line3_pos > line2_pos, "Should have advanced");

        // Move to line 4 (last line) - goes to END of line for visual selection
        state.cursor_line_down(text);
        let line4_pos = state.cursor_char;
        assert!(
            line4_pos > line3_pos,
            "Should be on line 4, but got same position {} as line 3",
            line4_pos
        );

        // Verify we're at the END of the last line
        assert_eq!(line4_pos, text_len - 1, "Should be at last character of text");

        // The character at cursor should be the last char of the line (';')
        assert_eq!(text.chars().nth(line4_pos), Some(';'), "Should be at ';' at end of last line");
    }
}

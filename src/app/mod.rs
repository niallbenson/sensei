//! Application state and event handling

pub mod command;
pub mod input;
pub mod state;

use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::book::storage;
use crate::config::{Config, progress::Progress, session::Session};
use crate::notes::NotesStore;
use crate::ui;
use crate::ui::image::ImageCache;
use command::{Command, ParseResult, parse_command};
use input::{Action, key_with_modifier_to_action};
use state::{AppState, CommandMode, Panel, Screen};

/// The main application
pub struct App {
    /// Application configuration
    config: Config,

    /// Current application state
    state: AppState,

    /// Progress tracking data
    progress: Progress,

    /// Session state (UI state persistence)
    session: Session,

    /// Notes storage
    notes_store: NotesStore,

    /// Image cache for rendering images in content
    image_cache: ImageCache,

    /// Terminal backend
    terminal: Terminal<CrosstermBackend<Stdout>>,

    /// Channel receiver for Claude streaming events
    claude_rx: Option<tokio::sync::mpsc::Receiver<crate::claude::StreamEvent>>,

    /// Cancellation token for current Claude request
    claude_cancel: Option<tokio_util::sync::CancellationToken>,

    /// Channel receiver for quiz generation results
    quiz_rx: Option<tokio::sync::mpsc::Receiver<QuizGenerationResult>>,
}

/// Result from quiz generation task
enum QuizGenerationResult {
    Success(Vec<crate::app::state::QuizQuestion>),
    Error(String),
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Result<Self> {
        let terminal = Self::setup_terminal()?;
        let progress = Progress::load().unwrap_or_default();
        let session = Session::load().unwrap_or_default();
        let notes_store = NotesStore::load().unwrap_or_default();

        // Create image cache after terminal setup for proper protocol detection
        let image_cache = ImageCache::new();

        let mut app = Self {
            config,
            state: AppState::default(),
            progress,
            session,
            notes_store,
            image_cache,
            terminal,
            claude_rx: None,
            claude_cancel: None,
            quiz_rx: None,
        };

        // Apply saved panel widths from session
        app.state.panel_visibility.curriculum_width_percent = app.session.curriculum_width_percent;
        app.state.panel_visibility.notes_width_percent = app.session.notes_width_percent;

        // Check if Claude API key is configured
        app.state.claude.needs_setup = !crate::claude::ApiKeyManager::has_api_key();

        // Restore Claude model preference from session
        if let Some(model_str) = &app.session.claude_model {
            if let Some(model) = crate::claude::ClaudeModel::parse(model_str) {
                app.state.claude.model = model;
            }
        }

        // Auto-load first book from library if available
        app.auto_load_book();

        Ok(app)
    }

    /// Auto-load book from the library (preferring last session's book)
    fn auto_load_book(&mut self) {
        let Ok(library) = storage::Library::load() else { return };

        // Try to load the last opened book from session
        let entry = if let Some(ref book_id) = self.session.current_book_id {
            library.find_by_id(book_id).or_else(|| library.entries.first())
        } else {
            library.entries.first()
        };

        let Some(entry) = entry else { return };
        let Ok(book) = storage::load_book(entry) else { return };

        // Set image cache base path from book source
        self.set_image_base_path(&book);

        let book_id = book.metadata.id.clone();
        self.state.book = Some(book);

        // Apply saved session state for this book
        if let Some(book_session) = self.session.book(&book_id) {
            self.state.current_chapter = book_session.current_chapter;
            self.state.current_section = book_session.current_section;
            self.state.content.scroll_offset = book_session.content_scroll_offset;
            self.state.curriculum.selected_index = book_session.selected_index;
            self.state.curriculum.scroll_offset = book_session.curriculum_scroll_offset;
            self.state.curriculum.expanded_chapters = book_session.expanded_chapters.clone();
        } else {
            self.state.current_chapter = 0;
            self.state.current_section = 0;
        }
    }

    /// Set the image cache base path from a book's source
    fn set_image_base_path(&mut self, book: &crate::book::Book) {
        use crate::book::BookSource;
        match &book.metadata.source {
            BookSource::Markdown(path) => {
                self.image_cache.set_base_path(path.clone());
            }
            BookSource::Epub(_) => {
                // EPUB images are embedded, not file-based
                // Clear the base path so images aren't found
                self.image_cache.clear();
            }
        }
    }

    /// Set up the terminal for TUI rendering
    fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(terminal)
    }

    /// Restore the terminal to its original state
    fn restore_terminal(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    /// Save current UI state to session
    fn save_session(&mut self) {
        // Save panel widths (always, not book-specific)
        self.session.curriculum_width_percent =
            self.state.panel_visibility.curriculum_width_percent;
        self.session.notes_width_percent = self.state.panel_visibility.notes_width_percent;

        // Save Claude model preference
        self.session.claude_model = Some(self.state.claude.model.model_id().to_string());

        // Save book-specific state if a book is loaded
        if let Some(book) = &self.state.book {
            let book_id = book.metadata.id.clone();
            self.session.current_book_id = Some(book_id.clone());

            let book_session = self.session.book_mut(&book_id);
            book_session.current_chapter = self.state.current_chapter;
            book_session.current_section = self.state.current_section;
            book_session.content_scroll_offset = self.state.content.scroll_offset;
            book_session.selected_index = self.state.curriculum.selected_index;
            book_session.curriculum_scroll_offset = self.state.curriculum.scroll_offset;
            book_session.expanded_chapters = self.state.curriculum.expanded_chapters.clone();
        }

        if let Err(e) = self.session.save() {
            tracing::warn!("Failed to save session: {}", e);
        }
    }

    /// Run the application main loop
    pub async fn run(&mut self) -> Result<()> {
        // Set up panic hook to restore terminal
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
            original_hook(panic_info);
        }));

        loop {
            // Draw UI - need to borrow state mutably for scroll clamping
            let state = &mut self.state;
            let config = &self.config;
            let progress = &self.progress;
            let notes_store = &self.notes_store;
            let image_cache = &mut self.image_cache;
            self.terminal.draw(|frame| {
                ui::draw(frame, state, config, progress, notes_store, image_cache);
            })?;

            // Process Claude streaming events (non-blocking)
            self.process_claude_events();

            // Process quiz generation results (non-blocking)
            self.process_quiz_events();

            // Handle all pending events before next redraw (makes scrolling feel faster)
            let mut should_quit = false;
            while event::poll(std::time::Duration::from_millis(0))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        // Ctrl+C to cancel Claude streaming
                        if key_event.code == KeyCode::Char('c')
                            && key_event.modifiers.contains(KeyModifiers::CONTROL)
                            && self.state.claude.streaming
                        {
                            self.cancel_claude_stream();
                            continue;
                        }

                        // Route to Claude panel if it's visible
                        if self.state.claude.is_response_visible() {
                            self.handle_claude_panel_input(key_event.code);
                        // Route to notes input if editing a note
                        } else if self.state.notes.is_editing() {
                            self.handle_notes_input(key_event.code);
                        // Route to command line if in input mode
                        } else if self.state.command_line.is_input_mode() {
                            match self.handle_command_line_input(key_event.code).await {
                                Ok(true) => {
                                    should_quit = true;
                                    break;
                                }
                                Ok(false) => {}
                                Err(e) => {
                                    self.state.command_line.set_error(format!("Error: {}", e));
                                }
                            }
                        // Special handling for Ctrl+J which terminals often send as different codes
                        // Works in both cursor mode and visual mode (for extending selection)
                        // NOTE: Terminals may send Ctrl+J as: '\n', '\r', Enter, or 'j' with CONTROL
                        } else if self.state.content.cursor_mode
                            && (key_event.code == KeyCode::Char('\n')
                                || key_event.code == KeyCode::Char('\r')
                                || key_event.code == KeyCode::Enter
                                || (key_event.code == KeyCode::Char('j')
                                    && key_event.modifiers.contains(KeyModifiers::CONTROL)))
                        {
                            // Direct line-down for cursor/visual mode
                            if let Some(text) = self.get_block_text(self.state.content.cursor_block)
                            {
                                self.state.content.cursor_line_down(&text);
                            }
                            self.ensure_cursor_visible();
                            self.update_cursor_message();
                        } else if let Some(action) =
                            key_with_modifier_to_action(key_event.code, key_event.modifiers)
                        {
                            match self.handle_action(action).await {
                                Ok(true) => {
                                    should_quit = true;
                                    break;
                                }
                                Ok(false) => {}
                                Err(e) => {
                                    tracing::error!("Error handling action: {}", e);
                                }
                            }
                        } else {
                            // Handle : and / to enter command modes, 'c' for Claude panel
                            match key_event.code {
                                KeyCode::Char(':') => {
                                    self.state.command_line.enter_command_mode();
                                }
                                KeyCode::Char('/') => {
                                    self.state.command_line.enter_search_mode();
                                }
                                KeyCode::Char('c') => {
                                    // Toggle Claude response panel if there's a response
                                    if !self.state.claude.response.is_empty() {
                                        self.state.claude.toggle_response();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            if should_quit {
                break;
            }

            // Small sleep to prevent busy-waiting when idle (~120fps)
            std::thread::sleep(std::time::Duration::from_millis(4));

            // Update animations
            if matches!(self.state.screen, Screen::Landing) {
                self.state.landing_animation.tick();
            }
        }

        // Save session state before exiting
        self.save_session();

        self.restore_terminal()?;
        Ok(())
    }

    /// Handle an action, returns true if should exit
    async fn handle_action(&mut self, action: Action) -> Result<bool> {
        match &self.state.screen {
            Screen::Landing => {
                // Any action progresses from landing
                self.state.screen = Screen::Main;
            }
            Screen::Main => {
                return self.handle_main_action(action);
            }
            Screen::Help | Screen::Quiz | Screen::Notes => {
                // Escape or quit returns to main
                match action {
                    Action::Quit => return Ok(true),
                    Action::Back => {
                        self.state.screen = Screen::Main;
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    /// Handle actions on the main screen
    fn handle_main_action(&mut self, action: Action) -> Result<bool> {
        // Handle quiz input if quiz is active
        if self.state.quiz.active {
            return self.handle_quiz_action(action);
        }

        // If content panel is focused, handle cursor/visual mode
        if self.state.focused_panel == Panel::Content && self.state.content.cursor_mode {
            return self.handle_content_cursor_action(action);
        }

        match action {
            Action::Quit => return Ok(true),

            // Toggle visual mode (only when content focused)
            Action::VisualMode => {
                if self.state.focused_panel == Panel::Content {
                    self.toggle_visual_mode();
                }
            }

            // Panel toggles
            Action::ToggleCurriculum => {
                self.state.panel_visibility.curriculum = !self.state.panel_visibility.curriculum;
                // If we're hiding the currently focused panel, move focus
                if !self.state.panel_visibility.curriculum
                    && self.state.focused_panel == Panel::Curriculum
                {
                    self.state.focused_panel = Panel::Content;
                }
            }
            Action::ToggleNotes => {
                self.state.panel_visibility.notes = !self.state.panel_visibility.notes;
                // If we're hiding the currently focused panel, move focus
                if !self.state.panel_visibility.notes && self.state.focused_panel == Panel::Notes {
                    self.state.focused_panel = Panel::Content;
                }
            }

            // Panel resize (based on focused panel)
            Action::IncreasePanelWidth => match self.state.focused_panel {
                Panel::Curriculum => {
                    self.state.panel_visibility.increase_curriculum_width();
                }
                Panel::Notes => {
                    self.state.panel_visibility.increase_notes_width();
                }
                Panel::Content => {}
            },
            Action::DecreasePanelWidth => match self.state.focused_panel {
                Panel::Curriculum => {
                    self.state.panel_visibility.decrease_curriculum_width();
                }
                Panel::Notes => {
                    self.state.panel_visibility.decrease_notes_width();
                }
                Panel::Content => {}
            },

            // Panel navigation (h/l move between panels)
            // But when footer is focused, switch between footer buttons instead
            Action::Left => {
                if self.state.focused_panel == Panel::Content && self.state.content.footer_focused {
                    self.state.content.footer_prev();
                } else {
                    self.move_panel_focus_left();
                }
            }
            Action::Right => {
                if self.state.focused_panel == Panel::Content && self.state.content.footer_focused {
                    self.state.content.footer_next();
                } else {
                    self.move_panel_focus_right();
                }
            }

            // Vertical navigation depends on focused panel
            Action::Up | Action::Down | Action::Top | Action::Bottom => {
                self.handle_vertical_navigation(action);
            }

            // Page scrolling (content panel)
            Action::PageUp | Action::PageDown | Action::HalfPageUp | Action::HalfPageDown => {
                self.handle_scroll(action);
            }

            // Selection
            Action::Select => {
                self.handle_select();
            }

            Action::Help => {
                self.state.screen = Screen::Help;
            }

            Action::MarkComplete => {
                self.toggle_section_complete();
            }

            // Note actions
            Action::CreateNote => {
                self.start_creating_note();
            }
            Action::EditNote => {
                self.start_editing_note();
            }
            Action::DeleteNote => {
                self.delete_selected_note();
            }

            _ => {}
        }
        Ok(false)
    }

    /// Handle actions when cursor mode is active in content panel
    fn handle_content_cursor_action(&mut self, action: Action) -> Result<bool> {
        match action {
            Action::Quit => return Ok(true),

            Action::Back => {
                if self.state.visual_mode.active {
                    // Exit visual mode but keep cursor mode
                    self.state.visual_mode.exit();
                    self.state
                        .command_line
                        .set_message("-- CURSOR -- (h/j/k/l to move, v to select, Esc to exit)");
                } else {
                    // Exit cursor mode
                    self.state.content.exit_cursor_mode();
                    self.state.command_line.clear_message();
                }
            }

            Action::VisualMode => {
                self.toggle_visual_mode();
            }

            // Cursor movement
            Action::Left => {
                self.state.content.cursor_left();
                self.ensure_cursor_visible();
                self.update_cursor_message();
            }
            Action::Right => {
                let max_chars = self.get_block_char_count(self.state.content.cursor_block);
                self.state.content.cursor_right(max_chars);
                self.ensure_cursor_visible();
                self.update_cursor_message();
            }
            Action::Up => {
                let min_block = self.find_first_text_block_index();
                self.state.content.cursor_up(min_block);
                // Clamp char position to new block
                let max_chars = self.get_block_char_count(self.state.content.cursor_block);
                if self.state.content.cursor_char > max_chars {
                    self.state.content.cursor_char = max_chars;
                }
                self.ensure_cursor_visible();
                self.update_cursor_message();
            }
            Action::Down => {
                let max_block = self.get_block_count().saturating_sub(1);
                self.state.content.cursor_down(max_block);
                // Clamp char position to new block
                let max_chars = self.get_block_char_count(self.state.content.cursor_block);
                if self.state.content.cursor_char > max_chars {
                    self.state.content.cursor_char = max_chars;
                }
                self.ensure_cursor_visible();
                self.update_cursor_message();
            }

            // Word motions
            Action::WordForward => {
                if let Some(text) = self.get_block_text(self.state.content.cursor_block) {
                    self.state.content.cursor_word_forward(&text);
                    self.ensure_cursor_visible();
                    self.update_cursor_message();
                }
            }
            Action::WordBackward => {
                if let Some(text) = self.get_block_text(self.state.content.cursor_block) {
                    self.state.content.cursor_word_backward(&text);
                    self.ensure_cursor_visible();
                    self.update_cursor_message();
                }
            }
            Action::WordEnd => {
                if let Some(text) = self.get_block_text(self.state.content.cursor_block) {
                    self.state.content.cursor_word_end(&text);
                    self.ensure_cursor_visible();
                    self.update_cursor_message();
                }
            }

            // Create note on selection (visual mode) or move line down (cursor mode)
            // Note: Enter key (Ctrl+J) often comes through as Select action
            Action::CreateNote => {
                if self.state.visual_mode.active {
                    self.create_note_from_selection();
                }
            }
            Action::Select => {
                if self.state.visual_mode.active {
                    self.create_note_from_selection();
                } else {
                    // In cursor mode, Enter moves down one line (like Ctrl+J)
                    if let Some(text) = self.get_block_text(self.state.content.cursor_block) {
                        self.state.content.cursor_line_down(&text);
                    }
                    self.ensure_cursor_visible();
                    self.update_cursor_message();
                }
            }

            // Line-by-line navigation within blocks
            Action::LineUp => {
                if let Some(text) = self.get_block_text(self.state.content.cursor_block) {
                    self.state.content.cursor_line_up(&text);
                }
                self.ensure_cursor_visible();
                self.update_cursor_message();
            }
            Action::LineDown => {
                if let Some(text) = self.get_block_text(self.state.content.cursor_block) {
                    self.state.content.cursor_line_down(&text);
                }
                self.ensure_cursor_visible();
                self.update_cursor_message();
            }

            // Yank (copy to clipboard)
            Action::Yank => {
                if self.state.visual_mode.active {
                    self.yank_selection();
                }
            }

            _ => {}
        }
        Ok(false)
    }

    /// Enter cursor mode at the top of visible content
    fn enter_cursor_mode(&mut self) {
        // Find the first text block that's visible on screen
        let first_visible = self.find_first_visible_text_block();
        self.state.content.enter_cursor_mode(first_visible);
        self.state
            .command_line
            .set_message("-- CURSOR -- (h/j/k/l to move, v to select, Esc to exit)");
    }

    /// Toggle between normal -> cursor mode -> visual mode
    fn toggle_visual_mode(&mut self) {
        if self.state.visual_mode.active {
            // Exit visual mode back to cursor mode
            self.state.visual_mode.exit();
            self.state
                .command_line
                .set_message("-- CURSOR -- (h/j/k/l to move, v to select, Esc to exit)");
        } else if self.state.content.cursor_mode {
            // Already in cursor mode, start visual selection
            self.state
                .visual_mode
                .enter(self.state.content.cursor_block, self.state.content.cursor_char);
            self.state
                .command_line
                .set_message("-- VISUAL -- (move to select, a/Enter to annotate, v/Esc to cancel)");
        } else {
            // Enter cursor mode (navigation)
            self.enter_cursor_mode();
        }
    }

    /// Update the status message based on cursor/visual mode state
    fn update_cursor_message(&mut self) {
        if self.state.visual_mode.active {
            let (sb, sc, eb, ec) = self
                .state
                .visual_mode
                .selection_range(self.state.content.cursor_block, self.state.content.cursor_char);
            if sb == eb {
                let len = ec.saturating_sub(sc);
                self.state.command_line.set_message(format!("-- VISUAL -- {} chars selected", len));
            } else {
                self.state
                    .command_line
                    .set_message(format!("-- VISUAL -- blocks {}-{} selected", sb, eb));
            }
        } else {
            self.state.command_line.set_message(format!(
                "-- CURSOR -- block {}, char {}",
                self.state.content.cursor_block, self.state.content.cursor_char
            ));
        }
    }

    /// Ensure the cursor is visible by scrolling if needed
    fn ensure_cursor_visible(&mut self) {
        // Use the actual block line offsets computed during rendering
        let cursor_block = self.state.content.cursor_block;
        self.state.content.ensure_block_visible(cursor_block);
    }

    /// Check if a block is navigable (contains selectable text)
    fn is_navigable_block(block: &crate::book::ContentBlock) -> bool {
        matches!(
            block,
            crate::book::ContentBlock::Paragraph(_)
                | crate::book::ContentBlock::Heading { .. }
                | crate::book::ContentBlock::Blockquote(_)
                | crate::book::ContentBlock::UnorderedList(_)
                | crate::book::ContentBlock::OrderedList(_)
                | crate::book::ContentBlock::Code(_)
        )
    }

    /// Find the first text block that's currently visible
    fn find_first_visible_text_block(&self) -> usize {
        // Approximate: assume each block takes ~3 lines
        let scroll_offset = self.state.content.scroll_offset;
        let approx_block = scroll_offset / 3;

        // Find the first navigable block at or after this position
        let Some(book) = &self.state.book else { return 0 };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return 0;
        };

        section
            .content
            .iter()
            .enumerate()
            .skip(approx_block)
            .find(|(_, b)| Self::is_navigable_block(b))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Find the index of the first navigable block
    fn find_first_text_block_index(&self) -> usize {
        let Some(book) = &self.state.book else { return 0 };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return 0;
        };

        section.content.iter().position(Self::is_navigable_block).unwrap_or(0)
    }

    /// Get the total number of content blocks in current section
    fn get_block_count(&self) -> usize {
        let Some(book) = &self.state.book else { return 0 };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return 0;
        };
        section.content.len()
    }

    /// Get the character count for a block
    fn get_block_char_count(&self, block_index: usize) -> usize {
        self.get_block_text(block_index).map(|t| t.chars().count()).unwrap_or(0)
    }

    /// Get the text content of a block
    fn get_block_text(&self, block_index: usize) -> Option<String> {
        let book = self.state.book.as_ref()?;
        let section = book.get_section(self.state.current_chapter, self.state.current_section)?;

        match section.content.get(block_index) {
            Some(crate::book::ContentBlock::Paragraph(text)) => Some(text.clone()),
            Some(crate::book::ContentBlock::Blockquote(text)) => Some(text.clone()),
            Some(crate::book::ContentBlock::Heading { text, .. }) => Some(text.clone()),
            Some(crate::book::ContentBlock::UnorderedList(items)) => {
                // Combine all list items with newlines for navigation
                Some(items.join("\n"))
            }
            Some(crate::book::ContentBlock::OrderedList(items)) => {
                // Combine all list items with newlines for navigation
                Some(items.join("\n"))
            }
            Some(crate::book::ContentBlock::Code(code)) => {
                // Return the code content for navigation
                Some(code.code.clone())
            }
            _ => None,
        }
    }

    /// Create a note from the current visual selection
    fn create_note_from_selection(&mut self) {
        use crate::notes::Note;

        let Some(book) = &self.state.book else {
            self.state.visual_mode.exit();
            return;
        };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            self.state.visual_mode.exit();
            return;
        };

        let (start_block, start_char, end_block, end_char) = self
            .state
            .visual_mode
            .selection_range(self.state.content.cursor_block, self.state.content.cursor_char);

        // For simplicity, only support single-block selection for now
        if start_block != end_block {
            self.state.command_line.set_error("Multi-block selection not yet supported");
            self.state.visual_mode.exit();
            return;
        }

        // Extract the selected text
        let selected_text = match section.content.get(start_block) {
            Some(crate::book::ContentBlock::Paragraph(text)) => {
                text.chars().skip(start_char).take(end_char - start_char).collect::<String>()
            }
            Some(crate::book::ContentBlock::Blockquote(text)) => {
                text.chars().skip(start_char).take(end_char - start_char).collect::<String>()
            }
            Some(crate::book::ContentBlock::Heading { text, .. }) => {
                text.chars().skip(start_char).take(end_char - start_char).collect::<String>()
            }
            _ => {
                self.state.command_line.set_error("Cannot annotate this block type");
                self.state.visual_mode.exit();
                return;
            }
        };

        if selected_text.is_empty() {
            self.state.command_line.set_error("No text selected");
            self.state.visual_mode.exit();
            return;
        }

        // Create the note with anchor
        let note = Note::new_selection_note(
            &book.metadata.id,
            &section.path,
            "", // Empty content - user will edit
            start_block,
            start_char,
            &selected_text,
        );

        let note_id = note.id.clone();
        self.notes_store.add_note(note);

        // Start editing the note content
        self.state.notes.start_editing(&note_id, "");
        self.state.panel_visibility.notes = true;
        self.state.focused_panel = Panel::Notes;

        // Exit visual mode
        self.state.visual_mode.exit();
        self.state
            .command_line
            .set_message(format!("Annotating: \"{}\"", truncate_str(&selected_text, 30)));
    }

    /// Mark current section as viewed
    fn mark_section_viewed(&mut self) {
        let Some(book) = &self.state.book else { return };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return;
        };

        let book_progress = self.progress.book_mut(&book.metadata.id);
        let section_progress = book_progress.sections.entry(section.path.clone()).or_default();

        if !section_progress.viewed {
            section_progress.viewed = true;
            section_progress.last_accessed = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64),
            );
            if let Err(e) = self.progress.save() {
                tracing::warn!("Failed to save progress: {}", e);
            }
        }
    }

    /// Toggle current section's complete status
    fn toggle_section_complete(&mut self) {
        let Some(book) = &self.state.book else { return };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return;
        };

        let book_progress = self.progress.book_mut(&book.metadata.id);
        let section_progress = book_progress.sections.entry(section.path.clone()).or_default();

        section_progress.completed = !section_progress.completed;
        if section_progress.completed {
            section_progress.viewed = true;
        }
        if let Err(e) = self.progress.save() {
            tracing::warn!("Failed to save progress: {}", e);
        }
    }

    /// Mark current section as complete and navigate to next section
    fn complete_section_and_next(&mut self) {
        // Mark current section as complete
        self.mark_section_complete();

        // Navigate to next section
        self.navigate_to_next_section();
    }

    /// Mark the current section as complete (not just viewed)
    fn mark_section_complete(&mut self) {
        let Some(book) = &self.state.book else { return };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return;
        };

        let book_progress = self.progress.book_mut(&book.metadata.id);
        let section_progress = book_progress.sections.entry(section.path.clone()).or_default();

        section_progress.completed = true;
        section_progress.viewed = true;
        section_progress.last_accessed = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
        );
        if let Err(e) = self.progress.save() {
            tracing::warn!("Failed to save progress: {}", e);
        }

        self.state.command_line.set_message("Section marked complete!");
    }

    /// Navigate to the next section in the book
    fn navigate_to_next_section(&mut self) {
        let Some(book) = &self.state.book else { return };

        let current_chapter = self.state.current_chapter;
        let current_section = self.state.current_section;

        // Try next section in current chapter
        if let Some(chapter) = book.chapters.get(current_chapter) {
            if current_section + 1 < chapter.sections.len() {
                // Next section in same chapter
                self.state.current_section = current_section + 1;
            } else if current_chapter + 1 < book.chapters.len() {
                // First section of next chapter
                self.state.current_chapter = current_chapter + 1;
                self.state.current_section = 0;
                // Expand the new chapter in curriculum
                self.state.curriculum.expanded_chapters.insert(current_chapter + 1);
            } else {
                // End of book
                self.state.command_line.set_message("Congratulations! You've completed the book!");
                self.state.content.exit_footer();
                return;
            }
        } else {
            return;
        }

        // Reset scroll and footer state
        self.state.content.scroll_offset = 0;
        self.state.content.exit_footer();

        // Mark new section as viewed
        self.mark_section_viewed();

        self.state.command_line.set_message("Moving to next section...");
    }

    /// Start the quiz for current section
    fn start_quiz(&mut self) {
        let Some(book) = &self.state.book else { return };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return;
        };

        // Check for API key
        if self.state.claude.needs_setup {
            self.state.command_line.set_error("API key not set. Use :claude-key <key>");
            return;
        }

        // Get API key
        let api_key = match crate::claude::ApiKeyManager::get_api_key() {
            Ok(key) => key,
            Err(e) => {
                self.state.command_line.set_error(format!("Failed to get API key: {}", e));
                return;
            }
        };

        // Reset quiz state and set loading
        self.state.quiz.start_loading(&section.path);
        self.state.command_line.set_message("Generating quiz questions...");

        // Get section content for the prompt
        let section_title = section.title.clone();
        let section_content = section.plain_text();

        // Truncate content if too long
        let content = if section_content.len() > 6000 {
            format!("{}...\n\n[Content truncated]", &section_content[..6000])
        } else {
            section_content
        };

        // Create channel for results
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        self.quiz_rx = Some(rx);

        let model = self.state.claude.model;

        // Spawn the quiz generation task
        tokio::spawn(async move {
            let result = generate_quiz_questions(api_key, model, &section_title, &content).await;
            let _ = tx.send(result).await;
        });
    }
}

/// Generate quiz questions using Claude API
async fn generate_quiz_questions(
    api_key: String,
    model: crate::claude::ClaudeModel,
    section_title: &str,
    content: &str,
) -> QuizGenerationResult {
    use crate::claude::{ClaudeClient, CreateMessageRequest, Message};

    let client = ClaudeClient::new(api_key);

    let prompt = format!(
        r#"Based on this educational content about "{}", generate exactly 5 multiple-choice quiz questions to test comprehension.

Content:
{}

Generate your response as a JSON object with this exact structure:
{{
  "questions": [
    {{
      "question": "The question text",
      "options": ["Option A", "Option B", "Option C", "Option D"],
      "correct_index": 0
    }}
  ]
}}

Requirements:
- Exactly 5 questions
- Exactly 4 options per question
- correct_index is 0-3 indicating which option is correct
- Questions should test understanding, not just memorization
- Make questions challenging but fair based on the content provided

Respond with ONLY the JSON object, no other text."#,
        section_title, content
    );

    let messages = vec![Message::user(prompt)];
    let request =
        CreateMessageRequest::new(model, messages).with_max_tokens(2000).without_streaming();

    match client.send_message(request).await {
        Ok(response) => {
            // Extract text from response content blocks
            let text = response
                .content
                .iter()
                .filter_map(|block| block.text.as_deref())
                .collect::<Vec<_>>()
                .join("");

            // Parse JSON response
            match parse_quiz_json(&text) {
                Ok(questions) => QuizGenerationResult::Success(questions),
                Err(e) => QuizGenerationResult::Error(format!("Failed to parse quiz: {}", e)),
            }
        }
        Err(e) => QuizGenerationResult::Error(format!("API error: {}", e)),
    }
}

/// Parse quiz questions from Claude's JSON response
fn parse_quiz_json(text: &str) -> Result<Vec<crate::app::state::QuizQuestion>> {
    use crate::app::state::QuizQuestion;

    // Try to extract JSON from the response (Claude might add markdown code blocks)
    let json_str = if text.contains("```json") {
        text.split("```json").nth(1).and_then(|s| s.split("```").next()).unwrap_or(text).trim()
    } else if text.contains("```") {
        text.split("```").nth(1).and_then(|s| s.split("```").next()).unwrap_or(text).trim()
    } else {
        text.trim()
    };

    #[derive(serde::Deserialize)]
    struct QuizResponse {
        questions: Vec<QuestionJson>,
    }

    #[derive(serde::Deserialize)]
    struct QuestionJson {
        question: String,
        options: Vec<String>,
        correct_index: usize,
    }

    let response: QuizResponse = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("JSON parse error: {} in text: {}", e, json_str))?;

    if response.questions.len() != 5 {
        return Err(anyhow::anyhow!("Expected 5 questions, got {}", response.questions.len()));
    }

    let questions: Vec<QuizQuestion> = response
        .questions
        .into_iter()
        .map(|q| QuizQuestion {
            question: q.question,
            options: q.options,
            correct_index: q.correct_index,
        })
        .collect();

    Ok(questions)
}

impl App {
    /// Handle actions when quiz overlay is active
    fn handle_quiz_action(&mut self, action: Action) -> Result<bool> {
        match action {
            Action::Quit => return Ok(true),

            Action::Back => {
                // Escape closes quiz
                self.state.quiz.close();
                self.state.command_line.clear_message();
            }

            Action::Up => {
                // In question mode, move to previous option
                if !self.state.quiz.loading && !self.state.quiz.completed {
                    self.state.quiz.select_prev();
                }
            }

            Action::Down => {
                // In question mode, move to next option
                if !self.state.quiz.loading && !self.state.quiz.completed {
                    self.state.quiz.select_next();
                }
            }

            Action::Select => {
                if self.state.quiz.loading {
                    // Ignore while loading
                } else if self.state.quiz.error.is_some() {
                    // Retry on error
                    self.retry_quiz();
                } else if self.state.quiz.completed {
                    if self.state.quiz.passed() {
                        // Passed - complete section and continue
                        self.state.quiz.close();
                        self.mark_section_complete();
                        self.navigate_to_next_section();
                    } else {
                        // Failed - retry
                        self.state.quiz.retry();
                    }
                } else {
                    // Confirm current answer
                    self.state.quiz.confirm_answer();
                }
            }

            _ => {}
        }
        Ok(false)
    }

    /// Retry quiz generation after error
    fn retry_quiz(&mut self) {
        let Some(book) = &self.state.book else { return };
        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            return;
        };

        self.state.quiz.start_loading(&section.path);
        self.state.command_line.set_message("Retrying quiz generation...");
    }

    /// Start creating a new note
    fn start_creating_note(&mut self) {
        // Only allow if we have a book loaded
        let Some(book) = &self.state.book else {
            self.state.command_line.set_error("No book loaded");
            return;
        };

        // Check if we have a valid section
        if book.chapters.get(self.state.current_chapter).is_none() {
            self.state.command_line.set_error("No section selected");
            return;
        }

        self.state.notes.start_creating();
        // Show and focus the notes panel
        self.state.panel_visibility.notes = true;
        self.state.focused_panel = Panel::Notes;
    }

    /// Start editing the selected note
    fn start_editing_note(&mut self) {
        use crate::ui::notes_panel::get_selected_note;

        let Some(note) = get_selected_note(&self.state, &self.notes_store) else {
            self.state.command_line.set_error("No note selected");
            return;
        };

        self.state.notes.start_editing(&note.id, &note.content);
        self.state.panel_visibility.notes = true;
        self.state.focused_panel = Panel::Notes;
    }

    /// Delete the selected note
    fn delete_selected_note(&mut self) {
        use crate::ui::notes_panel::get_selected_note;

        let Some(note) = get_selected_note(&self.state, &self.notes_store) else {
            self.state.command_line.set_error("No note selected");
            return;
        };

        let note_id = note.id.clone();
        if self.notes_store.delete_note(&note_id) {
            if let Err(e) = self.notes_store.save() {
                tracing::warn!("Failed to save notes: {}", e);
            }
            self.state.command_line.set_message("Note deleted");
            // Reset selection if needed
            let total = crate::ui::notes_panel::get_note_count(&self.state, &self.notes_store);
            if self.state.notes.selected_index >= total && total > 0 {
                self.state.notes.selected_index = total - 1;
            }
        }
    }

    /// Save the current note being created or edited
    fn save_note(&mut self) {
        use crate::notes::Note;

        let content = self.state.notes.input.clone();
        if content.trim().is_empty() {
            self.state.notes.cancel_edit();
            return;
        }

        if self.state.notes.creating {
            // Create new note
            let Some(book) = &self.state.book else {
                self.state.notes.cancel_edit();
                return;
            };
            let Some(chapter) = book.chapters.get(self.state.current_chapter) else {
                self.state.notes.cancel_edit();
                return;
            };
            let Some(section) = chapter.sections.get(self.state.current_section) else {
                self.state.notes.cancel_edit();
                return;
            };

            let note = Note::new_section_note(&book.metadata.id, &section.path, &content);
            self.notes_store.add_note(note);

            if let Err(e) = self.notes_store.save() {
                tracing::warn!("Failed to save notes: {}", e);
            }
            self.state.command_line.set_message("Note created");
        } else if let Some(note_id) = &self.state.notes.editing.clone() {
            // Update existing note
            if self.notes_store.update_note(note_id, &content) {
                if let Err(e) = self.notes_store.save() {
                    tracing::warn!("Failed to save notes: {}", e);
                }
                self.state.command_line.set_message("Note updated");
            }
        }

        self.state.notes.cancel_edit();
    }

    /// Handle input while editing a note
    fn handle_notes_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.state.notes.cancel_edit();
            }
            KeyCode::Enter => {
                self.save_note();
            }
            KeyCode::Backspace => {
                self.state.notes.delete_char();
            }
            KeyCode::Left => {
                self.state.notes.move_left();
            }
            KeyCode::Right => {
                self.state.notes.move_right();
            }
            KeyCode::Char(c) => {
                self.state.notes.insert_char(c);
            }
            _ => {}
        }
    }

    /// Move panel focus left
    fn move_panel_focus_left(&mut self) {
        match self.state.focused_panel {
            Panel::Content => {
                if self.state.panel_visibility.curriculum {
                    self.state.focused_panel = Panel::Curriculum;
                }
            }
            Panel::Notes => {
                self.state.focused_panel = Panel::Content;
            }
            Panel::Curriculum => {}
        }
    }

    /// Move panel focus right
    fn move_panel_focus_right(&mut self) {
        match self.state.focused_panel {
            Panel::Curriculum => {
                self.state.focused_panel = Panel::Content;
            }
            Panel::Content => {
                if self.state.panel_visibility.notes {
                    self.state.focused_panel = Panel::Notes;
                }
            }
            Panel::Notes => {}
        }
    }

    /// Handle vertical navigation based on focused panel
    fn handle_vertical_navigation(&mut self, action: Action) {
        match self.state.focused_panel {
            Panel::Curriculum => self.navigate_curriculum(action),
            Panel::Content => self.navigate_content(action),
            Panel::Notes => self.navigate_notes(action),
        }
    }

    /// Navigate the notes panel
    fn navigate_notes(&mut self, action: Action) {
        use crate::ui::notes_panel::get_note_count;

        let total_notes = get_note_count(&self.state, &self.notes_store);
        if total_notes == 0 {
            return;
        }

        match action {
            Action::Up => {
                if self.state.notes.selected_index > 0 {
                    self.state.notes.selected_index -= 1;
                }
            }
            Action::Down => {
                if self.state.notes.selected_index < total_notes - 1 {
                    self.state.notes.selected_index += 1;
                }
            }
            Action::Top => {
                self.state.notes.selected_index = 0;
            }
            Action::Bottom => {
                self.state.notes.selected_index = total_notes.saturating_sub(1);
            }
            _ => {}
        }
    }

    /// Navigate the curriculum tree
    fn navigate_curriculum(&mut self, action: Action) {
        if self.state.book.is_none() {
            return;
        }

        // Calculate total items (chapters + visible sections)
        let total_items = self.calculate_curriculum_items();
        if total_items == 0 {
            return;
        }

        match action {
            Action::Up => {
                if self.state.curriculum.selected_index > 0 {
                    self.state.curriculum.selected_index -= 1;
                }
            }
            Action::Down => {
                if self.state.curriculum.selected_index < total_items - 1 {
                    self.state.curriculum.selected_index += 1;
                }
            }
            Action::Top => {
                self.state.curriculum.selected_index = 0;
            }
            Action::Bottom => {
                self.state.curriculum.selected_index = total_items.saturating_sub(1);
            }
            _ => {}
        }

        // Keep selection in view
        self.state.curriculum.ensure_selection_visible();
    }

    /// Calculate total visible items in curriculum
    fn calculate_curriculum_items(&self) -> usize {
        let Some(book) = &self.state.book else { return 0 };

        let mut count = 0;
        for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
            count += 1; // Chapter itself
            if self.state.curriculum.expanded_chapters.contains(&chapter_idx) {
                count += chapter.sections.len();
            }
        }
        count
    }

    /// Navigate content (scrolling)
    fn navigate_content(&mut self, action: Action) {
        // Handle footer-focused state
        if self.state.content.footer_focused {
            match action {
                Action::Up => {
                    // Exit footer, return to content
                    self.state.content.exit_footer();
                    self.state.command_line.clear_message();
                }
                Action::Left => {
                    // Switch to previous button
                    self.state.content.footer_prev();
                }
                Action::Right => {
                    // Switch to next button
                    self.state.content.footer_next();
                }
                _ => {}
            }
            return;
        }

        // Normal content scrolling
        match action {
            Action::Up => {
                self.state.content.scroll_offset =
                    self.state.content.scroll_offset.saturating_sub(2);
            }
            Action::Down => {
                // Check if we're at max scroll and should enter footer
                let max_scroll = self.state.content.max_scroll();
                if self.state.content.scroll_offset >= max_scroll {
                    // At bottom of content, move focus to footer
                    self.state.content.enter_footer();
                    self.state.command_line.set_message("[h/l] switch  [Enter] select  [k] back");
                } else {
                    self.state.content.scroll_offset += 2;
                }
            }
            Action::Top => {
                self.state.content.scroll_offset = 0;
            }
            Action::Bottom => {
                // Jump to max scroll position
                self.state.content.scroll_offset = self.state.content.max_scroll();
            }
            _ => {}
        }
        // Clamp to valid range
        self.state.content.clamp_scroll();
    }

    /// Handle page/half-page scrolling based on focused panel
    fn handle_scroll(&mut self, action: Action) {
        let scroll_amount = match action {
            Action::PageUp | Action::PageDown => 20,
            Action::HalfPageUp | Action::HalfPageDown => 10,
            _ => return,
        };

        match self.state.focused_panel {
            Panel::Curriculum => {
                let total_items = self.calculate_curriculum_items();
                match action {
                    Action::PageUp | Action::HalfPageUp => {
                        self.state.curriculum.selected_index =
                            self.state.curriculum.selected_index.saturating_sub(scroll_amount);
                    }
                    Action::PageDown | Action::HalfPageDown => {
                        self.state.curriculum.selected_index =
                            (self.state.curriculum.selected_index + scroll_amount)
                                .min(total_items.saturating_sub(1));
                    }
                    _ => {}
                }
                // Keep selection in view
                self.state.curriculum.ensure_selection_visible();
            }
            Panel::Content => {
                match action {
                    Action::PageUp | Action::HalfPageUp => {
                        self.state.content.scroll_offset =
                            self.state.content.scroll_offset.saturating_sub(scroll_amount);
                    }
                    Action::PageDown | Action::HalfPageDown => {
                        self.state.content.scroll_offset += scroll_amount;
                    }
                    _ => {}
                }
                // Clamp to valid range
                self.state.content.clamp_scroll();
            }
            Panel::Notes => {}
        }
    }

    /// Handle selection (Enter key)
    fn handle_select(&mut self) {
        // Handle Content panel with footer focused
        if self.state.focused_panel == Panel::Content && self.state.content.footer_focused {
            match self.state.content.footer_button_index {
                0 => {
                    // Take Quiz button
                    self.start_quiz();
                }
                1 => {
                    // Complete & Next button
                    self.complete_section_and_next();
                }
                _ => {}
            }
            return;
        }

        if self.state.focused_panel != Panel::Curriculum {
            return;
        }

        // Toggle chapter expansion or select section
        let Some(book) = &self.state.book else { return };

        let mut current_idx = 0;
        for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
            if current_idx == self.state.curriculum.selected_index {
                // Toggle chapter expansion
                if self.state.curriculum.expanded_chapters.contains(&chapter_idx) {
                    self.state.curriculum.expanded_chapters.remove(&chapter_idx);
                } else {
                    self.state.curriculum.expanded_chapters.insert(chapter_idx);
                }
                return;
            }
            current_idx += 1;

            if self.state.curriculum.expanded_chapters.contains(&chapter_idx) {
                for (section_idx, _section) in chapter.sections.iter().enumerate() {
                    if current_idx == self.state.curriculum.selected_index {
                        // Select this section
                        self.state.current_chapter = chapter_idx;
                        self.state.current_section = section_idx;
                        self.state.content.scroll_offset = 0;
                        // Move focus to content
                        self.state.focused_panel = Panel::Content;
                        // Mark as viewed
                        self.mark_section_viewed();
                        return;
                    }
                    current_idx += 1;
                }
            }
        }
    }

    /// Handle command line input, returns true if should exit
    async fn handle_command_line_input(&mut self, key: KeyCode) -> Result<bool> {
        match key {
            KeyCode::Esc => {
                self.state.command_line.exit_input_mode();
            }
            KeyCode::Enter => {
                let input = self.state.command_line.input.clone();
                let mode = self.state.command_line.mode;

                // Add to history before clearing
                self.state.command_line.add_to_history(input.clone());
                self.state.command_line.exit_input_mode();

                // Parse and execute
                match mode {
                    CommandMode::Command => {
                        return self.execute_command(&input);
                    }
                    CommandMode::Search => {
                        self.execute_search(&input);
                    }
                    CommandMode::Normal => {}
                }
            }
            KeyCode::Backspace => {
                self.state.command_line.delete_char();
            }
            KeyCode::Delete => {
                self.state.command_line.delete_char_forward();
            }
            KeyCode::Left => {
                self.state.command_line.move_left();
            }
            KeyCode::Right => {
                self.state.command_line.move_right();
            }
            KeyCode::Home => {
                self.state.command_line.move_start();
            }
            KeyCode::End => {
                self.state.command_line.move_end();
            }
            KeyCode::Up => {
                self.state.command_line.history_up();
            }
            KeyCode::Down => {
                self.state.command_line.history_down();
            }
            KeyCode::Char(c) => {
                self.state.command_line.insert_char(c);
            }
            _ => {}
        }
        Ok(false)
    }

    /// Execute a parsed command, returns true if should exit
    fn execute_command(&mut self, input: &str) -> Result<bool> {
        match parse_command(input) {
            ParseResult::Ok(cmd) => self.run_command(cmd),
            ParseResult::UnknownCommand(cmd) => {
                self.state.command_line.set_error(format!("Unknown command: {}", cmd));
                Ok(false)
            }
            ParseResult::MissingArgument(cmd) => {
                self.state.command_line.set_error(format!("Missing argument for: {}", cmd));
                Ok(false)
            }
        }
    }

    /// Run a command, returns true if should exit
    fn run_command(&mut self, cmd: Command) -> Result<bool> {
        match cmd {
            Command::Quit => Ok(true),
            Command::Help => {
                self.state.screen = Screen::Help;
                Ok(false)
            }
            Command::Add(path) => {
                self.add_book(&path)?;
                Ok(false)
            }
            Command::Open(book_id) => {
                self.open_book(&book_id)?;
                Ok(false)
            }
            Command::Remove(book_id) => {
                self.remove_book(&book_id)?;
                Ok(false)
            }
            Command::List => {
                self.list_books();
                Ok(false)
            }
            Command::Search(query) => {
                self.execute_search(&query);
                Ok(false)
            }
            Command::Goto(path) => {
                self.goto_section(&path);
                Ok(false)
            }
            Command::Nop => Ok(false),
            Command::ClaudeSetup => {
                self.start_claude_setup();
                Ok(false)
            }
            Command::ClaudeKey(key) => {
                match crate::claude::ApiKeyManager::set_api_key(&key) {
                    Ok(()) => {
                        self.state.claude.needs_setup = false;
                        self.state.command_line.set_message("API key saved to keyring");
                    }
                    Err(e) => {
                        self.state.command_line.set_error(e.to_string());
                    }
                }
                Ok(false)
            }
            Command::ClaudeModel(model_str) => {
                if let Some(model) = crate::claude::ClaudeModel::parse(&model_str) {
                    self.set_claude_model(model);
                } else {
                    self.state.command_line.set_error(format!(
                        "Unknown model: {}. Options: haiku, sonnet35, sonnet, opus",
                        model_str
                    ));
                }
                Ok(false)
            }
            Command::ClaudeClear => {
                self.state.claude.clear_streaming();
                self.state.claude.clear_error();
                self.state.command_line.set_message("Claude state cleared");
                Ok(false)
            }
            Command::Ask(question) => {
                self.ask_claude(&question);
                Ok(false)
            }
            Command::Explain(topic) => {
                self.explain_section(topic.as_deref());
                Ok(false)
            }
            Command::AskSelection(question) => {
                self.ask_about_selection(&question);
                Ok(false)
            }
        }
    }

    /// Send a question to Claude and start streaming the response
    fn ask_claude(&mut self, question: &str) {
        // Check if already streaming
        if self.state.claude.streaming {
            self.state.command_line.set_error("Already waiting for Claude response");
            return;
        }

        // Check for API key
        if self.state.claude.needs_setup {
            self.state.command_line.set_error("API key not set. Use :claude-key <key>");
            return;
        }

        // Get API key
        let api_key = match crate::claude::ApiKeyManager::get_api_key() {
            Ok(key) => key,
            Err(e) => {
                self.state.command_line.set_error(format!("Failed to get API key: {}", e));
                return;
            }
        };

        // Clear previous response and set streaming state
        self.state.claude.clear_streaming();
        self.state.claude.streaming = true;
        self.state.command_line.set_message("Asking Claude...");

        // Create the client and message
        let client = crate::claude::ClaudeClient::new(api_key);
        let messages = vec![crate::claude::Message::user(question)];
        let request = crate::claude::CreateMessageRequest::new(self.state.claude.model, messages)
            .with_system("You are a helpful assistant for a book reader application. Answer questions concisely.");

        // Create channel and cancellation token
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let cancel_token = tokio_util::sync::CancellationToken::new();

        self.claude_rx = Some(rx);
        self.claude_cancel = Some(cancel_token.clone());

        // Spawn the streaming task
        tokio::spawn(async move {
            if let Err(e) = client.send_streaming(request, tx, cancel_token).await {
                tracing::error!("Claude API error: {}", e);
            }
        });
    }

    /// Explain the current section using Claude
    fn explain_section(&mut self, topic: Option<&str>) {
        // Check if already streaming
        if self.state.claude.streaming {
            self.state.command_line.set_error("Already waiting for Claude response");
            return;
        }

        // Check for API key
        if self.state.claude.needs_setup {
            self.state.command_line.set_error("API key not set. Use :claude-key <key>");
            return;
        }

        // Get current section content
        let Some(book) = &self.state.book else {
            self.state.command_line.set_error("No book loaded");
            return;
        };

        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            self.state.command_line.set_error("No section selected");
            return;
        };

        let section_title = section.title.clone();
        let section_content = section.plain_text();

        // Truncate content if too long (Claude has context limits)
        let content = if section_content.len() > 8000 {
            format!("{}...\n\n[Content truncated]", &section_content[..8000])
        } else {
            section_content
        };

        // Get API key
        let api_key = match crate::claude::ApiKeyManager::get_api_key() {
            Ok(key) => key,
            Err(e) => {
                self.state.command_line.set_error(format!("Failed to get API key: {}", e));
                return;
            }
        };

        // Build the prompt
        let prompt = if let Some(focus) = topic {
            format!(
                "Here is a section from a book titled \"{}\":\n\n{}\n\nPlease explain {} in this context. Be concise.",
                section_title, content, focus
            )
        } else {
            format!(
                "Here is a section from a book titled \"{}\":\n\n{}\n\nPlease provide a brief explanation of the key concepts in this section. Be concise.",
                section_title, content
            )
        };

        // Clear previous response and set streaming state
        self.state.claude.clear_streaming();
        self.state.claude.streaming = true;
        self.state.command_line.set_message("Asking Claude to explain...");

        // Create the client and message
        let client = crate::claude::ClaudeClient::new(api_key);
        let messages = vec![crate::claude::Message::user(&prompt)];
        let request = crate::claude::CreateMessageRequest::new(self.state.claude.model, messages)
            .with_system("You are an expert tutor helping someone understand technical content from a book. Explain concepts clearly and concisely.");

        // Create channel and cancellation token
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let cancel_token = tokio_util::sync::CancellationToken::new();

        self.claude_rx = Some(rx);
        self.claude_cancel = Some(cancel_token.clone());

        // Spawn the streaming task
        tokio::spawn(async move {
            if let Err(e) = client.send_streaming(request, tx, cancel_token).await {
                tracing::error!("Claude API error: {}", e);
            }
        });
    }

    /// Get currently selected text (if in visual mode with selection)
    fn get_selected_text(&self) -> Option<String> {
        if !self.state.visual_mode.active {
            return None;
        }

        let book = self.state.book.as_ref()?;
        let section = book.get_section(self.state.current_chapter, self.state.current_section)?;

        let (start_block, start_char, end_block, end_char) = self
            .state
            .visual_mode
            .selection_range(self.state.content.cursor_block, self.state.content.cursor_char);

        // Helper to extract text from a block
        fn block_text(block: &crate::book::ContentBlock) -> Option<String> {
            match block {
                crate::book::ContentBlock::Paragraph(text) => Some(text.clone()),
                crate::book::ContentBlock::Blockquote(text) => Some(text.clone()),
                crate::book::ContentBlock::Heading { text, .. } => Some(text.clone()),
                crate::book::ContentBlock::Code(code_block) => Some(code_block.code.clone()),
                crate::book::ContentBlock::UnorderedList(items) => Some(items.join("\n")),
                crate::book::ContentBlock::OrderedList(items) => Some(items.join("\n")),
                _ => None,
            }
        }

        // For single-block selection
        if start_block == end_block {
            let text = block_text(section.content.get(start_block)?)?;
            Some(text.chars().skip(start_char).take(end_char - start_char).collect())
        } else {
            // Multi-block selection: collect text from all blocks
            let mut result = String::new();
            for block_idx in start_block..=end_block {
                let Some(block) = section.content.get(block_idx) else { continue };
                let Some(text) = block_text(block) else { continue };

                if block_idx == start_block {
                    result.push_str(&text.chars().skip(start_char).collect::<String>());
                } else if block_idx == end_block {
                    result.push_str(&text.chars().take(end_char).collect::<String>());
                } else {
                    result.push_str(&text);
                }
                result.push('\n');
            }
            Some(result.trim().to_string())
        }
    }

    /// Copy selected text to clipboard (yank)
    fn yank_selection(&mut self) {
        let Some(text) = self.get_selected_text() else {
            self.state.command_line.set_error("No text selected");
            return;
        };

        if text.is_empty() {
            self.state.command_line.set_error("No text selected");
            return;
        }

        match arboard::Clipboard::new() {
            Ok(mut clipboard) => match clipboard.set_text(&text) {
                Ok(()) => {
                    let char_count = text.chars().count();
                    self.state
                        .command_line
                        .set_message(format!("Yanked {} characters", char_count));
                    // Exit visual mode after yanking
                    self.state.visual_mode.exit();
                }
                Err(e) => {
                    self.state.command_line.set_error(format!("Failed to copy: {}", e));
                }
            },
            Err(e) => {
                self.state.command_line.set_error(format!("Clipboard unavailable: {}", e));
            }
        }
    }

    /// Ask Claude about selected text (includes full section context)
    fn ask_about_selection(&mut self, question: &str) {
        // Check if already streaming
        if self.state.claude.streaming {
            self.state.command_line.set_error("Already waiting for Claude response");
            return;
        }

        // Check for API key
        if self.state.claude.needs_setup {
            self.state.command_line.set_error("API key not set. Use :claude-key <key>");
            return;
        }

        // Get book and section info for context
        let Some(book) = &self.state.book else {
            self.state.command_line.set_error("No book loaded");
            return;
        };

        let Some(section) =
            book.get_section(self.state.current_chapter, self.state.current_section)
        else {
            self.state.command_line.set_error("No section selected");
            return;
        };

        // Get full section content for context
        let section_content = section.plain_text();
        let section_title = section.title.clone();
        let section_path = section.path.clone();
        let book_id = book.metadata.id.clone();

        // Get selected text and selection info
        let (selected_text, selection_block, selection_char) = if self.state.visual_mode.active {
            let (start_block, start_char, _, _) = self
                .state
                .visual_mode
                .selection_range(self.state.content.cursor_block, self.state.content.cursor_char);
            match self.get_selected_text() {
                Some(text) if !text.is_empty() => (text, Some(start_block), Some(start_char)),
                _ => {
                    self.state.command_line.set_error(
                        "No text selected. Use 'v' to enter visual mode and select text.",
                    );
                    return;
                }
            }
        } else {
            self.state
                .command_line
                .set_error("No text selected. Use 'v' to enter visual mode and select text.");
            return;
        };

        // Get API key
        let api_key = match crate::claude::ApiKeyManager::get_api_key() {
            Ok(key) => key,
            Err(e) => {
                self.state.command_line.set_error(format!("Failed to get API key: {}", e));
                return;
            }
        };

        // Truncate section content if too long (but preserve full selection)
        let context = if section_content.len() > 6000 {
            format!("{}...\n\n[Section truncated for context]", &section_content[..6000])
        } else {
            section_content
        };

        // Truncate selection display if too long
        let selection_display = if selected_text.len() > 2000 {
            format!("{}...", &selected_text[..2000])
        } else {
            selected_text.clone()
        };

        // Build the prompt with full context and highlighted selection
        let prompt = format!(
            "Here is a section from the book titled \"{}\":\n\n---\n{}\n---\n\n\
             The reader has highlighted this specific passage:\n\n\"\"\"\n{}\n\"\"\"\n\n\
             Their question about this passage: {}\n\n\
             Please provide a clear, concise answer that considers both the highlighted passage and its surrounding context.",
            section_title, context, selection_display, question
        );

        // Store pending note info for saving Q&A after response
        self.state.claude.set_pending_note(
            question,
            &book_id,
            &section_path,
            Some(&selected_text),
            selection_block,
            selection_char,
        );

        // Exit visual mode
        self.state.visual_mode.exit();

        // Clear previous response and set streaming state
        self.state.claude.clear_streaming();
        self.state.claude.streaming = true;
        self.state
            .command_line
            .set_message(format!("Asking about selection ({} chars)...", selected_text.len()));

        // Create the client and message
        let client = crate::claude::ClaudeClient::new(api_key);
        let messages = vec![crate::claude::Message::user(&prompt)];
        let request = crate::claude::CreateMessageRequest::new(self.state.claude.model, messages)
            .with_system("You are an expert tutor helping someone understand content from a book. Answer questions about the selected passage clearly and concisely, using the surrounding context to provide more complete explanations when relevant.");

        // Create channel and cancellation token
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let cancel_token = tokio_util::sync::CancellationToken::new();

        self.claude_rx = Some(rx);
        self.claude_cancel = Some(cancel_token.clone());

        // Spawn the streaming task
        tokio::spawn(async move {
            if let Err(e) = client.send_streaming(request, tx, cancel_token).await {
                tracing::error!("Claude API error: {}", e);
            }
        });
    }

    /// Expand tilde in path to home directory
    fn expand_path(path: &std::path::Path) -> std::path::PathBuf {
        let path_str = path.to_string_lossy();
        if let Some(stripped) = path_str.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return std::path::PathBuf::from(home).join(stripped);
            }
        } else if path_str == "~" {
            if let Ok(home) = std::env::var("HOME") {
                return std::path::PathBuf::from(home);
            }
        }
        path.to_path_buf()
    }

    /// Add a book from path
    fn add_book(&mut self, path: &std::path::Path) -> Result<()> {
        let expanded_path = Self::expand_path(path);
        match storage::add_book(&expanded_path) {
            Ok(entry) => {
                // Load the book using the entry we just created
                match storage::load_book(&entry) {
                    Ok(book) => {
                        let title = book.metadata.title.clone();
                        self.state.book = Some(book);
                        self.state.current_chapter = 0;
                        self.state.current_section = 0;
                        self.state.curriculum.selected_index = 0;
                        self.state.curriculum.expanded_chapters.clear();
                        self.state.content.scroll_offset = 0;
                        self.state.command_line.set_message(format!("Added: {}", title));
                    }
                    Err(e) => {
                        self.state.command_line.set_error(format!("Failed to load: {}", e));
                    }
                }
            }
            Err(e) => {
                self.state.command_line.set_error(format!("Failed to add: {}", e));
            }
        }
        Ok(())
    }

    /// Remove a book from the library
    fn remove_book(&mut self, book_id: &str) -> Result<()> {
        // Load library
        let mut library = match storage::Library::load() {
            Ok(lib) => lib,
            Err(e) => {
                self.state.command_line.set_error(format!("Failed to load library: {}", e));
                return Ok(());
            }
        };

        // Find the book by ID or title
        let entry_idx = library
            .entries
            .iter()
            .position(|e| e.metadata.id == book_id || e.metadata.title == book_id);

        match entry_idx {
            Some(idx) => {
                let removed_title = library.entries[idx].metadata.title.clone();
                let removed_id = library.entries[idx].metadata.id.clone();

                // Remove from library
                library.entries.remove(idx);
                if let Err(e) = library.save() {
                    self.state.command_line.set_error(format!("Failed to save library: {}", e));
                    return Ok(());
                }

                // If we removed the currently loaded book, clear it
                if let Some(book) = &self.state.book {
                    if book.metadata.id == removed_id {
                        self.state.book = None;
                        self.state.current_chapter = 0;
                        self.state.current_section = 0;
                        self.state.curriculum.selected_index = 0;
                        self.state.curriculum.expanded_chapters.clear();
                        self.state.content.scroll_offset = 0;
                    }
                }

                self.state.command_line.set_message(format!("Removed: {}", removed_title));
            }
            None => {
                self.state.command_line.set_error(format!("Book not found: {}", book_id));
            }
        }
        Ok(())
    }

    /// Open a book by ID
    fn open_book(&mut self, book_id: &str) -> Result<()> {
        // Load library and find the entry
        let library = match storage::Library::load() {
            Ok(lib) => lib,
            Err(e) => {
                self.state.command_line.set_error(format!("Failed to load library: {}", e));
                return Ok(());
            }
        };

        // Try to find by ID first, then by title
        let entry = library.find_by_id(book_id).or_else(|| library.find_by_title(book_id));

        match entry {
            Some(entry) => match storage::load_book(entry) {
                Ok(book) => {
                    let title = book.metadata.title.clone();
                    let loaded_book_id = book.metadata.id.clone();
                    self.state.book = Some(book);

                    // Apply saved session state if available
                    if let Some(book_session) = self.session.book(&loaded_book_id) {
                        self.state.current_chapter = book_session.current_chapter;
                        self.state.current_section = book_session.current_section;
                        self.state.content.scroll_offset = book_session.content_scroll_offset;
                        self.state.curriculum.selected_index = book_session.selected_index;
                        self.state.curriculum.scroll_offset = book_session.curriculum_scroll_offset;
                        self.state.curriculum.expanded_chapters =
                            book_session.expanded_chapters.clone();
                    } else {
                        self.state.current_chapter = 0;
                        self.state.current_section = 0;
                        self.state.curriculum.selected_index = 0;
                        self.state.curriculum.expanded_chapters.clear();
                        self.state.content.scroll_offset = 0;
                    }
                    self.state.command_line.set_message(format!("Opened: {}", title));
                }
                Err(e) => {
                    self.state.command_line.set_error(format!("Failed to load: {}", e));
                }
            },
            None => {
                self.state.command_line.set_error(format!("Book not found: {}", book_id));
            }
        }
        Ok(())
    }

    /// List available books
    fn list_books(&mut self) {
        match storage::Library::load() {
            Ok(library) => {
                if library.entries.is_empty() {
                    self.state.command_line.set_message("No books. Use :add <path> to add one.");
                } else {
                    let names: Vec<_> =
                        library.entries.iter().map(|e| e.metadata.id.as_str()).collect();
                    self.state.command_line.set_message(format!("Books: {}", names.join(", ")));
                }
            }
            Err(_) => {
                self.state.command_line.set_message("No books. Use :add <path> to add one.");
            }
        }
    }

    /// Execute a search
    fn execute_search(&mut self, query: &str) {
        if query.is_empty() {
            self.state.search.active = false;
            self.state.search.query.clear();
            self.state.command_line.clear_message();
        } else {
            self.state.search.active = true;
            self.state.search.query = query.to_string();
            // TODO: Implement actual search highlighting
            self.state.command_line.set_message(format!("Search: {}", query));
        }
    }

    /// Go to a specific section
    fn goto_section(&mut self, path: &str) {
        let Some(book) = &self.state.book else {
            self.state.command_line.set_error("No book loaded");
            return;
        };

        // Try to find section by path
        for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
            for (section_idx, section) in chapter.sections.iter().enumerate() {
                if section.path.contains(path)
                    || section.title.to_lowercase().contains(&path.to_lowercase())
                {
                    self.state.current_chapter = chapter_idx;
                    self.state.current_section = section_idx;
                    self.state.content.scroll_offset = 0;
                    self.state.curriculum.expanded_chapters.insert(chapter_idx);
                    self.state.command_line.set_message(format!("→ {}", section.title));
                    self.mark_section_viewed();
                    return;
                }
            }
        }

        self.state.command_line.set_error(format!("Section not found: {}", path));
    }

    // ==================== Claude Integration ====================

    /// Process pending Claude streaming events (non-blocking)
    fn process_claude_events(&mut self) {
        // Collect events first to avoid borrow conflict
        let events: Vec<_> = if let Some(ref mut rx) = self.claude_rx {
            let mut collected = Vec::new();
            while let Ok(event) = rx.try_recv() {
                collected.push(event);
            }
            collected
        } else {
            Vec::new()
        };

        // Now process collected events
        for event in events {
            self.handle_claude_event(event);
        }
    }

    /// Process pending quiz generation results (non-blocking)
    fn process_quiz_events(&mut self) {
        // Check for quiz generation result
        if let Some(ref mut rx) = self.quiz_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    QuizGenerationResult::Success(questions) => {
                        self.state.quiz.set_questions(questions);
                        self.state
                            .command_line
                            .set_message("Quiz ready! Use j/k to select, Enter to confirm.");
                    }
                    QuizGenerationResult::Error(message) => {
                        self.state.quiz.set_error(&message);
                        self.state.command_line.set_error(format!("Quiz error: {}", message));
                    }
                }
                self.quiz_rx = None;
            }
        }
    }

    /// Handle a single Claude streaming event
    fn handle_claude_event(&mut self, event: crate::claude::StreamEvent) {
        use crate::claude::StreamEvent;

        match event {
            StreamEvent::ContentBlockDelta { text } => {
                self.state.claude.stream_buffer.push_str(&text);
            }
            StreamEvent::MessageStop => {
                // Response complete - finalize and show the response panel
                self.state.claude.finalize_response();

                // Create note from Q&A if pending info exists
                if self.state.claude.has_pending_note() {
                    self.save_claude_qa_as_note();
                }

                self.state
                    .command_line
                    .set_message("Response ready (press 'c' to toggle, Esc to close)");
                self.claude_rx = None;
                self.claude_cancel = None;
            }
            StreamEvent::Error { message } => {
                self.state.claude.set_error(&message);
                self.state.claude.streaming = false;
                self.state.claude.clear_pending_note(); // Clear pending on error
                self.claude_rx = None;
                self.claude_cancel = None;
            }
            StreamEvent::MessageStart { .. } => {
                // Response started
                self.state.claude.clear_error();
            }
            _ => {
                // Ignore other events (Ping, ContentBlockStart/Stop, MessageDelta)
            }
        }
    }

    /// Save the completed Claude Q&A as a note
    fn save_claude_qa_as_note(&mut self) {
        use crate::notes::Note;

        // Extract pending note info
        let question = match &self.state.claude.pending_question {
            Some(q) => q.clone(),
            None => return,
        };
        let book_id = match &self.state.claude.pending_book_id {
            Some(id) => id.clone(),
            None => return,
        };
        let section_path = match &self.state.claude.pending_section_path {
            Some(path) => path.clone(),
            None => return,
        };
        let answer = self.state.claude.response.clone();

        // Get optional selection info
        let selected_text = self.state.claude.pending_selection.clone();
        let selection_block = self.state.claude.pending_selection_block;
        let selection_char = self.state.claude.pending_selection_char;

        // Create the note
        let note = Note::new_claude_note(
            &book_id,
            &section_path,
            &question,
            &answer,
            selection_block,
            selection_char,
            selected_text.as_deref(),
        );

        // Add to store and save
        self.notes_store.add_note(note);
        if let Err(e) = self.notes_store.save() {
            tracing::warn!("Failed to save Claude Q&A note: {}", e);
        }

        // Clear pending note info
        self.state.claude.clear_pending_note();

        // Show notes panel so user can see the saved Q&A
        self.state.panel_visibility.notes = true;
    }

    /// Cancel the current Claude streaming request
    fn cancel_claude_stream(&mut self) {
        if let Some(token) = self.claude_cancel.take() {
            token.cancel();
        }
        self.state.claude.streaming = false;
        self.state.claude.stream_buffer.clear();
        self.state.command_line.set_message("Request cancelled");
        self.claude_rx = None;
    }

    /// Handle input when Claude response panel is visible
    fn handle_claude_panel_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state.claude.hide_response();
            }
            KeyCode::Char('c') => {
                self.state.claude.toggle_response();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.state.claude.scroll_response_down(1, 1000);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.state.claude.scroll_response_up(1);
            }
            KeyCode::Char('d') | KeyCode::PageDown => {
                self.state.claude.scroll_response_down(10, 1000);
            }
            KeyCode::Char('u') | KeyCode::PageUp => {
                self.state.claude.scroll_response_up(10);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.state.claude.response_scroll = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.state.claude.scroll_response_down(10000, 10000);
            }
            _ => {}
        }
    }

    /// Start Claude setup wizard
    fn start_claude_setup(&mut self) {
        self.state.claude.start_setup();
        self.state.command_line.set_message("Claude API Setup");
    }

    /// Handle saving API key from setup wizard
    #[allow(dead_code)]
    fn save_claude_api_key(&mut self) {
        let key = self.state.claude.setup_api_key.trim();
        if key.is_empty() {
            self.state.claude.set_error("API key cannot be empty");
            return;
        }

        match crate::claude::ApiKeyManager::set_api_key(key) {
            Ok(()) => {
                self.state.claude.next_setup_step();
                self.state.command_line.set_message("API key saved");
            }
            Err(e) => {
                self.state.claude.set_error(e.to_string());
            }
        }
    }

    /// Set the Claude model
    fn set_claude_model(&mut self, model: crate::claude::ClaudeModel) {
        self.state.claude.model = model;
        self.state
            .command_line
            .set_message(format!("Claude model set to {}", model.display_name()));
    }
}

/// Truncate a string to a maximum length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.restore_terminal();
    }
}

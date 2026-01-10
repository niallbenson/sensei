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
        };

        // Apply saved panel widths from session
        app.state.panel_visibility.curriculum_width_percent = app.session.curriculum_width_percent;
        app.state.panel_visibility.notes_width_percent = app.session.notes_width_percent;

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

            // Handle all pending events before next redraw (makes scrolling feel faster)
            let mut should_quit = false;
            while event::poll(std::time::Duration::from_millis(0))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        // Route to notes input if editing a note
                        if self.state.notes.is_editing() {
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
                            // Handle : and / to enter command modes
                            match key_event.code {
                                KeyCode::Char(':') => {
                                    self.state.command_line.enter_command_mode();
                                }
                                KeyCode::Char('/') => {
                                    self.state.command_line.enter_search_mode();
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
            Action::Left => {
                self.move_panel_focus_left();
            }
            Action::Right => {
                self.move_panel_focus_right();
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
        match action {
            Action::Up => {
                self.state.content.scroll_offset =
                    self.state.content.scroll_offset.saturating_sub(2);
            }
            Action::Down => {
                self.state.content.scroll_offset += 2;
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
        }
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

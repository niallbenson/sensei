//! Application state and event handling

pub mod command;
pub mod input;
pub mod state;

use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::book::storage;
use crate::config::{Config, progress::Progress};
use crate::ui;
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

    /// Terminal backend
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Result<Self> {
        let terminal = Self::setup_terminal()?;
        let progress = Progress::load().unwrap_or_default();

        let mut app = Self { config, state: AppState::default(), progress, terminal };

        // Auto-load first book from library if available
        app.auto_load_book();

        Ok(app)
    }

    /// Auto-load the first book from the library
    fn auto_load_book(&mut self) {
        if let Ok(library) = storage::Library::load() {
            if let Some(entry) = library.entries.first() {
                if let Ok(book) = storage::load_book(entry) {
                    self.state.book = Some(book);
                    self.state.current_chapter = 0;
                    self.state.current_section = 0;
                }
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
            self.terminal.draw(|frame| {
                ui::draw(frame, state, config, progress);
            })?;

            // Handle all pending events before next redraw (makes scrolling feel faster)
            let mut should_quit = false;
            while event::poll(std::time::Duration::from_millis(0))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        // Route to command line if in input mode
                        if self.state.command_line.is_input_mode() {
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
        match action {
            Action::Quit => return Ok(true),

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

            _ => {}
        }
        Ok(false)
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
            let _ = self.progress.save();
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
        let _ = self.progress.save();
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
            Panel::Notes => {}
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
                    self.state.book = Some(book);
                    self.state.current_chapter = 0;
                    self.state.current_section = 0;
                    self.state.curriculum.selected_index = 0;
                    self.state.curriculum.expanded_chapters.clear();
                    self.state.content.scroll_offset = 0;
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

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.restore_terminal();
    }
}

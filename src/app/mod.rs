//! Application state and event handling

pub mod input;
pub mod state;

use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::config::{Config, progress::Progress};
use crate::ui;
use input::{Action, key_with_modifier_to_action};
use state::{AppState, Panel, Screen};

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

        Ok(Self { config, state: AppState::default(), progress, terminal })
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
            // Draw UI
            self.terminal.draw(|frame| {
                ui::draw(frame, &self.state, &self.config, &self.progress);
            })?;

            // Handle events
            if event::poll(std::time::Duration::from_millis(16))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        if let Some(action) =
                            key_with_modifier_to_action(key_event.code, key_event.modifiers)
                        {
                            match self.handle_action(action).await {
                                Ok(true) => break, // Exit requested
                                Ok(false) => {}    // Continue
                                Err(e) => {
                                    tracing::error!("Error handling action: {}", e);
                                }
                            }
                        }
                    }
                }
            }

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
            _ => {
                if action == Action::Quit {
                    return Ok(true);
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
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
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
        self.state.curriculum.scroll_offset =
            self.state.curriculum.scroll_offset.min(self.state.curriculum.selected_index);
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
                    self.state.content.scroll_offset.saturating_sub(1);
            }
            Action::Down => {
                self.state.content.scroll_offset += 1;
            }
            Action::Top => {
                self.state.content.scroll_offset = 0;
            }
            Action::Bottom => {
                // Set to a large value; rendering will clamp it
                self.state.content.scroll_offset = usize::MAX / 2;
            }
            _ => {}
        }
    }

    /// Handle page/half-page scrolling
    fn handle_scroll(&mut self, action: Action) {
        let scroll_amount = match action {
            Action::PageUp | Action::PageDown => 20,
            Action::HalfPageUp | Action::HalfPageDown => 10,
            _ => return,
        };

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
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.restore_terminal();
    }
}

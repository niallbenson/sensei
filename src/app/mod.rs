//! Application state and event handling

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

use crate::config::Config;
use crate::ui;
use state::{AppState, Screen};

/// The main application
pub struct App {
    /// Application configuration
    config: Config,

    /// Current application state
    state: AppState,

    /// Terminal backend
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Result<Self> {
        let terminal = Self::setup_terminal()?;

        Ok(Self { config, state: AppState::default(), terminal })
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
                ui::draw(frame, &self.state, &self.config);
            })?;

            // Handle events
            if event::poll(std::time::Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match self.handle_key(key.code).await {
                            Ok(true) => break, // Exit requested
                            Ok(false) => {}    // Continue
                            Err(e) => {
                                tracing::error!("Error handling key: {}", e);
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

    /// Handle a key press, returns true if should exit
    async fn handle_key(&mut self, key: KeyCode) -> Result<bool> {
        match &self.state.screen {
            Screen::Landing => {
                // Any key progresses from landing
                self.state.screen = Screen::Main;
            }
            Screen::Main => {
                if key == KeyCode::Char('q') {
                    return Ok(true);
                }
                // TODO: Add more key handlers
            }
            _ => {
                // Handle other screens
                if key == KeyCode::Char('q') {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.restore_terminal();
    }
}

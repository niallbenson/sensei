//! Sensei - A beautiful TUI for interactive technical book learning
//!
//! Sensei helps you learn from technical books through adaptive quizzing,
//! contextual Q&A, and integrated note-taking, all powered by Claude.

pub mod app;
pub mod book;
pub mod config;
pub mod export;
pub mod learning;
pub mod notes;
pub mod syntax;
pub mod theme;
pub mod ui;

pub use app::App;
pub use config::Config;
pub use theme::Theme;

//! Session state persistence
//!
//! Stores UI state between sessions so users can resume where they left off.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::Config;

/// Session state for a specific book
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookSession {
    /// Expanded chapter indices
    pub expanded_chapters: HashSet<usize>,
    /// Selected index in curriculum view
    pub selected_index: usize,
    /// Current chapter being viewed
    pub current_chapter: usize,
    /// Current section being viewed
    pub current_section: usize,
    /// Scroll offset in content view
    pub content_scroll_offset: usize,
    /// Scroll offset in curriculum view
    pub curriculum_scroll_offset: usize,
}

/// All session state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    /// Currently open book ID (if any)
    pub current_book_id: Option<String>,
    /// Session state per book (key is book ID)
    pub books: std::collections::HashMap<String, BookSession>,
    /// Curriculum panel width percentage (10-50)
    #[serde(default = "default_curriculum_width")]
    pub curriculum_width_percent: u16,
    /// Notes panel width percentage (10-50)
    #[serde(default = "default_notes_width")]
    pub notes_width_percent: u16,
}

fn default_curriculum_width() -> u16 {
    20
}

fn default_notes_width() -> u16 {
    25
}

impl Session {
    /// Load session from disk
    pub fn load() -> Result<Self> {
        let path = Self::session_path()?;

        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read session from {:?}", path))?;
            serde_json::from_str(&contents).with_context(|| "Failed to parse session.json")
        } else {
            Ok(Self::default())
        }
    }

    /// Save session to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::session_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create data directory {:?}", parent))?;
        }

        let contents =
            serde_json::to_string_pretty(self).with_context(|| "Failed to serialize session")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write session to {:?}", path))?;

        Ok(())
    }

    /// Get the path to the session file
    fn session_path() -> Result<PathBuf> {
        Ok(Config::data_dir()?.join("session.json"))
    }

    /// Get or create session for a book
    pub fn book_mut(&mut self, book_id: &str) -> &mut BookSession {
        self.books.entry(book_id.to_string()).or_default()
    }

    /// Get session for a book (if exists)
    pub fn book(&self, book_id: &str) -> Option<&BookSession> {
        self.books.get(book_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_default_is_empty() {
        let session = Session::default();
        assert!(session.current_book_id.is_none());
        assert!(session.books.is_empty());
    }

    #[test]
    fn book_mut_creates_entry() {
        let mut session = Session::default();
        let book_session = session.book_mut("test-book");
        book_session.current_chapter = 5;

        assert!(session.books.contains_key("test-book"));
        assert_eq!(session.books["test-book"].current_chapter, 5);
    }

    #[test]
    fn session_serializes() {
        let mut session = Session::default();
        session.current_book_id = Some("my-book".into());
        let book = session.book_mut("my-book");
        book.expanded_chapters.insert(0);
        book.expanded_chapters.insert(2);
        book.selected_index = 3;

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("my-book"));
    }

    #[test]
    fn session_deserializes() {
        let json = r#"{
            "current_book_id": "test",
            "books": {
                "test": {
                    "expanded_chapters": [1, 2],
                    "selected_index": 5,
                    "current_chapter": 1,
                    "current_section": 2,
                    "content_scroll_offset": 100,
                    "curriculum_scroll_offset": 10
                }
            }
        }"#;

        let session: Session = serde_json::from_str(json).unwrap();
        assert_eq!(session.current_book_id, Some("test".into()));
        let book = session.book("test").unwrap();
        assert_eq!(book.selected_index, 5);
        assert_eq!(book.content_scroll_offset, 100);
    }
}

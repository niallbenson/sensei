//! Note-taking functionality
//!
//! This module provides note storage and management for book content.
//! Notes can be attached at the section level or to specific text selections.

pub mod model;
pub mod storage;

// Re-exports
pub use model::{Note, NoteAnchor, NoteSource};
pub use storage::NotesStore;

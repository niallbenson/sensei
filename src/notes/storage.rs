//! Note persistence

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::model::{Note, NoteAnchor};
use crate::config::Config;

/// All notes organized by book
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotesStore {
    /// Notes per book (book_id -> list of notes)
    pub books: HashMap<String, Vec<Note>>,
}

impl NotesStore {
    /// Load notes from disk
    pub fn load() -> Result<Self> {
        let path = Self::notes_path()?;

        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read notes from {:?}", path))?;
            serde_json::from_str(&contents).with_context(|| "Failed to parse notes.json")
        } else {
            Ok(Self::default())
        }
    }

    /// Save notes to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::notes_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create data directory {:?}", parent))?;
        }

        let contents =
            serde_json::to_string_pretty(self).with_context(|| "Failed to serialize notes")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write notes to {:?}", path))?;

        Ok(())
    }

    /// Get path to notes.json
    fn notes_path() -> Result<PathBuf> {
        Ok(Config::data_dir()?.join("notes.json"))
    }

    /// Get all notes for a book
    pub fn get_book_notes(&self, book_id: &str) -> &[Note] {
        self.books.get(book_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get notes for a specific section
    pub fn get_section_notes(&self, book_id: &str, section_path: &str) -> Vec<&Note> {
        self.books
            .get(book_id)
            .map(|notes| notes.iter().filter(|n| n.section_path == section_path).collect())
            .unwrap_or_default()
    }

    /// Get only section-level notes for a section
    pub fn get_section_level_notes(&self, book_id: &str, section_path: &str) -> Vec<&Note> {
        self.get_section_notes(book_id, section_path)
            .into_iter()
            .filter(|n| n.is_section_note())
            .collect()
    }

    /// Get only text-selection notes for a section
    pub fn get_selection_notes(&self, book_id: &str, section_path: &str) -> Vec<&Note> {
        self.get_section_notes(book_id, section_path)
            .into_iter()
            .filter(|n| n.is_selection_note())
            .collect()
    }

    /// Get note anchors for highlighting (text ranges only)
    pub fn get_note_anchors(&self, book_id: &str, section_path: &str) -> Vec<&NoteAnchor> {
        self.get_selection_notes(book_id, section_path)
            .into_iter()
            .map(|n| &n.anchor)
            .collect()
    }

    /// Add a note
    pub fn add_note(&mut self, note: Note) {
        self.books.entry(note.book_id.clone()).or_default().push(note);
    }

    /// Update an existing note by ID
    pub fn update_note(&mut self, note_id: &str, new_content: &str) -> bool {
        for notes in self.books.values_mut() {
            if let Some(note) = notes.iter_mut().find(|n| n.id == note_id) {
                note.update_content(new_content);
                return true;
            }
        }
        false
    }

    /// Delete a note by ID
    pub fn delete_note(&mut self, note_id: &str) -> bool {
        for notes in self.books.values_mut() {
            let len_before = notes.len();
            notes.retain(|n| n.id != note_id);
            if notes.len() < len_before {
                return true;
            }
        }
        false
    }

    /// Get a note by ID
    pub fn get_note(&self, note_id: &str) -> Option<&Note> {
        for notes in self.books.values() {
            if let Some(note) = notes.iter().find(|n| n.id == note_id) {
                return Some(note);
            }
        }
        None
    }

    /// Get a mutable reference to a note by ID
    pub fn get_note_mut(&mut self, note_id: &str) -> Option<&mut Note> {
        for notes in self.books.values_mut() {
            if let Some(note) = notes.iter_mut().find(|n| n.id == note_id) {
                return Some(note);
            }
        }
        None
    }

    /// Count total notes
    pub fn total_count(&self) -> usize {
        self.books.values().map(|v| v.len()).sum()
    }

    /// Count notes for a book
    pub fn book_count(&self, book_id: &str) -> usize {
        self.books.get(book_id).map(|v| v.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_note(book_id: &str, section_path: &str, content: &str) -> Note {
        Note::new_section_note(book_id, section_path, content)
    }

    #[test]
    fn add_and_get_note() {
        let mut store = NotesStore::default();
        let note = create_test_note("book1", "ch01/s01", "Test note");
        let note_id = note.id.clone();

        store.add_note(note);

        assert_eq!(store.total_count(), 1);
        assert!(store.get_note(&note_id).is_some());
    }

    #[test]
    fn get_section_notes() {
        let mut store = NotesStore::default();

        store.add_note(create_test_note("book1", "ch01/s01", "Note 1"));
        store.add_note(create_test_note("book1", "ch01/s01", "Note 2"));
        store.add_note(create_test_note("book1", "ch01/s02", "Note 3"));
        store.add_note(create_test_note("book2", "ch01/s01", "Note 4"));

        let section_notes = store.get_section_notes("book1", "ch01/s01");
        assert_eq!(section_notes.len(), 2);

        let section_notes = store.get_section_notes("book1", "ch01/s02");
        assert_eq!(section_notes.len(), 1);

        let section_notes = store.get_section_notes("book2", "ch01/s01");
        assert_eq!(section_notes.len(), 1);
    }

    #[test]
    fn update_note() {
        let mut store = NotesStore::default();
        let note = create_test_note("book1", "ch01/s01", "Original");
        let note_id = note.id.clone();

        store.add_note(note);
        assert!(store.update_note(&note_id, "Updated"));

        let updated = store.get_note(&note_id).unwrap();
        assert_eq!(updated.content, "Updated");
    }

    #[test]
    fn delete_note() {
        let mut store = NotesStore::default();
        let note = create_test_note("book1", "ch01/s01", "To delete");
        let note_id = note.id.clone();

        store.add_note(note);
        assert_eq!(store.total_count(), 1);

        assert!(store.delete_note(&note_id));
        assert_eq!(store.total_count(), 0);
        assert!(store.get_note(&note_id).is_none());
    }

    #[test]
    fn delete_nonexistent_note() {
        let mut store = NotesStore::default();
        assert!(!store.delete_note("nonexistent"));
    }

    #[test]
    fn section_vs_selection_notes() {
        let mut store = NotesStore::default();

        // Section note
        store.add_note(create_test_note("book1", "ch01/s01", "Section note"));

        // Selection note
        let selection_note =
            Note::new_selection_note("book1", "ch01/s01", "Selection note", 0, 10, "some text");
        store.add_note(selection_note);

        let section_only = store.get_section_level_notes("book1", "ch01/s01");
        assert_eq!(section_only.len(), 1);
        assert!(section_only[0].is_section_note());

        let selection_only = store.get_selection_notes("book1", "ch01/s01");
        assert_eq!(selection_only.len(), 1);
        assert!(selection_only[0].is_selection_note());
    }

    #[test]
    fn get_note_anchors() {
        let mut store = NotesStore::default();

        // Section note (should not appear in anchors)
        store.add_note(create_test_note("book1", "ch01/s01", "Section note"));

        // Selection notes
        store.add_note(Note::new_selection_note("book1", "ch01/s01", "Note 1", 0, 10, "text1"));
        store.add_note(Note::new_selection_note("book1", "ch01/s01", "Note 2", 2, 20, "text2"));

        let anchors = store.get_note_anchors("book1", "ch01/s01");
        assert_eq!(anchors.len(), 2);
    }

    #[test]
    fn book_count() {
        let mut store = NotesStore::default();

        store.add_note(create_test_note("book1", "ch01/s01", "Note 1"));
        store.add_note(create_test_note("book1", "ch01/s02", "Note 2"));
        store.add_note(create_test_note("book2", "ch01/s01", "Note 3"));

        assert_eq!(store.book_count("book1"), 2);
        assert_eq!(store.book_count("book2"), 1);
        assert_eq!(store.book_count("book3"), 0);
    }
}

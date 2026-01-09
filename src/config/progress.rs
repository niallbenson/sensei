//! Progress tracking for book learning

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::Config;

/// Progress data for a single section
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectionProgress {
    /// Has the user viewed this section?
    pub viewed: bool,

    /// Has the user marked this as complete?
    pub completed: bool,

    /// Pre-quiz score (0-100), if taken
    pub quiz_score: Option<u8>,

    /// Number of questions asked about this section
    pub questions_asked: u32,

    /// Timestamp of last access
    pub last_accessed: Option<i64>,
}

/// Progress data for an entire book
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookProgress {
    /// Book identifier
    pub book_id: String,

    /// Progress per section (key is section path like "ch01/section02")
    pub sections: HashMap<String, SectionProgress>,

    /// Overall quiz average
    pub overall_quiz_average: Option<f32>,

    /// Total time spent (seconds)
    pub total_time_seconds: u64,
}

/// All progress data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    /// Progress per book
    pub books: HashMap<String, BookProgress>,
}

impl Progress {
    /// Load progress from disk
    pub fn load() -> Result<Self> {
        let path = Self::progress_path()?;

        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read progress from {:?}", path))?;
            serde_json::from_str(&contents).with_context(|| "Failed to parse progress.json")
        } else {
            Ok(Self::default())
        }
    }

    /// Save progress to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::progress_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create data directory {:?}", parent))?;
        }

        let contents =
            serde_json::to_string_pretty(self).with_context(|| "Failed to serialize progress")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write progress to {:?}", path))?;

        Ok(())
    }

    /// Get progress path
    fn progress_path() -> Result<PathBuf> {
        Ok(Config::data_dir()?.join("progress.json"))
    }

    /// Get or create book progress
    pub fn book_mut(&mut self, book_id: &str) -> &mut BookProgress {
        self.books
            .entry(book_id.to_string())
            .or_insert_with(|| BookProgress { book_id: book_id.to_string(), ..Default::default() })
    }

    /// Calculate weak areas (sections with low quiz scores or many questions)
    pub fn weak_areas(&self) -> Vec<(String, String, u8)> {
        let mut weak = Vec::new();

        for (book_id, book_progress) in &self.books {
            for (section_id, section) in &book_progress.sections {
                if let Some(score) = section.quiz_score {
                    if score < 70 {
                        weak.push((book_id.clone(), section_id.clone(), score));
                    }
                }
            }
        }

        weak.sort_by_key(|(_, _, score)| *score);
        weak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_progress_is_empty() {
        let progress = Progress::default();
        assert!(progress.books.is_empty());
    }

    #[test]
    fn book_mut_creates_entry_if_missing() {
        let mut progress = Progress::default();
        let book = progress.book_mut("test-book");
        assert_eq!(book.book_id, "test-book");
        assert!(progress.books.contains_key("test-book"));
    }

    #[test]
    fn weak_areas_identifies_low_scores() {
        let mut progress = Progress::default();
        let book = progress.book_mut("test-book");

        book.sections.insert(
            "ch01".to_string(),
            SectionProgress { quiz_score: Some(50), ..Default::default() },
        );
        book.sections.insert(
            "ch02".to_string(),
            SectionProgress { quiz_score: Some(90), ..Default::default() },
        );

        let weak = progress.weak_areas();
        assert_eq!(weak.len(), 1);
        assert_eq!(weak[0].2, 50);
    }
}

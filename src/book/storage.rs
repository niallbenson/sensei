//! Book storage and caching
//!
//! Handles persisting parsed books and managing the book library.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::epub::parse_epub_file;
use super::markdown::parse_markdown_directory;
use super::model::{Book, BookMetadata, BookSource};
use crate::config::Config;

/// Library entry with cache metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// Book metadata (lightweight)
    pub metadata: BookMetadata,
    /// Unix timestamp of when the cache was created
    pub cached_at: i64,
    /// File modification time of the source (for invalidation)
    pub source_mtime: Option<i64>,
}

/// The book library
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Library {
    /// All books in the library
    pub entries: Vec<LibraryEntry>,
}

impl Library {
    /// Load library from disk
    pub fn load() -> Result<Self> {
        let path = Self::library_path()?;

        if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read library from {:?}", path))?;
            serde_json::from_str(&contents).with_context(|| "Failed to parse library.json")
        } else {
            Ok(Self::default())
        }
    }

    /// Save library to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::library_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create library directory {:?}", parent))?;
        }

        let contents =
            serde_json::to_string_pretty(self).with_context(|| "Failed to serialize library")?;

        fs::write(&path, contents)
            .with_context(|| format!("Failed to write library to {:?}", path))?;

        Ok(())
    }

    /// Get library path
    fn library_path() -> Result<PathBuf> {
        Ok(Config::data_dir()?.join("library.json"))
    }

    /// Find a book by ID
    pub fn find_by_id(&self, id: &str) -> Option<&LibraryEntry> {
        self.entries.iter().find(|e| e.metadata.id == id)
    }

    /// Find a book by title (case-insensitive partial match)
    pub fn find_by_title(&self, query: &str) -> Option<&LibraryEntry> {
        let query_lower = query.to_lowercase();
        self.entries.iter().find(|e| e.metadata.title.to_lowercase().contains(&query_lower))
    }

    /// Add or update a book in the library
    pub fn upsert(&mut self, entry: LibraryEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.metadata.id == entry.metadata.id)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Remove a book by ID
    pub fn remove(&mut self, id: &str) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.metadata.id != id);
        self.entries.len() < len_before
    }

    /// List all books
    pub fn list(&self) -> &[LibraryEntry] {
        &self.entries
    }
}

/// Get the books directory
pub fn books_dir() -> Result<PathBuf> {
    let dir = Config::data_dir()?.join("books");
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create books directory {:?}", dir))?;
    Ok(dir)
}

/// Get the cache directory for a specific book
fn book_cache_dir(book_id: &str) -> Result<PathBuf> {
    let dir = books_dir()?.join(book_id);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create book cache directory {:?}", dir))?;
    Ok(dir)
}

/// Get the path to the cached book JSON
fn book_cache_path(book_id: &str) -> Result<PathBuf> {
    Ok(book_cache_dir(book_id)?.join("parsed.json"))
}

/// Check if cache is valid for a source path
fn is_cache_valid(cached_mtime: Option<i64>, source_path: &Path) -> bool {
    let Some(cached) = cached_mtime else {
        return false;
    };

    let source_mtime = get_source_mtime(source_path);
    source_mtime.is_some_and(|m| m <= cached)
}

/// Get modification time of a source (file or directory)
fn get_source_mtime(path: &Path) -> Option<i64> {
    if path.is_file() {
        fs::metadata(path)
            .ok()?
            .modified()
            .ok()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64)
    } else if path.is_dir() {
        // For directories, find the most recent modification
        let mut latest: Option<i64> = None;

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(dur) = mtime.duration_since(SystemTime::UNIX_EPOCH) {
                            let t = dur.as_secs() as i64;
                            latest = Some(latest.map_or(t, |l| l.max(t)));
                        }
                    }
                }
            }
        }

        latest
    } else {
        None
    }
}

/// Load a book from cache if valid, otherwise parse and cache
pub fn load_book(entry: &LibraryEntry) -> Result<Book> {
    let source_path = match &entry.metadata.source {
        BookSource::Epub(p) => p.clone(),
        BookSource::Markdown(p) => p.clone(),
    };

    // Check if cache is valid
    let cache_path = book_cache_path(&entry.metadata.id)?;
    if cache_path.exists() && is_cache_valid(entry.source_mtime, &source_path) {
        // Load from cache
        let contents = fs::read_to_string(&cache_path)
            .with_context(|| format!("Failed to read cached book from {:?}", cache_path))?;
        return serde_json::from_str(&contents).with_context(|| "Failed to parse cached book");
    }

    // Parse the book
    let book = match &entry.metadata.source {
        BookSource::Epub(path) => parse_epub_file(path)?,
        BookSource::Markdown(path) => parse_markdown_directory(path)?,
    };

    // Cache the parsed book
    let contents =
        serde_json::to_string_pretty(&book).with_context(|| "Failed to serialize book")?;
    fs::write(&cache_path, contents)
        .with_context(|| format!("Failed to write book cache to {:?}", cache_path))?;

    Ok(book)
}

/// Add a book to the library from a source path
pub fn add_book(source_path: &Path) -> Result<LibraryEntry> {
    let source_path =
        source_path.canonicalize().with_context(|| format!("Invalid path: {:?}", source_path))?;

    // Determine source type and parse
    let book = if source_path.is_dir() {
        parse_markdown_directory(&source_path)?
    } else if source_path.extension().is_some_and(|ext| ext == "epub") {
        parse_epub_file(&source_path)?
    } else {
        anyhow::bail!(
            "Unsupported source type. Expected directory (markdown) or .epub file: {:?}",
            source_path
        );
    };

    // Create library entry
    let now =
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);

    let entry = LibraryEntry {
        metadata: book.metadata.clone(),
        cached_at: now,
        source_mtime: get_source_mtime(&source_path),
    };

    // Cache the parsed book
    let cache_path = book_cache_path(&entry.metadata.id)?;
    let contents =
        serde_json::to_string_pretty(&book).with_context(|| "Failed to serialize book")?;
    fs::write(&cache_path, contents)
        .with_context(|| format!("Failed to write book cache to {:?}", cache_path))?;

    // Update library
    let mut library = Library::load()?;
    library.upsert(entry.clone());
    library.save()?;

    Ok(entry)
}

/// Remove a book from the library
pub fn remove_book(book_id: &str) -> Result<bool> {
    let mut library = Library::load()?;

    if library.remove(book_id) {
        library.save()?;

        // Remove cache directory
        if let Ok(cache_dir) = book_cache_dir(book_id) {
            let _ = fs::remove_dir_all(cache_dir);
        }

        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn library_default_is_empty() {
        let library = Library::default();
        assert!(library.entries.is_empty());
    }

    #[test]
    fn library_find_by_id() {
        let mut library = Library::default();
        library.entries.push(LibraryEntry {
            metadata: BookMetadata {
                id: "test-book".into(),
                title: "Test Book".into(),
                author: None,
                source: BookSource::Markdown(PathBuf::from("/test")),
                language: None,
                description: None,
                cover_image: None,
                added_at: 0,
                last_accessed: None,
            },
            cached_at: 0,
            source_mtime: None,
        });

        assert!(library.find_by_id("test-book").is_some());
        assert!(library.find_by_id("nonexistent").is_none());
    }

    #[test]
    fn library_find_by_title() {
        let mut library = Library::default();
        library.entries.push(LibraryEntry {
            metadata: BookMetadata {
                id: "test-book".into(),
                title: "The Rust Programming Language".into(),
                author: None,
                source: BookSource::Markdown(PathBuf::from("/test")),
                language: None,
                description: None,
                cover_image: None,
                added_at: 0,
                last_accessed: None,
            },
            cached_at: 0,
            source_mtime: None,
        });

        assert!(library.find_by_title("rust").is_some());
        assert!(library.find_by_title("PROGRAMMING").is_some());
        assert!(library.find_by_title("python").is_none());
    }

    #[test]
    fn library_upsert() {
        let mut library = Library::default();

        let entry1 = LibraryEntry {
            metadata: BookMetadata {
                id: "test".into(),
                title: "Version 1".into(),
                author: None,
                source: BookSource::Markdown(PathBuf::from("/test")),
                language: None,
                description: None,
                cover_image: None,
                added_at: 0,
                last_accessed: None,
            },
            cached_at: 1,
            source_mtime: None,
        };

        library.upsert(entry1);
        assert_eq!(library.entries.len(), 1);
        assert_eq!(library.entries[0].metadata.title, "Version 1");

        let entry2 = LibraryEntry {
            metadata: BookMetadata {
                id: "test".into(),
                title: "Version 2".into(),
                author: None,
                source: BookSource::Markdown(PathBuf::from("/test")),
                language: None,
                description: None,
                cover_image: None,
                added_at: 0,
                last_accessed: None,
            },
            cached_at: 2,
            source_mtime: None,
        };

        library.upsert(entry2);
        assert_eq!(library.entries.len(), 1);
        assert_eq!(library.entries[0].metadata.title, "Version 2");
    }

    #[test]
    fn library_remove() {
        let mut library = Library::default();
        library.entries.push(LibraryEntry {
            metadata: BookMetadata {
                id: "test".into(),
                title: "Test".into(),
                author: None,
                source: BookSource::Markdown(PathBuf::from("/test")),
                language: None,
                description: None,
                cover_image: None,
                added_at: 0,
                last_accessed: None,
            },
            cached_at: 0,
            source_mtime: None,
        });

        assert!(library.remove("test"));
        assert!(library.entries.is_empty());
        assert!(!library.remove("nonexistent"));
    }

    #[test]
    fn get_source_mtime_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "# Test").unwrap();

        let mtime = get_source_mtime(&file_path);
        assert!(mtime.is_some());
        assert!(mtime.unwrap() > 0);
    }

    #[test]
    fn is_cache_valid_checks_mtime() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "# Test").unwrap();

        let mtime = get_source_mtime(&file_path).unwrap();

        // Cache with same mtime should be valid
        assert!(is_cache_valid(Some(mtime), &file_path));

        // Cache with older mtime should be invalid
        assert!(!is_cache_valid(Some(mtime - 1000), &file_path));

        // No cached mtime should be invalid
        assert!(!is_cache_valid(None, &file_path));
    }
}

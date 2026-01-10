//! Content model for books
//!
//! This module defines the core data structures for representing technical books.
//! The model supports both EPUB and Markdown sources with a unified representation.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Source type for a book
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BookSource {
    /// EPUB file
    Epub(PathBuf),
    /// Directory of Markdown files
    Markdown(PathBuf),
}

/// Metadata about a book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMetadata {
    /// Unique identifier for the book
    pub id: String,
    /// Display title
    pub title: String,
    /// Author name(s)
    pub author: Option<String>,
    /// Source location
    pub source: BookSource,
    /// Language (e.g., "en")
    pub language: Option<String>,
    /// Description or summary
    pub description: Option<String>,
    /// Cover image path (relative to book directory)
    pub cover_image: Option<String>,
    /// Unix timestamp of when the book was added
    pub added_at: i64,
    /// Unix timestamp of last access
    pub last_accessed: Option<i64>,
}

/// A complete parsed book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    /// Book metadata
    pub metadata: BookMetadata,
    /// Chapters in order
    pub chapters: Vec<Chapter>,
}

impl Book {
    /// Create a new book with the given metadata
    pub fn new(metadata: BookMetadata) -> Self {
        Self { metadata, chapters: Vec::new() }
    }

    /// Get total section count across all chapters
    pub fn section_count(&self) -> usize {
        self.chapters.iter().map(|c| c.sections.len()).sum()
    }

    /// Get a section by chapter and section index
    pub fn get_section(&self, chapter_idx: usize, section_idx: usize) -> Option<&Section> {
        self.chapters.get(chapter_idx).and_then(|c| c.sections.get(section_idx))
    }

    /// Find a section by its path (e.g., "ch01/section02")
    pub fn find_section_by_path(&self, path: &str) -> Option<&Section> {
        for chapter in &self.chapters {
            for section in &chapter.sections {
                if section.path == path {
                    return Some(section);
                }
            }
        }
        None
    }
}

/// A chapter in a book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    /// Chapter title
    pub title: String,
    /// Chapter number (1-indexed, for display). None for unnumbered chapters (e.g., Foreword)
    pub number: Option<usize>,
    /// Path identifier (e.g., "ch01")
    pub path: String,
    /// Sections within this chapter
    pub sections: Vec<Section>,
}

impl Chapter {
    /// Create a new numbered chapter
    pub fn new(title: impl Into<String>, number: usize, path: impl Into<String>) -> Self {
        Self { title: title.into(), number: Some(number), path: path.into(), sections: Vec::new() }
    }

    /// Create a new unnumbered chapter (e.g., Foreword, Introduction)
    pub fn new_unnumbered(title: impl Into<String>, path: impl Into<String>) -> Self {
        Self { title: title.into(), number: None, path: path.into(), sections: Vec::new() }
    }
}

/// A section within a chapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section title
    pub title: String,
    /// Section number within chapter (1-indexed)
    pub number: usize,
    /// Full path identifier (e.g., "ch01/section02")
    pub path: String,
    /// Content blocks
    pub content: Vec<ContentBlock>,
    /// Estimated reading time in minutes
    pub reading_time_minutes: Option<u32>,
}

impl Section {
    /// Create a new section
    pub fn new(title: impl Into<String>, number: usize, path: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            number,
            path: path.into(),
            content: Vec::new(),
            reading_time_minutes: None,
        }
    }

    /// Calculate estimated reading time based on word count
    pub fn calculate_reading_time(&mut self) {
        let word_count: usize = self.content.iter().map(|block| block.word_count()).sum();
        // Average reading speed: 200 words per minute for technical content
        self.reading_time_minutes = Some((word_count / 200).max(1) as u32);
    }

    /// Get plain text content for search/quiz generation
    pub fn plain_text(&self) -> String {
        self.content.iter().filter_map(|block| block.plain_text()).collect::<Vec<_>>().join("\n\n")
    }
}

/// A block of content within a section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    /// A heading (level 1-6)
    Heading { level: u8, text: String },
    /// A paragraph of text
    Paragraph(String),
    /// A code block with optional language annotation
    Code(CodeBlock),
    /// An unordered list
    UnorderedList(Vec<String>),
    /// An ordered list
    OrderedList(Vec<String>),
    /// A blockquote
    Blockquote(String),
    /// An image reference (cannot display in TUI, but preserved)
    Image { alt: String, src: String },
    /// A horizontal rule
    HorizontalRule,
    /// A table
    Table(Table),
    // Note: Inline code within text is handled in Paragraph with markdown.
    // This enum is for standalone code blocks only.
}

impl ContentBlock {
    /// Estimate word count for this block
    pub fn word_count(&self) -> usize {
        match self {
            ContentBlock::Heading { text, .. } => text.split_whitespace().count(),
            ContentBlock::Paragraph(text) => text.split_whitespace().count(),
            ContentBlock::Code(code) => code.code.split_whitespace().count() / 3, // Code reads slower
            ContentBlock::UnorderedList(items) | ContentBlock::OrderedList(items) => {
                items.iter().map(|s| s.split_whitespace().count()).sum()
            }
            ContentBlock::Blockquote(text) => text.split_whitespace().count(),
            ContentBlock::Image { .. } => 0,
            ContentBlock::HorizontalRule => 0,
            ContentBlock::Table(table) => table.word_count(),
        }
    }

    /// Get plain text representation (if applicable)
    pub fn plain_text(&self) -> Option<String> {
        match self {
            ContentBlock::Heading { text, .. } => Some(text.clone()),
            ContentBlock::Paragraph(text) => Some(text.clone()),
            ContentBlock::Code(code) => Some(code.code.clone()),
            ContentBlock::UnorderedList(items) | ContentBlock::OrderedList(items) => {
                Some(items.join("\n"))
            }
            ContentBlock::Blockquote(text) => Some(text.clone()),
            ContentBlock::Image { alt, .. } => {
                if alt.is_empty() {
                    None
                } else {
                    Some(format!("[Image: {}]", alt))
                }
            }
            ContentBlock::HorizontalRule => None,
            ContentBlock::Table(table) => Some(table.plain_text()),
        }
    }
}

/// A code block with language annotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    /// The actual code content
    pub code: String,
    /// Programming language (for syntax highlighting)
    pub language: Option<String>,
    /// Optional filename or label
    pub filename: Option<String>,
    /// Line numbers to highlight (if any)
    pub highlight_lines: Vec<usize>,
}

impl CodeBlock {
    /// Create a new code block
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into(), language: None, filename: None, highlight_lines: Vec::new() }
    }

    /// Set the language
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set the filename
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }
}

/// A table structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    /// Header row
    pub headers: Vec<String>,
    /// Data rows
    pub rows: Vec<Vec<String>>,
    /// Column alignments
    pub alignments: Vec<Alignment>,
}

impl Table {
    /// Create a new table
    pub fn new(headers: Vec<String>) -> Self {
        let col_count = headers.len();
        Self { headers, rows: Vec::new(), alignments: vec![Alignment::Left; col_count] }
    }

    /// Word count for the table
    pub fn word_count(&self) -> usize {
        let header_words: usize = self.headers.iter().map(|s| s.split_whitespace().count()).sum();
        let row_words: usize =
            self.rows.iter().flat_map(|row| row.iter().map(|s| s.split_whitespace().count())).sum();
        header_words + row_words
    }

    /// Plain text representation
    pub fn plain_text(&self) -> String {
        let mut lines = vec![self.headers.join(" | ")];
        for row in &self.rows {
            lines.push(row.join(" | "));
        }
        lines.join("\n")
    }
}

/// Column alignment for tables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_section_count() {
        let mut book = Book::new(BookMetadata {
            id: "test".into(),
            title: "Test Book".into(),
            author: None,
            source: BookSource::Markdown(PathBuf::from("/test")),
            language: None,
            description: None,
            cover_image: None,
            added_at: 0,
            last_accessed: None,
        });

        let mut ch1 = Chapter::new("Chapter 1", 1, "ch01");
        ch1.sections.push(Section::new("Section 1", 1, "ch01/s01"));
        ch1.sections.push(Section::new("Section 2", 2, "ch01/s02"));
        book.chapters.push(ch1);

        let mut ch2 = Chapter::new("Chapter 2", 2, "ch02");
        ch2.sections.push(Section::new("Section 1", 1, "ch02/s01"));
        book.chapters.push(ch2);

        assert_eq!(book.section_count(), 3);
    }

    #[test]
    fn find_section_by_path() {
        let mut book = Book::new(BookMetadata {
            id: "test".into(),
            title: "Test Book".into(),
            author: None,
            source: BookSource::Markdown(PathBuf::from("/test")),
            language: None,
            description: None,
            cover_image: None,
            added_at: 0,
            last_accessed: None,
        });

        let mut ch1 = Chapter::new("Chapter 1", 1, "ch01");
        ch1.sections.push(Section::new("Target Section", 1, "ch01/target"));
        book.chapters.push(ch1);

        let section = book.find_section_by_path("ch01/target");
        assert!(section.is_some());
        assert_eq!(section.unwrap().title, "Target Section");
    }

    #[test]
    fn content_block_word_count() {
        let para = ContentBlock::Paragraph("This is a test paragraph with seven words.".into());
        assert_eq!(para.word_count(), 8);

        let code = ContentBlock::Code(CodeBlock::new("fn main() { }"));
        assert!(code.word_count() < 3); // Code counts less
    }

    #[test]
    fn code_block_builder() {
        let code =
            CodeBlock::new("println!(\"Hello\");").with_language("rust").with_filename("main.rs");

        assert_eq!(code.language, Some("rust".into()));
        assert_eq!(code.filename, Some("main.rs".into()));
    }

    #[test]
    fn section_plain_text() {
        let mut section = Section::new("Test", 1, "test");
        section.content.push(ContentBlock::Heading { level: 1, text: "Hello".into() });
        section.content.push(ContentBlock::Paragraph("World".into()));
        section.content.push(ContentBlock::HorizontalRule);

        let text = section.plain_text();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn table_word_count() {
        let mut table = Table::new(vec!["Name".into(), "Value".into()]);
        table.rows.push(vec!["foo".into(), "bar".into()]);
        table.rows.push(vec!["baz".into(), "qux".into()]);

        assert_eq!(table.word_count(), 6); // 2 headers + 4 cells
    }
}

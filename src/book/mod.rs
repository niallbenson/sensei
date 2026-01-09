//! Book handling and parsing
//!
//! This module provides functionality for parsing and managing technical books
//! from EPUB and Markdown sources.

pub mod epub;
pub mod markdown;
pub mod model;
pub mod storage;

pub use model::{
    Alignment, Book, BookMetadata, BookSource, Chapter, CodeBlock, ContentBlock, Section, Table,
};
pub use storage::{add_book, load_book, remove_book, Library, LibraryEntry};

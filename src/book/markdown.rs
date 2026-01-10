//! Markdown parser for technical books
//!
//! Parses markdown files and directories into the unified content model.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::model::{
    Alignment, Book, BookMetadata, BookSource, Chapter, CodeBlock, ContentBlock, Section, Table,
};

/// Parse a markdown string into content blocks
#[allow(clippy::cognitive_complexity)]
pub fn parse_markdown_content(markdown: &str) -> Vec<ContentBlock> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = Parser::new_ext(markdown, options);
    let mut blocks = Vec::new();

    let mut current_text = String::default();
    let mut in_code_block = false;
    let mut code_language: Option<String> = None;
    let mut code_content = String::default();

    let mut in_list = false;
    let mut list_items: Vec<String> = Vec::new();
    let mut list_ordered = false;
    let mut current_list_item = String::default();

    let mut in_blockquote = false;
    let mut blockquote_content = String::default();

    let mut in_table = false;
    let mut table_headers: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::default();
    let mut table_alignments: Vec<Alignment> = Vec::new();

    let mut current_heading_level: Option<u8> = None;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_text(&mut current_text, &mut blocks);
                current_heading_level = Some(heading_level_to_u8(level));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = current_heading_level.take() {
                    let text = std::mem::take(&mut current_text).trim().to_string();
                    if !text.is_empty() {
                        blocks.push(ContentBlock::Heading { level, text });
                    }
                }
            }

            Event::Start(Tag::Paragraph) => {
                // Starting a new paragraph
            }
            Event::End(TagEnd::Paragraph) => {
                if in_blockquote {
                    blockquote_content.push_str(&current_text);
                    blockquote_content.push('\n');
                    current_text.clear();
                } else if in_list {
                    current_list_item.push_str(&current_text);
                    current_text.clear();
                } else {
                    flush_text(&mut current_text, &mut blocks);
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                flush_text(&mut current_text, &mut blocks);
                in_code_block = true;
                code_language = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.to_string();
                        if lang.is_empty() { None } else { Some(lang) }
                    }
                    CodeBlockKind::Indented => None,
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                let code = std::mem::take(&mut code_content);
                let mut code_block = CodeBlock::new(code.trim_end());
                if let Some(lang) = code_language.take() {
                    code_block = code_block.with_language(lang);
                }
                blocks.push(ContentBlock::Code(code_block));
            }

            Event::Start(Tag::List(first_item)) => {
                flush_text(&mut current_text, &mut blocks);
                in_list = true;
                list_ordered = first_item.is_some();
                list_items.clear();
            }
            Event::End(TagEnd::List(_)) => {
                in_list = false;
                let items = std::mem::take(&mut list_items);
                if !items.is_empty() {
                    if list_ordered {
                        blocks.push(ContentBlock::OrderedList(items));
                    } else {
                        blocks.push(ContentBlock::UnorderedList(items));
                    }
                }
            }

            Event::Start(Tag::Item) => {
                current_list_item.clear();
            }
            Event::End(TagEnd::Item) => {
                let item = std::mem::take(&mut current_list_item).trim().to_string();
                if !item.is_empty() {
                    list_items.push(item);
                }
            }

            Event::Start(Tag::BlockQuote(_)) => {
                flush_text(&mut current_text, &mut blocks);
                in_blockquote = true;
                blockquote_content.clear();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_blockquote = false;
                let content = std::mem::take(&mut blockquote_content).trim().to_string();
                if !content.is_empty() {
                    blocks.push(ContentBlock::Blockquote(content));
                }
            }

            Event::Start(Tag::Table(alignments)) => {
                flush_text(&mut current_text, &mut blocks);
                in_table = true;
                table_headers.clear();
                table_rows.clear();
                table_alignments = alignments.iter().map(|a| convert_alignment(*a)).collect();
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                let mut table = Table::new(std::mem::take(&mut table_headers));
                table.rows = std::mem::take(&mut table_rows);
                table.alignments = std::mem::take(&mut table_alignments);
                blocks.push(ContentBlock::Table(table));
            }

            Event::Start(Tag::TableHead) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                // pulldown-cmark emits header cells directly inside TableHead without TableRow
                // So we capture them here when TableHead ends
                if !current_row.is_empty() {
                    table_headers = std::mem::take(&mut current_row);
                }
            }

            Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                // Data rows (headers are handled in TableHead)
                if !current_row.is_empty() {
                    table_rows.push(std::mem::take(&mut current_row));
                }
            }

            Event::Start(Tag::TableCell) => {
                current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                current_row.push(std::mem::take(&mut current_cell).trim().to_string());
            }

            Event::Start(Tag::Image { dest_url, title, .. }) => {
                flush_text(&mut current_text, &mut blocks);
                blocks.push(ContentBlock::Image {
                    alt: title.to_string(),
                    src: dest_url.to_string(),
                });
            }

            Event::Rule => {
                flush_text(&mut current_text, &mut blocks);
                blocks.push(ContentBlock::HorizontalRule);
            }

            Event::Text(text) => {
                if in_code_block {
                    code_content.push_str(&text);
                } else if in_table {
                    current_cell.push_str(&text);
                } else if in_list {
                    current_list_item.push_str(&text);
                } else if in_blockquote {
                    blockquote_content.push_str(&text);
                } else {
                    current_text.push_str(&text);
                }
            }

            Event::Code(code) => {
                // Inline code - wrap in backticks for display
                if in_table {
                    current_cell.push('`');
                    current_cell.push_str(&code);
                    current_cell.push('`');
                } else if in_list {
                    current_list_item.push('`');
                    current_list_item.push_str(&code);
                    current_list_item.push('`');
                } else {
                    current_text.push('`');
                    current_text.push_str(&code);
                    current_text.push('`');
                }
            }

            Event::SoftBreak | Event::HardBreak => {
                if in_code_block {
                    code_content.push('\n');
                } else if in_list {
                    current_list_item.push(' ');
                } else if in_blockquote {
                    blockquote_content.push('\n');
                } else {
                    current_text.push(' ');
                }
            }

            // Ignore emphasis/strong markers for plain text extraction
            Event::Start(Tag::Emphasis)
            | Event::End(TagEnd::Emphasis)
            | Event::Start(Tag::Strong)
            | Event::End(TagEnd::Strong)
            | Event::Start(Tag::Strikethrough)
            | Event::End(TagEnd::Strikethrough) => {}

            // Links - extract text only
            Event::Start(Tag::Link { .. }) | Event::End(TagEnd::Link) => {}

            // Handle HTML - extract text content, ignore tags
            Event::Html(html) | Event::InlineHtml(html) => {
                // Extract any visible text from HTML, skip tags
                let text = html
                    .replace("&vert;", "|")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&")
                    .replace("&quot;", "\"");

                // Strip HTML tags but keep content
                let mut result = String::new();
                let mut in_tag = false;
                for c in text.chars() {
                    if c == '<' {
                        in_tag = true;
                    } else if c == '>' {
                        in_tag = false;
                    } else if !in_tag {
                        result.push(c);
                    }
                }

                let trimmed = result.trim();
                if !trimmed.is_empty() {
                    if in_table {
                        current_cell.push_str(trimmed);
                    } else if in_list {
                        current_list_item.push_str(trimmed);
                    } else {
                        current_text.push_str(trimmed);
                    }
                }
            }

            _ => {}
        }
    }

    // Flush any remaining text
    flush_text(&mut current_text, &mut blocks);

    blocks
}

fn flush_text(text: &mut String, blocks: &mut Vec<ContentBlock>) {
    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        blocks.push(ContentBlock::Paragraph(trimmed));
    }
    text.clear();
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn convert_alignment(align: pulldown_cmark::Alignment) -> Alignment {
    match align {
        pulldown_cmark::Alignment::None | pulldown_cmark::Alignment::Left => Alignment::Left,
        pulldown_cmark::Alignment::Center => Alignment::Center,
        pulldown_cmark::Alignment::Right => Alignment::Right,
    }
}

/// Parse a single markdown file into a section
pub fn parse_markdown_file(path: &Path, section_number: usize) -> Result<Section> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read markdown file: {}", path.display()))?;

    let filename = path.file_stem().unwrap_or_default().to_string_lossy();

    // Try to extract title from first heading
    let blocks = parse_markdown_content(&content);
    let title = blocks
        .iter()
        .find_map(|b| {
            if let ContentBlock::Heading { level: 1, text } = b { Some(text.clone()) } else { None }
        })
        .unwrap_or_else(|| filename.to_string());

    let section_path = path
        .file_stem()
        .map_or_else(|| format!("section{}", section_number), |s| s.to_string_lossy().to_string());

    let mut section = Section::new(title, section_number, section_path);
    section.content = blocks;
    section.calculate_reading_time();

    Ok(section)
}

/// Parse a directory of markdown files into a book
pub fn parse_markdown_directory(path: &Path) -> Result<Book> {
    let path = path.canonicalize().with_context(|| format!("Invalid path: {}", path.display()))?;

    // Check for SUMMARY.md (mdbook format)
    let summary_path = path.join("SUMMARY.md");
    if summary_path.exists() {
        return parse_mdbook_directory(&path, &summary_path);
    }

    // Generate book ID from directory name
    let book_id =
        path.file_name().map_or_else(|| "unknown".to_string(), |s| s.to_string_lossy().to_string());

    // Look for README or index as title source
    let readme_path = path.join("README.md");
    let index_path = path.join("index.md");

    let title = if readme_path.exists() {
        extract_title_from_file(&readme_path).unwrap_or_else(|| book_id.clone())
    } else if index_path.exists() {
        extract_title_from_file(&index_path).unwrap_or_else(|| book_id.clone())
    } else {
        book_id.clone()
    };

    let metadata = BookMetadata {
        id: book_id,
        title,
        author: None,
        source: BookSource::Markdown(path.clone()),
        language: Some("en".to_string()),
        description: None,
        cover_image: None,
        added_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64),
        last_accessed: None,
    };

    let mut book = Book::new(metadata);

    // Find all markdown files and organize into chapters
    let mut md_files: Vec<_> = fs::read_dir(&path)
        .with_context(|| format!("Failed to read directory: {}", path.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "md")
                && e.file_name() != "README.md"
                && e.file_name() != "index.md"
        })
        .collect();

    // Sort by filename for consistent ordering
    md_files.sort_by_key(|e| e.file_name());

    // Create a single chapter if files are flat, or detect chapter structure
    if md_files.is_empty() {
        // Check for subdirectories as chapters
        let mut subdirs: Vec<_> = fs::read_dir(&path)
            .with_context(|| format!("Failed to read directory: {}", path.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
            .collect();

        subdirs.sort_by_key(|e| e.file_name());

        for (chapter_num, dir_entry) in subdirs.iter().enumerate() {
            let chapter_path = dir_entry.path();
            let chapter_name = dir_entry.file_name().to_string_lossy().to_string();

            let mut chapter = Chapter::new(&chapter_name, chapter_num + 1, &chapter_name);

            // Find markdown files in this chapter directory
            let mut chapter_files: Vec<_> = fs::read_dir(&chapter_path)
                .with_context(|| {
                    format!("Failed to read chapter directory: {}", chapter_path.display())
                })?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .collect();

            chapter_files.sort_by_key(|e| e.file_name());

            for (section_num, file_entry) in chapter_files.iter().enumerate() {
                let section = parse_markdown_file(&file_entry.path(), section_num + 1)?;
                chapter.sections.push(section);
            }

            if !chapter.sections.is_empty() {
                book.chapters.push(chapter);
            }
        }
    } else {
        // Flat structure - create one chapter
        let mut chapter = Chapter::new("Content", 1, "content");

        for (section_num, file_entry) in md_files.iter().enumerate() {
            let section = parse_markdown_file(&file_entry.path(), section_num + 1)?;
            chapter.sections.push(section);
        }

        if !chapter.sections.is_empty() {
            book.chapters.push(chapter);
        }
    }

    Ok(book)
}

/// Parse an mdbook-format directory using SUMMARY.md
fn parse_mdbook_directory(path: &Path, summary_path: &Path) -> Result<Book> {
    let summary_content = fs::read_to_string(summary_path)
        .with_context(|| format!("Failed to read SUMMARY.md: {}", summary_path.display()))?;

    // Extract book title from first line (usually # Title)
    let title = summary_content
        .lines()
        .find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let book_id =
        path.file_name().map_or_else(|| "unknown".to_string(), |s| s.to_string_lossy().to_string());

    let metadata = BookMetadata {
        id: book_id,
        title,
        author: None,
        source: BookSource::Markdown(path.to_path_buf()),
        language: Some("en".to_string()),
        description: None,
        cover_image: None,
        added_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64),
        last_accessed: None,
    };

    let mut book = Book::new(metadata);

    // Parse SUMMARY.md structure
    // Format: - [Title](file.md) for chapters, indented for sections
    let mut current_chapter: Option<Chapter> = None;
    let mut chapter_num = 0;

    for line in summary_content.lines() {
        // Skip empty lines and comments
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Parse markdown link: [Title](file.md)
        if let Some((link_title, link_path)) = parse_markdown_link(trimmed) {
            let file_path = path.join(&link_path);

            // Determine if this is a chapter (starts with -) or section (indented -)
            let indent = line.len() - line.trim_start().len();
            let is_chapter_line = trimmed.starts_with('-') || trimmed.starts_with('[');

            if indent == 0 && is_chapter_line && trimmed.starts_with('-') {
                // New chapter
                if let Some(ch) = current_chapter.take() {
                    if !ch.sections.is_empty() {
                        book.chapters.push(ch);
                    }
                }

                chapter_num += 1;
                let mut chapter = Chapter::new(&link_title, chapter_num, &link_path);

                // Parse the chapter's main file as first section if it exists
                if file_path.exists() {
                    if let Ok(section) = parse_markdown_file(&file_path, 0) {
                        chapter.sections.push(section);
                    }
                }

                current_chapter = Some(chapter);
            } else if indent > 0 && trimmed.starts_with('-') {
                // Section within current chapter
                if let Some(ref mut chapter) = current_chapter {
                    if file_path.exists() {
                        let section_num = chapter.sections.len() + 1;
                        if let Ok(mut section) = parse_markdown_file(&file_path, section_num) {
                            // Use the title from SUMMARY.md instead of extracting from file
                            section.title = link_title;
                            chapter.sections.push(section);
                        }
                    }
                }
            } else if indent == 0 && trimmed.starts_with('[') {
                // Top-level link without - (like [Foreword](foreword.md))
                // Treat as a standalone chapter with one section
                if let Some(ch) = current_chapter.take() {
                    if !ch.sections.is_empty() {
                        book.chapters.push(ch);
                    }
                }

                if file_path.exists() {
                    chapter_num += 1;
                    let mut chapter = Chapter::new(&link_title, chapter_num, &link_path);
                    if let Ok(section) = parse_markdown_file(&file_path, 1) {
                        chapter.sections.push(section);
                    }
                    book.chapters.push(chapter);
                }
            }
        }
    }

    // Don't forget the last chapter
    if let Some(ch) = current_chapter {
        if !ch.sections.is_empty() {
            book.chapters.push(ch);
        }
    }

    Ok(book)
}

/// Parse a markdown link [Title](path.md) and return (title, path)
fn parse_markdown_link(text: &str) -> Option<(String, String)> {
    let text = text.trim().trim_start_matches('-').trim();

    let open_bracket = text.find('[')?;
    let close_bracket = text.find(']')?;
    let open_paren = text.find('(')?;
    let close_paren = text.find(')')?;

    if close_bracket > open_bracket && open_paren == close_bracket + 1 && close_paren > open_paren {
        let title = text[open_bracket + 1..close_bracket].to_string();
        let path = text[open_paren + 1..close_paren].to_string();
        Some((title, path))
    } else {
        None
    }
}

fn extract_title_from_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let blocks = parse_markdown_content(&content);
    blocks.iter().find_map(|b| {
        if let ContentBlock::Heading { level: 1, text } = b { Some(text.clone()) } else { None }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_heading() {
        let blocks = parse_markdown_content("# Hello World");
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(&blocks[0], ContentBlock::Heading { level: 1, text } if text == "Hello World")
        );
    }

    #[test]
    fn parse_multiple_headings() {
        let md = "# H1\n## H2\n### H3";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], ContentBlock::Heading { level: 1, .. }));
        assert!(matches!(&blocks[1], ContentBlock::Heading { level: 2, .. }));
        assert!(matches!(&blocks[2], ContentBlock::Heading { level: 3, .. }));
    }

    #[test]
    fn parse_paragraph() {
        let blocks = parse_markdown_content("This is a paragraph.");
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(&blocks[0], ContentBlock::Paragraph(text) if text == "This is a paragraph.")
        );
    }

    #[test]
    fn parse_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::Code(code) = &blocks[0] {
            assert_eq!(code.language, Some("rust".to_string()));
            assert!(code.code.contains("fn main()"));
        } else {
            panic!("Expected code block");
        }
    }

    #[test]
    fn parse_unordered_list() {
        let md = "- Item 1\n- Item 2\n- Item 3";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::UnorderedList(items) = &blocks[0] {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], "Item 1");
        } else {
            panic!("Expected unordered list");
        }
    }

    #[test]
    fn parse_ordered_list() {
        let md = "1. First\n2. Second\n3. Third";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::OrderedList(items) = &blocks[0] {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], "First");
        } else {
            panic!("Expected ordered list");
        }
    }

    #[test]
    fn parse_blockquote() {
        let md = "> This is a quote\n> with multiple lines";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ContentBlock::Blockquote(text) if text.contains("quote")));
    }

    #[test]
    fn parse_horizontal_rule() {
        let md = "---";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ContentBlock::HorizontalRule));
    }

    #[test]
    fn parse_table() {
        let md = "| Name | Value |\n|------|-------|\n| foo  | 1     |\n| bar  | 2     |";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::Table(table) = &blocks[0] {
            assert_eq!(table.headers, vec!["Name", "Value"]);
            assert_eq!(table.rows.len(), 2);
            assert_eq!(table.rows[0], vec!["foo", "1"]);
            assert_eq!(table.rows[1], vec!["bar", "2"]);
        } else {
            panic!("Expected table, got {:?}", blocks.first());
        }
    }

    #[test]
    fn parse_inline_code() {
        let md = "Use `println!` to print.";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::Paragraph(text) = &blocks[0] {
            assert!(text.contains("`println!`"));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn parse_mixed_content() {
        let md = r#"# Title

This is a paragraph.

```rust
fn main() {}
```

- Item 1
- Item 2
"#;
        let blocks = parse_markdown_content(md);
        assert!(blocks.len() >= 4);
        assert!(matches!(&blocks[0], ContentBlock::Heading { level: 1, .. }));
        assert!(matches!(&blocks[1], ContentBlock::Paragraph(_)));
        assert!(matches!(&blocks[2], ContentBlock::Code(_)));
        assert!(matches!(&blocks[3], ContentBlock::UnorderedList(_)));
    }
}

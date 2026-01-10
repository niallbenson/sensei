//! Markdown parser for technical books
//!
//! Parses markdown files and directories into the unified content model.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use regex::Regex;

use super::model::{
    Alignment, Book, BookMetadata, BookSource, Chapter, CodeBlock, ContentBlock, Section, Table,
};

/// Regex for matching mdBook include directives (compiled once)
static INCLUDE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{\{#(include|rustdoc_include)\s+([^}]+)\}\}").unwrap());

/// Parse a markdown string into content blocks
// skipcq: RS-R1000 - Parser functions inherently have high cyclomatic complexity
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
    let mut in_html_comment = false;
    let mut in_caption = false; // Track when inside <span class="caption">

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
                } else if in_caption {
                    // Caption text becomes a heading (level 5)
                    flush_text(&mut current_text, &mut blocks);
                    blocks.push(ContentBlock::Heading { level: 5, text: text.to_string() });
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
                } else if in_blockquote {
                    blockquote_content.push('`');
                    blockquote_content.push_str(&code);
                    blockquote_content.push('`');
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

            // Handle HTML - extract text content, ignore tags and comments
            Event::Html(html) | Event::InlineHtml(html) => {
                let html_str = html.as_ref();

                // Track HTML comment blocks (may span multiple events)
                if html_str.contains("<!--") {
                    in_html_comment = true;
                }
                if html_str.contains("-->") {
                    in_html_comment = false;
                    continue; // Skip the closing part of comment
                }
                if in_html_comment {
                    continue; // Skip content inside HTML comments
                }

                // Check for caption/heading patterns (common in mdBook)
                // Track entering/exiting caption spans
                if html_str.contains("class=\"caption\"") || html_str.contains("class='caption'") {
                    in_caption = true;
                    continue; // Skip the opening tag itself
                }
                if in_caption && html_str.contains("</span>") {
                    in_caption = false;
                    continue; // Skip the closing tag
                }

                // Extract any visible text from HTML, skip tags
                let text = html_str
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

    // Preprocess to resolve mdBook include directives
    let base_dir = path.parent().unwrap_or(Path::new("."));
    let processed_content = preprocess_mdbook_includes(&content, base_dir);

    let filename = path.file_stem().unwrap_or_default().to_string_lossy();

    // Try to extract title from first heading
    let blocks = parse_markdown_content(&processed_content);
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

/// Preprocess mdBook include directives
/// Handles: {{#include path}}, {{#rustdoc_include path}}, with optional anchors
fn preprocess_mdbook_includes(content: &str, base_dir: &Path) -> String {
    let mut result = content.to_string();

    // Process all includes (may need multiple passes for nested includes)
    for _ in 0..5 {
        // Limit recursion depth
        let new_result = INCLUDE_RE
            .replace_all(&result, |caps: &regex::Captures| {
                let include_type = caps.get(1).map(|m| m.as_str()).unwrap_or("include");
                let path_spec = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");

                resolve_include(base_dir, path_spec, include_type == "rustdoc_include")
            })
            .to_string();

        if new_result == result {
            break; // No more changes
        }
        result = new_result;
    }

    result
}

/// Resolve a single include directive
fn resolve_include(base_dir: &Path, path_spec: &str, is_rustdoc: bool) -> String {
    // Parse path specification: path/to/file.rs:anchor or path/to/file.rs:start:end
    let (file_path, anchor) = if let Some(idx) = path_spec.find(':') {
        (&path_spec[..idx], Some(&path_spec[idx + 1..]))
    } else {
        (path_spec, None)
    };

    // Early path traversal protection: block suspicious patterns before filesystem access
    // This catches attacks even when the target file doesn't exist
    if file_path.contains("..") || file_path.starts_with('/') || file_path.starts_with('\\') {
        return format!("// Path traversal blocked: {}", file_path);
    }

    let full_path = base_dir.join(file_path);

    // Secondary protection: canonicalize and verify path stays within book directory
    let canonical_full = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return format!("// File not found: {}", file_path);
        }
    };

    // Get the book root (parent of src directory, or base_dir itself)
    let book_root = base_dir.ancestors().find(|p| p.join("book.toml").exists()).unwrap_or(base_dir);

    if let Ok(canonical_root) = book_root.canonicalize() {
        if !canonical_full.starts_with(&canonical_root) {
            return format!("// Path traversal blocked: {}", file_path);
        }
    }

    // Try to read the file
    let file_content = match fs::read_to_string(&canonical_full) {
        Ok(content) => content,
        Err(_) => {
            return format!("// File not found: {}", file_path);
        }
    };

    // Extract the relevant portion based on anchor

    if let Some(anchor) = anchor {
        extract_anchored_content(&file_content, anchor, is_rustdoc)
    } else if is_rustdoc {
        filter_rustdoc_hidden(&file_content)
    } else {
        file_content
    }
}

/// Extract content between anchor markers or line ranges
fn extract_anchored_content(content: &str, anchor: &str, is_rustdoc: bool) -> String {
    let lines: Vec<&str> = content.lines().collect();

    // Check if anchor is a line range like "start:end" or just "anchor_name"
    if let Some(colon_idx) = anchor.find(':') {
        // Line range: start:end (but could also be anchor:subanchor)
        let start_part = &anchor[..colon_idx];
        let end_part = &anchor[colon_idx + 1..];

        // Try to parse as numbers first
        if let (Ok(start), Ok(end)) = (start_part.parse::<usize>(), end_part.parse::<usize>()) {
            let start = start.saturating_sub(1); // Convert to 0-indexed
            let count = end.saturating_sub(start); // Prevent underflow if end < start
            let extracted: Vec<&str> = lines.into_iter().skip(start).take(count).collect();
            let result = extracted.join("\n");
            return if is_rustdoc { filter_rustdoc_hidden(&result) } else { result };
        }
    }

    // Try to find anchor markers: ANCHOR: name and ANCHOR_END: name
    // Or for rustdoc_include: specific named anchors
    let anchor_start = format!("ANCHOR: {}", anchor);
    let anchor_end = format!("ANCHOR_END: {}", anchor);

    let mut in_anchor = false;
    let mut extracted_lines = Vec::new();

    for line in &lines {
        if line.contains(&anchor_start) {
            in_anchor = true;
            continue;
        }
        if line.contains(&anchor_end) {
            in_anchor = false;
            continue;
        }
        if in_anchor {
            extracted_lines.push(*line);
        }
    }

    // If no anchor markers found, try special rustdoc anchors like "main", "all", "io", etc.
    if extracted_lines.is_empty() {
        // For simple anchors like "main", "all", "io", "print", "string"
        // these typically mean extract specific portions based on common patterns
        match anchor {
            "all" => {
                // Return entire file
                let result = lines.join("\n");
                return if is_rustdoc { filter_rustdoc_hidden(&result) } else { result };
            }
            "main" => {
                // Extract the main function
                return extract_function(&lines, "fn main", is_rustdoc);
            }
            _ => {
                // For other anchors, return the whole file filtered
                let result = lines.join("\n");
                return if is_rustdoc { filter_rustdoc_hidden(&result) } else { result };
            }
        }
    }

    let result = extracted_lines.join("\n");
    if is_rustdoc { filter_rustdoc_hidden(&result) } else { result }
}

/// Extract a function from code
fn extract_function(lines: &[&str], fn_start: &str, is_rustdoc: bool) -> String {
    let mut in_function = false;
    let mut brace_count = 0;
    let mut extracted = Vec::new();

    for line in lines {
        if !in_function && line.contains(fn_start) {
            in_function = true;
        }

        if in_function {
            extracted.push(*line);
            brace_count += line.chars().filter(|&c| c == '{').count();
            brace_count = brace_count.saturating_sub(line.chars().filter(|&c| c == '}').count());

            if brace_count == 0 && !extracted.is_empty() {
                break;
            }
        }
    }

    let result = extracted.join("\n");
    if is_rustdoc { filter_rustdoc_hidden(&result) } else { result }
}

/// Filter out rustdoc hidden lines (lines starting with # in doc examples)
fn filter_rustdoc_hidden(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            // Keep lines that don't start with # (hidden marker in rustdoc)
            // But keep lines that are just "#" (empty hidden) or "# " followed by code
            // Actually in rustdoc, # at start of line in code blocks hides the line
            !trimmed.starts_with("# ") && trimmed != "#"
        })
        .collect::<Vec<_>>()
        .join("\n")
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
                        // Section numbering: if chapter has intro (section 0), use len() directly
                        // Otherwise use len() + 1 to start at 1
                        let section_num = if chapter.sections.first().is_some_and(|s| s.number == 0)
                        {
                            chapter.sections.len() // Chapter intro is section 0, so len() gives us 1, 2, 3...
                        } else {
                            chapter.sections.len() + 1
                        };
                        if let Ok(mut section) = parse_markdown_file(&file_path, section_num) {
                            // Use the title from SUMMARY.md instead of extracting from file
                            section.title = link_title;
                            chapter.sections.push(section);
                        }
                    }
                }
            } else if indent == 0 && trimmed.starts_with('[') {
                // Top-level link without - (like [Foreword](foreword.md))
                // These are unnumbered chapters (front matter, appendices, etc.)
                if let Some(ch) = current_chapter.take() {
                    if !ch.sections.is_empty() {
                        book.chapters.push(ch);
                    }
                }

                if file_path.exists() {
                    // Don't increment chapter_num - these are unnumbered
                    let mut chapter = Chapter::new_unnumbered(&link_title, &link_path);
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

    #[test]
    fn parse_markdown_link_valid() {
        let result = parse_markdown_link("[Title](./path.md)");
        assert!(result.is_some());
        let (title, path) = result.unwrap();
        assert_eq!(title, "Title");
        assert_eq!(path, "./path.md");
    }

    #[test]
    fn parse_markdown_link_invalid() {
        assert!(parse_markdown_link("not a link").is_none());
        assert!(parse_markdown_link("[Title]").is_none());
        assert!(parse_markdown_link("[Title](").is_none());
    }

    #[test]
    fn parse_markdown_link_with_spaces() {
        let result = parse_markdown_link("[My Title](./my path.md)");
        assert!(result.is_some());
        let (title, _) = result.unwrap();
        assert_eq!(title, "My Title");
    }

    #[test]
    fn convert_alignment_left() {
        use pulldown_cmark::Alignment as PulldownAlign;
        assert!(matches!(convert_alignment(PulldownAlign::Left), Alignment::Left));
        assert!(matches!(convert_alignment(PulldownAlign::None), Alignment::Left));
    }

    #[test]
    fn convert_alignment_center() {
        use pulldown_cmark::Alignment as PulldownAlign;
        assert!(matches!(convert_alignment(PulldownAlign::Center), Alignment::Center));
    }

    #[test]
    fn convert_alignment_right() {
        use pulldown_cmark::Alignment as PulldownAlign;
        assert!(matches!(convert_alignment(PulldownAlign::Right), Alignment::Right));
    }

    #[test]
    fn heading_level_conversion() {
        use pulldown_cmark::HeadingLevel::*;
        assert_eq!(heading_level_to_u8(H1), 1);
        assert_eq!(heading_level_to_u8(H2), 2);
        assert_eq!(heading_level_to_u8(H3), 3);
        assert_eq!(heading_level_to_u8(H4), 4);
        assert_eq!(heading_level_to_u8(H5), 5);
        assert_eq!(heading_level_to_u8(H6), 6);
    }

    #[test]
    fn parse_image() {
        let md = "![Alt text](image.png)";
        let blocks = parse_markdown_content(md);
        // Image is parsed with src, alt may be in separate paragraph due to markdown parsing
        let has_image = blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Image { src, .. } if src == "image.png"));
        assert!(has_image, "Expected image block, got {:?}", blocks);
    }

    #[test]
    fn parse_empty_content() {
        let blocks = parse_markdown_content("");
        assert!(blocks.is_empty());
    }

    #[test]
    fn parse_whitespace_only() {
        let blocks = parse_markdown_content("   \n\n   \n");
        assert!(blocks.is_empty());
    }

    #[test]
    fn parse_nested_list() {
        let md = "- Item 1\n  - Nested\n- Item 2";
        let blocks = parse_markdown_content(md);
        // Should have list(s)
        assert!(!blocks.is_empty());
    }

    #[test]
    fn parse_code_without_language() {
        let md = "```\nplain code\n```";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::Code(code) = &blocks[0] {
            assert!(code.language.is_none());
        }
    }

    #[test]
    fn parse_multiple_paragraphs() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn flush_text_empty() {
        let mut text = String::new();
        let mut blocks = Vec::new();
        flush_text(&mut text, &mut blocks);
        assert!(blocks.is_empty());
    }

    #[test]
    fn flush_text_whitespace() {
        let mut text = "   ".to_string();
        let mut blocks = Vec::new();
        flush_text(&mut text, &mut blocks);
        assert!(blocks.is_empty());
    }

    #[test]
    fn flush_text_content() {
        let mut text = "Hello world".to_string();
        let mut blocks = Vec::new();
        flush_text(&mut text, &mut blocks);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ContentBlock::Paragraph(t) if t == "Hello world"));
    }

    #[test]
    fn skip_html_comments() {
        let md = r#"Some text.

<!-- manual-regeneration
cd some/directory
rm -rf something
cargo build
-->

More text after comment."#;
        let blocks = parse_markdown_content(md);
        // Should have only two paragraphs, no content from the HTML comment
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], ContentBlock::Paragraph(t) if t == "Some text."));
        assert!(
            matches!(&blocks[1], ContentBlock::Paragraph(t) if t == "More text after comment.")
        );
    }

    #[test]
    fn skip_inline_html_comment() {
        let md = "Text before <!-- comment --> text after.";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::Paragraph(text) = &blocks[0] {
            assert!(!text.contains("comment"));
            assert!(text.contains("Text before"));
            assert!(text.contains("text after"));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn filter_rustdoc_hidden_lines() {
        let content = "fn main() {\n# hidden line\n    println!(\"visible\");\n#\n}";
        let result = super::filter_rustdoc_hidden(content);
        assert!(!result.contains("hidden line"));
        assert!(result.contains("println!"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn filter_rustdoc_preserves_hash_in_strings() {
        // Hash in middle of line should be preserved
        let content = "let s = \"# not hidden\";";
        let result = super::filter_rustdoc_hidden(content);
        assert!(result.contains("# not hidden"));
    }

    #[test]
    fn extract_anchored_content_line_range() {
        let content = "line1\nline2\nline3\nline4\nline5";
        // Extract lines 2-4 (1-indexed in mdBook, start is adjusted to 0-indexed)
        // start=2 becomes 1 (0-indexed), count=4-1=3, so lines at index 1,2,3 = line2,line3,line4
        let result = super::extract_anchored_content(content, "2:4", false);
        assert!(result.contains("line2"));
        assert!(result.contains("line3"));
        assert!(result.contains("line4"));
        assert!(!result.contains("line1"));
        assert!(!result.contains("line5"));
    }

    #[test]
    fn extract_anchored_content_invalid_range() {
        // When start > end, should not panic (use saturating_sub)
        let content = "line1\nline2\nline3";
        let result = super::extract_anchored_content(content, "5:2", false);
        // Should return empty or handle gracefully, not panic
        assert!(result.is_empty() || !result.contains("panic"));
    }

    #[test]
    fn extract_anchored_content_with_anchor_markers() {
        let content = r#"before
// ANCHOR: example
fn example() {}
// ANCHOR_END: example
after"#;
        let result = super::extract_anchored_content(content, "example", false);
        assert!(result.contains("fn example()"));
        assert!(!result.contains("before"));
        assert!(!result.contains("after"));
        assert!(!result.contains("ANCHOR"));
    }

    #[test]
    fn extract_anchored_content_all() {
        let content = "line1\nline2\nline3";
        let result = super::extract_anchored_content(content, "all", false);
        assert_eq!(result, content);
    }

    #[test]
    fn include_regex_matches_include() {
        assert!(super::INCLUDE_RE.is_match("{{#include ../file.rs}}"));
        assert!(super::INCLUDE_RE.is_match("{{#rustdoc_include ../file.rs:anchor}}"));
        assert!(!super::INCLUDE_RE.is_match("{{#unknown ../file.rs}}"));
    }

    #[test]
    fn blockquote_with_inline_code() {
        let md =
            "> Note: If you prefer not to use `rustup` for some reason, see the Other options.";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::Blockquote(text) = &blocks[0] {
            // Inline code should be in the blockquote, not separated
            assert!(text.contains("`rustup`"), "Expected `rustup` in blockquote, got: {}", text);
            assert!(text.contains("use `rustup` for"), "Text should flow around inline code");
        } else {
            panic!("Expected blockquote, got {:?}", blocks[0]);
        }
    }

    #[test]
    fn blockquote_with_multiple_inline_codes() {
        let md = "> Use `cargo build` and `cargo run` to compile.";
        let blocks = parse_markdown_content(md);
        assert_eq!(blocks.len(), 1);
        if let ContentBlock::Blockquote(text) = &blocks[0] {
            assert!(text.contains("`cargo build`"));
            assert!(text.contains("`cargo run`"));
            // Both should be in the correct order
            let build_pos = text.find("`cargo build`").unwrap();
            let run_pos = text.find("`cargo run`").unwrap();
            assert!(build_pos < run_pos, "cargo build should come before cargo run");
        } else {
            panic!("Expected blockquote");
        }
    }

    #[test]
    fn resolve_include_blocks_path_traversal() {
        use std::path::Path;

        let base_dir = Path::new("/tmp/book/src");

        // Test parent directory traversal
        let result = super::resolve_include(base_dir, "../../../etc/passwd", false);
        assert!(
            result.contains("Path traversal blocked"),
            "Should block .. traversal, got: {}",
            result
        );

        // Test absolute path
        let result = super::resolve_include(base_dir, "/etc/passwd", false);
        assert!(
            result.contains("Path traversal blocked"),
            "Should block absolute paths, got: {}",
            result
        );

        // Test Windows-style absolute path
        let result = super::resolve_include(base_dir, "\\etc\\passwd", false);
        assert!(
            result.contains("Path traversal blocked"),
            "Should block backslash paths, got: {}",
            result
        );

        // Test hidden traversal in middle of path
        let result = super::resolve_include(base_dir, "foo/../../../etc/passwd", false);
        assert!(
            result.contains("Path traversal blocked"),
            "Should block hidden .. in path, got: {}",
            result
        );
    }

    #[test]
    fn html_caption_becomes_heading() {
        let md = r#"<span class="caption">Integer Overflow</span>

Let's say you have a variable."#;
        let blocks = parse_markdown_content(md);

        // Should have a heading for the caption and a paragraph for the text
        assert!(blocks.len() >= 2, "Expected at least 2 blocks, got {:?}", blocks);

        // First block should be a heading with the caption text
        assert!(
            matches!(&blocks[0], ContentBlock::Heading { level: 5, text } if text == "Integer Overflow"),
            "Expected heading with 'Integer Overflow', got {:?}",
            blocks[0]
        );

        // Second block should be the paragraph
        assert!(
            matches!(&blocks[1], ContentBlock::Paragraph(text) if text.contains("variable")),
            "Expected paragraph with 'variable', got {:?}",
            blocks[1]
        );
    }
}

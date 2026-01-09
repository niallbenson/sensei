//! EPUB parser for technical books
//!
//! Parses EPUB files into the unified content model.

use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use epub::doc::EpubDoc;

use super::markdown::parse_markdown_content;
use super::model::{Book, BookMetadata, BookSource, Chapter, ContentBlock, Section};

/// Parse an EPUB file into a Book
pub fn parse_epub_file(path: &Path) -> Result<Book> {
    let path = path.canonicalize().with_context(|| format!("Invalid path: {}", path.display()))?;

    let file = std::fs::File::open(&path)
        .with_context(|| format!("Failed to open EPUB file: {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut doc = EpubDoc::from_reader(reader)
        .with_context(|| format!("Failed to parse EPUB: {}", path.display()))?;

    // Extract metadata - mdata returns Option<&MetadataItem>, we need the value field
    let title = doc.mdata("title").map_or_else(
        || {
            path.file_stem()
                .map_or_else(|| "Unknown".into(), |s| s.to_string_lossy().to_string())
        },
        |m| m.value.clone(),
    );

    let author = doc.mdata("creator").map(|m| m.value.clone());
    let description = doc.mdata("description").map(|m| m.value.clone());
    let language = doc.mdata("language").map(|m| m.value.clone());

    // Generate book ID from filename
    let book_id = path.file_stem().map_or_else(
        || "unknown".to_string(),
        |s| s.to_string_lossy().to_string().to_lowercase().replace(' ', "-"),
    );

    let metadata = BookMetadata {
        id: book_id,
        title,
        author,
        source: BookSource::Epub(path),
        language,
        description,
        cover_image: None,
        added_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64),
        last_accessed: None,
    };

    let mut book = Book::new(metadata);

    // Get the spine (reading order) - spine is Vec<SpineItem>, we need idref
    let spine: Vec<String> = doc.spine.iter().map(|s| s.idref.clone()).collect();

    // Track chapter boundaries using TOC or treat each spine item as a section
    let mut current_chapter: Option<Chapter> = None;
    let mut chapter_num = 0;
    let mut section_num = 0;

    for spine_id in spine {
        // Get the resource for this spine item
        if let Some((content, _mime)) = doc.get_resource(&spine_id) {
            let content_str = String::from_utf8_lossy(&content).to_string();

            // Try to extract title from the content
            let section_title = extract_title_from_xhtml(&content_str)
                .unwrap_or_else(|| format!("Section {}", section_num + 1));

            // Check if this looks like a new chapter (H1 heading)
            let is_new_chapter = content_str.contains("<h1") || section_num == 0;

            if is_new_chapter {
                // Save previous chapter if exists
                if let Some(ch) = current_chapter.take() {
                    if !ch.sections.is_empty() {
                        book.chapters.push(ch);
                    }
                }

                chapter_num += 1;
                section_num = 0;

                let chapter_title =
                    extract_h1_from_xhtml(&content_str).unwrap_or_else(|| section_title.clone());

                current_chapter = Some(Chapter::new(
                    &chapter_title,
                    chapter_num,
                    format!("ch{:02}", chapter_num),
                ));
            }

            section_num += 1;

            // Parse XHTML content to ContentBlocks
            let blocks = parse_xhtml_content(&content_str);

            let section_path = format!(
                "ch{:02}/s{:02}",
                current_chapter.as_ref().map_or(1, |c| c.number),
                section_num
            );

            let mut section = Section::new(&section_title, section_num, section_path);
            section.content = blocks;
            section.calculate_reading_time();

            if let Some(ref mut ch) = current_chapter {
                ch.sections.push(section);
            }
        }
    }

    // Don't forget the last chapter
    if let Some(ch) = current_chapter.take() {
        if !ch.sections.is_empty() {
            book.chapters.push(ch);
        }
    }

    // If no chapters were created, create a default one
    if book.chapters.is_empty() {
        book.chapters.push(Chapter::new("Content", 1, "content"));
    }

    Ok(book)
}

/// Extract title from XHTML content (first heading)
fn extract_title_from_xhtml(xhtml: &str) -> Option<String> {
    // Try h1, then h2, then title tag
    extract_h1_from_xhtml(xhtml)
        .or_else(|| extract_tag_content(xhtml, "h2"))
        .or_else(|| extract_tag_content(xhtml, "title"))
}

/// Extract H1 content from XHTML
fn extract_h1_from_xhtml(xhtml: &str) -> Option<String> {
    extract_tag_content(xhtml, "h1")
}

/// Extract content from a specific tag
fn extract_tag_content(xhtml: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{}", tag);
    let close_tag = format!("</{}>", tag);

    if let Some(start_idx) = xhtml.find(&open_tag) {
        // Find the end of the opening tag
        let content_start = xhtml[start_idx..].find('>')? + start_idx + 1;

        // Find the closing tag
        if let Some(end_idx) = xhtml[content_start..].find(&close_tag) {
            let content = &xhtml[content_start..content_start + end_idx];
            // Strip any nested tags and clean up
            let clean = strip_html_tags(content).trim().to_string();
            if !clean.is_empty() {
                return Some(clean);
            }
        }
    }
    None
}

/// Strip HTML tags from content
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Clean up HTML entities
    result
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Parse XHTML content into ContentBlocks
fn parse_xhtml_content(xhtml: &str) -> Vec<ContentBlock> {
    // Extract the body content
    let body_content = extract_body_content(xhtml).unwrap_or_else(|| xhtml.to_string());

    // Convert XHTML to a markdown-like representation for parsing
    let markdown = xhtml_to_markdown(&body_content);

    // Use the markdown parser
    parse_markdown_content(&markdown)
}

/// Extract content between body tags
fn extract_body_content(xhtml: &str) -> Option<String> {
    let start = xhtml.find("<body")?.checked_add(xhtml[xhtml.find("<body")?..].find('>')?)?;
    let end = xhtml.find("</body>")?;

    if start < end { Some(xhtml[start + 1..end].to_string()) } else { None }
}

/// Convert XHTML to markdown-like format
fn xhtml_to_markdown(xhtml: &str) -> String {
    let mut result = String::with_capacity(xhtml.len());

    // Simple state machine for conversion
    let mut in_tag = false;
    let mut current_tag = String::default();
    let mut tag_stack: Vec<String> = Vec::new();

    for c in xhtml.chars() {
        if c == '<' {
            in_tag = true;
            current_tag.clear();
            continue;
        }

        if c == '>' {
            in_tag = false;
            process_tag(&current_tag, &mut result, &mut tag_stack);
            continue;
        }

        if in_tag {
            current_tag.push(c);
        } else {
            // Handle text content
            result.push(c);
        }
    }

    result
}

/// Process an HTML tag and convert to markdown
#[allow(clippy::cognitive_complexity)]
fn process_tag(tag: &str, output: &mut String, tag_stack: &mut Vec<String>) {
    let tag_lower = tag.to_lowercase();
    let is_closing = tag_lower.starts_with('/');
    let tag_name = if is_closing {
        tag_lower[1..].split_whitespace().next().unwrap_or("")
    } else {
        tag_lower.split_whitespace().next().unwrap_or("")
    };

    if is_closing {
        // Closing tag
        match tag_name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                output.push('\n');
                tag_stack.pop();
            }
            "p" | "div" => {
                output.push_str("\n\n");
                tag_stack.pop();
            }
            "li" => {
                output.push('\n');
                tag_stack.pop();
            }
            "pre" | "code" => {
                if tag_name == "pre" {
                    output.push_str("\n```\n");
                }
                tag_stack.pop();
            }
            "blockquote" => {
                output.push('\n');
                tag_stack.pop();
            }
            "em" | "i" => {
                output.push('*');
                tag_stack.pop();
            }
            "strong" | "b" => {
                output.push_str("**");
                tag_stack.pop();
            }
            _ => {
                tag_stack.pop();
            }
        }
    } else {
        // Opening tag
        match tag_name {
            "h1" => {
                output.push_str("\n# ");
                tag_stack.push(tag_name.to_string());
            }
            "h2" => {
                output.push_str("\n## ");
                tag_stack.push(tag_name.to_string());
            }
            "h3" => {
                output.push_str("\n### ");
                tag_stack.push(tag_name.to_string());
            }
            "h4" => {
                output.push_str("\n#### ");
                tag_stack.push(tag_name.to_string());
            }
            "h5" => {
                output.push_str("\n##### ");
                tag_stack.push(tag_name.to_string());
            }
            "h6" => {
                output.push_str("\n###### ");
                tag_stack.push(tag_name.to_string());
            }
            "p" | "div" => {
                output.push_str("\n\n");
                tag_stack.push(tag_name.to_string());
            }
            "br" => {
                output.push('\n');
            }
            "hr" => {
                output.push_str("\n---\n");
            }
            "li" => {
                output.push_str("\n- ");
                tag_stack.push(tag_name.to_string());
            }
            "pre" => {
                // Try to extract language from class
                let lang = extract_code_language(tag);
                if let Some(l) = lang {
                    output.push_str(&format!("\n```{}\n", l));
                } else {
                    output.push_str("\n```\n");
                }
                tag_stack.push(tag_name.to_string());
            }
            "code" => {
                // Only add backticks if not inside a pre tag
                if !tag_stack.iter().any(|t| t == "pre") {
                    output.push('`');
                }
                tag_stack.push(tag_name.to_string());
            }
            "blockquote" => {
                output.push_str("\n> ");
                tag_stack.push(tag_name.to_string());
            }
            "em" | "i" => {
                output.push('*');
                tag_stack.push(tag_name.to_string());
            }
            "strong" | "b" => {
                output.push_str("**");
                tag_stack.push(tag_name.to_string());
            }
            _ => {
                tag_stack.push(tag_name.to_string());
            }
        }
    }
}

/// Extract code language from tag attributes
fn extract_code_language(tag: &str) -> Option<String> {
    // Look for class="language-xxx" or class="xxx"
    if let Some(class_start) = tag.find("class=") {
        let rest = &tag[class_start + 6..];
        let quote = rest.chars().next()?;
        if quote == '"' || quote == '\'' {
            let end = rest[1..].find(quote)?;
            let classes = &rest[1..=end];

            // Look for language-xxx pattern
            for class in classes.split_whitespace() {
                if let Some(lang) = class.strip_prefix("language-") {
                    return Some(lang.to_string());
                }
            }

            // Common code class names
            let lang_classes = [
                "rust",
                "python",
                "javascript",
                "typescript",
                "java",
                "c",
                "cpp",
                "go",
                "ruby",
                "swift",
                "kotlin",
                "scala",
                "haskell",
                "ocaml",
                "sql",
                "bash",
                "shell",
                "json",
                "yaml",
                "toml",
                "xml",
                "html",
                "css",
            ];
            for class in classes.split_whitespace() {
                if lang_classes.contains(&class) {
                    return Some(class.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_tags_basic() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
        assert_eq!(strip_html_tags("<strong>Bold</strong> text"), "Bold text");
    }

    #[test]
    fn strip_html_entities() {
        assert_eq!(strip_html_tags("&lt;code&gt;"), "<code>");
        assert_eq!(strip_html_tags("&amp;&nbsp;"), "& ");
    }

    #[test]
    fn extract_tag_content_h1() {
        let xhtml = "<h1>Chapter Title</h1><p>Content</p>";
        assert_eq!(extract_tag_content(xhtml, "h1"), Some("Chapter Title".into()));
    }

    #[test]
    fn extract_tag_content_nested() {
        let xhtml = "<h1><span>Nested</span> Title</h1>";
        assert_eq!(extract_tag_content(xhtml, "h1"), Some("Nested Title".into()));
    }

    #[test]
    fn xhtml_to_markdown_heading() {
        let xhtml = "<h1>Title</h1>";
        let md = xhtml_to_markdown(xhtml);
        assert!(md.contains("# Title"));
    }

    #[test]
    fn xhtml_to_markdown_paragraph() {
        let xhtml = "<p>This is a paragraph.</p>";
        let md = xhtml_to_markdown(xhtml);
        assert!(md.contains("This is a paragraph."));
    }

    #[test]
    fn xhtml_to_markdown_code() {
        let xhtml = "<pre class=\"language-rust\"><code>fn main() {}</code></pre>";
        let md = xhtml_to_markdown(xhtml);
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main()"));
    }

    #[test]
    fn xhtml_to_markdown_list() {
        let xhtml = "<ul><li>Item 1</li><li>Item 2</li></ul>";
        let md = xhtml_to_markdown(xhtml);
        assert!(md.contains("- Item 1"));
        assert!(md.contains("- Item 2"));
    }

    #[test]
    fn extract_code_language_from_class() {
        assert_eq!(extract_code_language("pre class=\"language-rust\""), Some("rust".into()));
        assert_eq!(extract_code_language("pre class=\"rust\""), Some("rust".into()));
        assert_eq!(extract_code_language("pre class=\"foo bar\""), None);
    }
}

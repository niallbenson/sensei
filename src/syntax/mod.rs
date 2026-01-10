//! Syntax highlighting using syntect
//!
//! Provides syntax highlighting for code blocks with support for many languages.

use once_cell::sync::Lazy;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::highlighting::{
    FontStyle, HighlightState, Highlighter, RangedHighlightIterator, ThemeSet,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use crate::theme::Theme;

/// Global syntax set with all default syntaxes
static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);

/// Global theme set (we'll convert to our theme colors, but need syntect structure)
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

/// Map common language names/aliases to syntect syntax names
fn normalize_language(lang: &str) -> &str {
    // Handle comma-separated modifiers like "rust,ignore"
    let base_lang = lang.split(',').next().unwrap_or(lang).trim();

    match base_lang.to_lowercase().as_str() {
        "rs" | "rust" => "Rust",
        "py" | "python" | "python3" => "Python",
        "js" | "javascript" => "JavaScript",
        "ts" | "typescript" => "TypeScript",
        "rb" | "ruby" => "Ruby",
        "go" | "golang" => "Go",
        "java" => "Java",
        "c" => "C",
        "cpp" | "c++" | "cxx" => "C++",
        "cs" | "csharp" | "c#" => "C#",
        "swift" => "Swift",
        "kotlin" | "kt" => "Kotlin",
        "scala" => "Scala",
        "php" => "PHP",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "scss" | "sass" => "SCSS",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "xml" => "XML",
        "md" | "markdown" => "Markdown",
        "sh" | "bash" | "shell" | "zsh" | "console" | "text" => "Bourne Again Shell (bash)",
        "sql" => "SQL",
        "lua" => "Lua",
        "perl" | "pl" => "Perl",
        "r" => "R",
        "haskell" | "hs" => "Haskell",
        "ocaml" | "ml" => "OCaml",
        "clojure" | "clj" => "Clojure",
        "elixir" | "ex" => "Elixir",
        "erlang" | "erl" => "Erlang",
        "dockerfile" | "docker" => "Dockerfile",
        "makefile" | "make" => "Makefile",
        "cmake" => "CMake",
        "diff" | "patch" => "Diff",
        "ini" | "cfg" | "conf" => "INI",
        "vim" | "viml" => "VimL",
        "asm" | "assembly" | "nasm" => "Assembly (x86_64)",
        _ => base_lang,
    }
}

/// Find the syntax definition for a given language
fn find_syntax(language: Option<&str>) -> Option<&'static SyntaxReference> {
    let lang = language?;
    let normalized = normalize_language(lang);

    // Try exact match first
    SYNTAX_SET
        .find_syntax_by_name(normalized)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension(normalized.to_lowercase().as_str()))
        .or_else(|| SYNTAX_SET.find_syntax_by_extension(lang))
        .or_else(|| {
            // Try to find by partial match
            SYNTAX_SET.syntaxes().iter().find(|s| {
                s.name.to_lowercase().contains(&normalized.to_lowercase())
                    || s.file_extensions
                        .iter()
                        .any(|ext| ext.to_lowercase() == normalized.to_lowercase())
            })
        })
}

/// Convert a syntect color to a ratatui color
fn syntect_to_ratatui_color(color: syntect::highlighting::Color) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

/// Highlight a single line of code and return styled spans
pub fn highlight_line(line: &str, language: Option<&str>, theme: &Theme) -> Vec<Span<'static>> {
    // Try to get syntax highlighting
    if let Some(syntax) = find_syntax(language) {
        if let Some(syntect_theme) = THEME_SET.themes.get("base16-ocean.dark") {
            let highlighter = Highlighter::new(syntect_theme);
            let mut highlight_state =
                HighlightState::new(&highlighter, syntect::parsing::ScopeStack::new());
            let ops = syntect::parsing::ParseState::new(syntax);

            // Parse the line
            let mut parse_state = ops;
            let parsed = parse_state.parse_line(line, &SYNTAX_SET);

            // Highlight
            let ranges: Vec<_> = RangedHighlightIterator::new(
                &mut highlight_state,
                &parsed.unwrap_or_default(),
                line,
                &highlighter,
            )
            .collect();

            if !ranges.is_empty() {
                return ranges
                    .into_iter()
                    .map(|(style, text, _range)| {
                        let fg = syntect_to_ratatui_color(style.foreground);
                        let mut ratatui_style = Style::default().fg(fg).bg(theme.bg_secondary);

                        if style.font_style.contains(FontStyle::BOLD) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                        }
                        if style.font_style.contains(FontStyle::ITALIC) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                        }
                        if style.font_style.contains(FontStyle::UNDERLINE) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                        }

                        Span::styled(text.to_string(), ratatui_style)
                    })
                    .collect();
            }
        }
    }

    // Fallback: apply basic keyword highlighting using our theme
    highlight_basic(line, language, theme)
}

/// Basic keyword-based highlighting as fallback
fn highlight_basic(line: &str, language: Option<&str>, theme: &Theme) -> Vec<Span<'static>> {
    let base_style = Style::default().fg(theme.fg_primary).bg(theme.bg_secondary);

    // Get language-specific keywords
    let keywords = match language.map(normalize_language) {
        Some("Rust") => vec![
            "fn", "let", "mut", "const", "static", "pub", "mod", "use", "struct", "enum", "impl",
            "trait", "type", "where", "for", "loop", "while", "if", "else", "match", "return",
            "break", "continue", "async", "await", "move", "ref", "self", "Self", "super", "crate",
            "dyn", "unsafe", "extern", "as", "in",
        ],
        Some("Python") => vec![
            "def", "class", "if", "elif", "else", "for", "while", "try", "except", "finally",
            "with", "as", "import", "from", "return", "yield", "raise", "pass", "break",
            "continue", "lambda", "and", "or", "not", "in", "is", "None", "True", "False", "async",
            "await", "global", "nonlocal",
        ],
        Some("JavaScript") | Some("TypeScript") => vec![
            "function",
            "const",
            "let",
            "var",
            "if",
            "else",
            "for",
            "while",
            "do",
            "switch",
            "case",
            "break",
            "continue",
            "return",
            "try",
            "catch",
            "finally",
            "throw",
            "class",
            "extends",
            "new",
            "this",
            "super",
            "import",
            "export",
            "default",
            "from",
            "async",
            "await",
            "yield",
            "typeof",
            "instanceof",
            "in",
            "of",
            "null",
            "undefined",
            "true",
            "false",
            "interface",
            "type",
            "enum",
            "implements",
            "private",
            "public",
            "protected",
        ],
        Some("Go") => vec![
            "func",
            "var",
            "const",
            "type",
            "struct",
            "interface",
            "map",
            "chan",
            "if",
            "else",
            "for",
            "range",
            "switch",
            "case",
            "default",
            "break",
            "continue",
            "return",
            "go",
            "defer",
            "select",
            "package",
            "import",
            "nil",
            "true",
            "false",
            "make",
            "new",
        ],
        _ => vec![
            // Common keywords across many languages
            "if", "else", "for", "while", "return", "function", "class", "import", "export",
            "const", "let", "var", "true", "false", "null", "nil", "None",
        ],
    };

    let types = vec![
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "Box", "Rc",
        "Arc", "Cell", "RefCell", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "int", "float",
        "double", "long", "short", "byte", "boolean", "void", "object", "string", "number", "any",
        "never", "unknown", "Array", "Object", "Map", "Set", "Promise",
    ];

    let mut spans = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // String literals
            '"' | '\'' | '`' => {
                if !current.is_empty() {
                    spans.push(make_span(&current, &keywords, &types, base_style, theme));
                    current.clear();
                }
                let quote = c;
                let mut string_content = String::from(quote);
                let mut escaped = false;
                for ch in chars.by_ref() {
                    string_content.push(ch);
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == quote {
                        break;
                    }
                }
                spans.push(Span::styled(
                    string_content,
                    Style::default().fg(theme.syntax_string).bg(theme.bg_secondary),
                ));
            }
            // Comments
            '/' if chars.peek() == Some(&'/') => {
                if !current.is_empty() {
                    spans.push(make_span(&current, &keywords, &types, base_style, theme));
                    current.clear();
                }
                let mut comment = String::from('/');
                for ch in chars.by_ref() {
                    comment.push(ch);
                }
                spans.push(Span::styled(
                    comment,
                    Style::default().fg(theme.syntax_comment).bg(theme.bg_secondary),
                ));
            }
            '#' if language.map(normalize_language) == Some("Python")
                || language.map(normalize_language) == Some("Bourne Again Shell (bash)") =>
            {
                if !current.is_empty() {
                    spans.push(make_span(&current, &keywords, &types, base_style, theme));
                    current.clear();
                }
                let mut comment = String::from('#');
                for ch in chars.by_ref() {
                    comment.push(ch);
                }
                spans.push(Span::styled(
                    comment,
                    Style::default().fg(theme.syntax_comment).bg(theme.bg_secondary),
                ));
            }
            // Numbers
            '0'..='9'
                if current.is_empty()
                    || !current.chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_') =>
            {
                if !current.is_empty() {
                    spans.push(make_span(&current, &keywords, &types, base_style, theme));
                    current.clear();
                }
                let mut number = String::from(c);
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_hexdigit()
                        || ch == 'x'
                        || ch == 'X'
                        || ch == 'b'
                        || ch == 'o'
                        || ch == '.'
                        || ch == '_'
                    {
                        number.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                spans.push(Span::styled(
                    number,
                    Style::default().fg(theme.syntax_number).bg(theme.bg_secondary),
                ));
            }
            // Word boundaries
            c if c.is_alphanumeric() || c == '_' => {
                current.push(c);
            }
            // Operators and punctuation
            _ => {
                if !current.is_empty() {
                    spans.push(make_span(&current, &keywords, &types, base_style, theme));
                    current.clear();
                }
                let style = if "+-*/%=<>!&|^~?:;,.()[]{}".contains(c) {
                    Style::default().fg(theme.syntax_operator).bg(theme.bg_secondary)
                } else {
                    base_style
                };
                spans.push(Span::styled(c.to_string(), style));
            }
        }
    }

    if !current.is_empty() {
        spans.push(make_span(&current, &keywords, &types, base_style, theme));
    }

    if spans.is_empty() {
        spans.push(Span::styled(line.to_string(), base_style));
    }

    spans
}

/// Create a span with appropriate styling based on whether it's a keyword/type
fn make_span(
    word: &str,
    keywords: &[&str],
    types: &[&str],
    base_style: Style,
    theme: &Theme,
) -> Span<'static> {
    let style = if keywords.contains(&word) {
        Style::default()
            .fg(theme.syntax_keyword)
            .bg(theme.bg_secondary)
            .add_modifier(Modifier::BOLD)
    } else if types.contains(&word) {
        Style::default().fg(theme.syntax_type).bg(theme.bg_secondary)
    } else if word.starts_with(|c: char| c.is_uppercase()) && word.len() > 1 {
        // Likely a type/class name
        Style::default().fg(theme.syntax_type).bg(theme.bg_secondary)
    } else if word.ends_with('!') {
        // Rust macro
        Style::default().fg(theme.syntax_function).bg(theme.bg_secondary)
    } else {
        base_style
    };

    Span::styled(word.to_string(), style)
}

/// Check if a language is supported
pub fn is_language_supported(language: &str) -> bool {
    find_syntax(Some(language)).is_some()
}

/// Get list of supported language names (for debugging/help)
#[allow(dead_code)]
pub fn supported_languages() -> Vec<&'static str> {
    SYNTAX_SET.syntaxes().iter().map(|s| s.name.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rust() {
        assert_eq!(normalize_language("rust"), "Rust");
        assert_eq!(normalize_language("rs"), "Rust");
        assert_eq!(normalize_language("rust,ignore"), "Rust");
    }

    #[test]
    fn normalize_python() {
        assert_eq!(normalize_language("python"), "Python");
        assert_eq!(normalize_language("py"), "Python");
    }

    #[test]
    fn normalize_javascript() {
        assert_eq!(normalize_language("javascript"), "JavaScript");
        assert_eq!(normalize_language("js"), "JavaScript");
    }

    #[test]
    fn normalize_shell() {
        assert_eq!(normalize_language("bash"), "Bourne Again Shell (bash)");
        assert_eq!(normalize_language("console"), "Bourne Again Shell (bash)");
        assert_eq!(normalize_language("sh"), "Bourne Again Shell (bash)");
    }

    #[test]
    fn find_rust_syntax() {
        let syntax = find_syntax(Some("rust"));
        assert!(syntax.is_some());
    }

    #[test]
    fn find_python_syntax() {
        let syntax = find_syntax(Some("python"));
        assert!(syntax.is_some());
    }

    #[test]
    fn highlight_rust_line() {
        let theme = Theme::default();
        let spans = highlight_line("let x = 5;", Some("rust"), &theme);
        assert!(!spans.is_empty());
    }

    #[test]
    fn highlight_unknown_language() {
        let theme = Theme::default();
        let spans = highlight_line("some code", Some("nonexistent_lang"), &theme);
        assert!(!spans.is_empty());
    }

    #[test]
    fn basic_highlight_keywords() {
        let theme = Theme::default();
        let spans = highlight_basic("fn main() {}", Some("Rust"), &theme);
        // Should have multiple spans including the keyword "fn"
        assert!(spans.len() > 1);
    }

    #[test]
    fn basic_highlight_string() {
        let theme = Theme::default();
        let spans = highlight_basic("let s = \"hello\";", Some("Rust"), &theme);
        // Should highlight the string
        assert!(spans.iter().any(|s| s.content.contains("hello")));
    }

    #[test]
    fn basic_highlight_comment() {
        let theme = Theme::default();
        let spans = highlight_basic("// this is a comment", Some("Rust"), &theme);
        assert!(!spans.is_empty());
    }
}

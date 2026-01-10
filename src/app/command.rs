//! Command parsing and execution for the command line

use std::path::PathBuf;

/// Parsed command from the command line
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Add a book from path: :add /path/to/book
    Add(PathBuf),
    /// Remove a book: :remove <book-id>
    Remove(String),
    /// Open/switch to a book: :open <book-id>
    Open(String),
    /// List available books: :list
    List,
    /// Quit the application: :q or :quit
    Quit,
    /// Show help: :help or :h
    Help,
    /// Search content: /pattern
    Search(String),
    /// Go to specific section: :goto <path>
    Goto(String),
    /// Clear message: (empty command)
    Nop,
    /// Start Claude API setup wizard: :claude-setup
    ClaudeSetup,
    /// Set Claude API key: :claude-key <api-key>
    ClaudeKey(String),
    /// Set Claude model: :claude-model <haiku|sonnet>
    ClaudeModel(String),
    /// Clear Claude state: :claude-clear
    ClaudeClear,
    /// Ask Claude a question: :ask <question>
    Ask(String),
}

/// Result of parsing a command
#[derive(Debug)]
pub enum ParseResult {
    /// Successfully parsed command
    Ok(Command),
    /// Unknown command
    UnknownCommand(String),
    /// Command needs an argument
    MissingArgument(String),
}

/// Parse a command string (without the leading : or /)
pub fn parse_command(input: &str) -> ParseResult {
    let input = input.trim();

    if input.is_empty() {
        return ParseResult::Ok(Command::Nop);
    }

    // Split into command and arguments
    let mut parts = input.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let args = parts.next().map(|s| s.trim()).unwrap_or("");

    match cmd.to_lowercase().as_str() {
        "add" | "a" => {
            if args.is_empty() {
                ParseResult::MissingArgument("add".to_string())
            } else {
                ParseResult::Ok(Command::Add(PathBuf::from(args)))
            }
        }
        "remove" | "rm" | "delete" => {
            if args.is_empty() {
                ParseResult::MissingArgument("remove".to_string())
            } else {
                ParseResult::Ok(Command::Remove(args.to_string()))
            }
        }
        "open" | "o" | "load" => {
            if args.is_empty() {
                ParseResult::MissingArgument("open".to_string())
            } else {
                ParseResult::Ok(Command::Open(args.to_string()))
            }
        }
        "list" | "ls" | "l" => ParseResult::Ok(Command::List),
        "quit" | "q" => ParseResult::Ok(Command::Quit),
        "help" | "h" | "?" => ParseResult::Ok(Command::Help),
        "goto" | "g" => {
            if args.is_empty() {
                ParseResult::MissingArgument("goto".to_string())
            } else {
                ParseResult::Ok(Command::Goto(args.to_string()))
            }
        }
        "claude-setup" | "cs" => ParseResult::Ok(Command::ClaudeSetup),
        "claude-key" | "ck" => {
            if args.is_empty() {
                ParseResult::MissingArgument("claude-key".to_string())
            } else {
                ParseResult::Ok(Command::ClaudeKey(args.to_string()))
            }
        }
        "claude-model" | "cm" => {
            if args.is_empty() {
                ParseResult::MissingArgument("claude-model".to_string())
            } else {
                ParseResult::Ok(Command::ClaudeModel(args.to_string()))
            }
        }
        "claude-clear" | "cc" => ParseResult::Ok(Command::ClaudeClear),
        "ask" => {
            if args.is_empty() {
                ParseResult::MissingArgument("ask".to_string())
            } else {
                ParseResult::Ok(Command::Ask(args.to_string()))
            }
        }
        _ => ParseResult::UnknownCommand(cmd.to_string()),
    }
}

/// Parse a search query (without the leading /)
pub fn parse_search(input: &str) -> Command {
    Command::Search(input.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quit_command() {
        assert!(matches!(parse_command("q"), ParseResult::Ok(Command::Quit)));
        assert!(matches!(parse_command("quit"), ParseResult::Ok(Command::Quit)));
        assert!(matches!(parse_command("Q"), ParseResult::Ok(Command::Quit)));
    }

    #[test]
    fn parse_help_command() {
        assert!(matches!(parse_command("help"), ParseResult::Ok(Command::Help)));
        assert!(matches!(parse_command("h"), ParseResult::Ok(Command::Help)));
        assert!(matches!(parse_command("?"), ParseResult::Ok(Command::Help)));
    }

    #[test]
    fn parse_list_command() {
        assert!(matches!(parse_command("list"), ParseResult::Ok(Command::List)));
        assert!(matches!(parse_command("ls"), ParseResult::Ok(Command::List)));
        assert!(matches!(parse_command("l"), ParseResult::Ok(Command::List)));
    }

    #[test]
    fn parse_add_command() {
        match parse_command("add /path/to/book") {
            ParseResult::Ok(Command::Add(path)) => {
                assert_eq!(path, PathBuf::from("/path/to/book"));
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn parse_add_missing_arg() {
        assert!(matches!(parse_command("add"), ParseResult::MissingArgument(_)));
    }

    #[test]
    fn parse_open_command() {
        match parse_command("open my-book") {
            ParseResult::Ok(Command::Open(id)) => {
                assert_eq!(id, "my-book");
            }
            _ => panic!("Expected Open command"),
        }
    }

    #[test]
    fn parse_remove_command() {
        match parse_command("remove my-book") {
            ParseResult::Ok(Command::Remove(id)) => {
                assert_eq!(id, "my-book");
            }
            _ => panic!("Expected Remove command"),
        }
        // Also test aliases
        assert!(matches!(parse_command("rm my-book"), ParseResult::Ok(Command::Remove(_))));
        assert!(matches!(parse_command("delete my-book"), ParseResult::Ok(Command::Remove(_))));
    }

    #[test]
    fn parse_remove_missing_arg() {
        assert!(matches!(parse_command("remove"), ParseResult::MissingArgument(_)));
    }

    #[test]
    fn parse_goto_command() {
        match parse_command("goto ch1/s2") {
            ParseResult::Ok(Command::Goto(path)) => {
                assert_eq!(path, "ch1/s2");
            }
            _ => panic!("Expected Goto command"),
        }
    }

    #[test]
    fn parse_unknown_command() {
        assert!(matches!(parse_command("unknown"), ParseResult::UnknownCommand(_)));
    }

    #[test]
    fn parse_empty_is_nop() {
        assert!(matches!(parse_command(""), ParseResult::Ok(Command::Nop)));
        assert!(matches!(parse_command("   "), ParseResult::Ok(Command::Nop)));
    }

    #[test]
    fn test_parse_search() {
        let cmd = super::parse_search("test query");
        assert!(matches!(cmd, Command::Search(q) if q == "test query"));
    }
}

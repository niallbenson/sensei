//! Data models for Claude API requests and responses

use serde::{Deserialize, Serialize};

/// Available Claude models
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaudeModel {
    /// Claude Haiku 4.5 - fast and cost-effective ($1/$5 per MTok)
    #[default]
    Haiku45,
    /// Claude Haiku 3 - legacy fast model ($0.25/$1.25 per MTok)
    Haiku3,
    /// Claude Sonnet 4 - capable ($3/$15 per MTok)
    Sonnet4,
    /// Claude Sonnet 4.5 - latest Sonnet ($3/$15 per MTok)
    Sonnet45,
    /// Claude Opus 4.5 - most capable ($5/$25 per MTok)
    Opus45,
}

impl ClaudeModel {
    /// Get the API model identifier
    pub fn model_id(&self) -> &'static str {
        match self {
            Self::Haiku45 => "claude-haiku-4-5-20251001",
            Self::Haiku3 => "claude-3-haiku-20240307",
            Self::Sonnet4 => "claude-sonnet-4-20250514",
            Self::Sonnet45 => "claude-sonnet-4-5-20250929",
            Self::Opus45 => "claude-opus-4-5-20251101",
        }
    }

    /// Get a human-readable display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Haiku45 => "Claude Haiku 4.5",
            Self::Haiku3 => "Claude Haiku 3",
            Self::Sonnet4 => "Claude Sonnet 4",
            Self::Sonnet45 => "Claude Sonnet 4.5",
            Self::Opus45 => "Claude Opus 4.5",
        }
    }

    /// Parse model from string (for command line or model ID)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            // User-friendly names
            "haiku" | "haiku45" | "haiku4.5" => Some(Self::Haiku45),
            "haiku3" => Some(Self::Haiku3),
            "sonnet4" => Some(Self::Sonnet4),
            "sonnet" | "sonnet45" | "sonnet4.5" => Some(Self::Sonnet45),
            "opus" | "opus45" | "opus4.5" => Some(Self::Opus45),
            // Model IDs (for session restoration)
            "claude-haiku-4-5-20251001" => Some(Self::Haiku45),
            "claude-3-haiku-20240307" => Some(Self::Haiku3),
            "claude-sonnet-4-20250514" => Some(Self::Sonnet4),
            "claude-sonnet-4-5-20250929" => Some(Self::Sonnet45),
            "claude-opus-4-5-20251101" => Some(Self::Opus45),
            // Legacy model IDs (for backward compatibility)
            "claude-3-5-haiku-20241022" => Some(Self::Haiku45),
            "claude-3-5-sonnet-20241022" => Some(Self::Sonnet45),
            _ => None,
        }
    }

    /// List all available models
    pub fn all() -> &'static [ClaudeModel] {
        &[Self::Haiku45, Self::Haiku3, Self::Sonnet4, Self::Sonnet45, Self::Opus45]
    }
}

impl std::str::FromStr for ClaudeModel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
            .ok_or_else(|| format!("Unknown model: {}. Options: haiku, haiku3, sonnet4, sonnet, opus", s))
    }
}

/// Message role in conversation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message
    User,
    /// Assistant (Claude) message
    Assistant,
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: Role,
    /// Message content
    pub content: String,
}

impl Message {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

/// Request body for Claude messages API
#[derive(Debug, Clone, Serialize)]
pub struct CreateMessageRequest {
    /// Model identifier
    pub model: String,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// Conversation messages
    pub messages: Vec<Message>,
    /// Optional system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Whether to stream the response
    pub stream: bool,
}

impl CreateMessageRequest {
    /// Create a new request with default settings
    pub fn new(model: ClaudeModel, messages: Vec<Message>) -> Self {
        Self {
            model: model.model_id().to_string(),
            max_tokens: 4096,
            messages,
            system: None,
            stream: true,
        }
    }

    /// Set the system prompt
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Set max tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Disable streaming
    pub fn without_streaming(mut self) -> Self {
        self.stream = false;
        self
    }
}

/// Events received from Claude's streaming API (SSE)
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Message started - contains message ID
    MessageStart {
        /// Unique message identifier
        id: String,
    },
    /// Content block started
    ContentBlockStart,
    /// Content block delta - contains text chunk
    ContentBlockDelta {
        /// Text chunk
        text: String,
    },
    /// Content block finished
    ContentBlockStop,
    /// Message metadata update
    MessageDelta {
        /// Stop reason (if finished)
        stop_reason: Option<String>,
    },
    /// Message finished
    MessageStop,
    /// Keepalive ping
    Ping,
    /// Error from API
    Error {
        /// Error message
        message: String,
    },
}

/// Non-streaming response from Claude API
#[derive(Debug, Clone, Deserialize)]
pub struct MessageResponse {
    /// Message ID
    pub id: String,
    /// Content blocks
    pub content: Vec<ContentBlock>,
    /// Stop reason
    pub stop_reason: Option<String>,
    /// Usage statistics
    pub usage: Usage,
}

/// Content block in response
#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlock {
    /// Block type (usually "text")
    #[serde(rename = "type")]
    pub block_type: String,
    /// Text content
    pub text: Option<String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    /// Input tokens used
    pub input_tokens: u32,
    /// Output tokens generated
    pub output_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_parse() {
        assert_eq!(ClaudeModel::parse("haiku"), Some(ClaudeModel::Haiku45));
        assert_eq!(ClaudeModel::parse("haiku3"), Some(ClaudeModel::Haiku3));
        assert_eq!(ClaudeModel::parse("sonnet4"), Some(ClaudeModel::Sonnet4));
        assert_eq!(ClaudeModel::parse("sonnet"), Some(ClaudeModel::Sonnet45));
        assert_eq!(ClaudeModel::parse("sonnet45"), Some(ClaudeModel::Sonnet45));
        assert_eq!(ClaudeModel::parse("opus"), Some(ClaudeModel::Opus45));
        assert_eq!(ClaudeModel::parse("HAIKU"), Some(ClaudeModel::Haiku45));
        assert_eq!(ClaudeModel::parse("unknown"), None);
    }

    #[test]
    fn create_message_request() {
        let messages = vec![Message::user("Hello")];
        let request = CreateMessageRequest::new(ClaudeModel::Haiku45, messages)
            .with_system("You are helpful")
            .with_max_tokens(1000);

        assert_eq!(request.model, "claude-haiku-4-5-20251001");
        assert_eq!(request.max_tokens, 1000);
        assert_eq!(request.system, Some("You are helpful".to_string()));
        assert!(request.stream);
    }
}

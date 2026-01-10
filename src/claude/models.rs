//! Data models for Claude API requests and responses

use serde::{Deserialize, Serialize};

/// Available Claude models
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaudeModel {
    /// Claude 3.5 Haiku - fast and cost-effective
    #[default]
    Haiku35,
    /// Claude 3.5 Sonnet - balanced (older)
    Sonnet35,
    /// Claude Sonnet 4 - capable
    Sonnet4,
    /// Claude Sonnet 4.5 - latest Sonnet
    Sonnet45,
    /// Claude Opus 4.5 - most capable
    Opus45,
}

impl ClaudeModel {
    /// Get the API model identifier
    pub fn model_id(&self) -> &'static str {
        match self {
            Self::Haiku35 => "claude-3-5-haiku-20241022",
            Self::Sonnet35 => "claude-3-5-sonnet-20241022",
            Self::Sonnet4 => "claude-sonnet-4-20250514",
            Self::Sonnet45 => "claude-sonnet-4-5-20250514",
            Self::Opus45 => "claude-opus-4-5-20251101",
        }
    }

    /// Get a human-readable display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Haiku35 => "Claude 3.5 Haiku",
            Self::Sonnet35 => "Claude 3.5 Sonnet",
            Self::Sonnet4 => "Claude Sonnet 4",
            Self::Sonnet45 => "Claude Sonnet 4.5",
            Self::Opus45 => "Claude Opus 4.5",
        }
    }

    /// Parse model from string (for command line)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "haiku" | "haiku35" | "3.5-haiku" => Some(Self::Haiku35),
            "sonnet35" | "3.5-sonnet" => Some(Self::Sonnet35),
            "sonnet4" => Some(Self::Sonnet4),
            "sonnet" | "sonnet45" | "4.5-sonnet" => Some(Self::Sonnet45),
            "opus" | "opus45" | "4.5-opus" => Some(Self::Opus45),
            _ => None,
        }
    }

    /// List all available models
    pub fn all() -> &'static [ClaudeModel] {
        &[Self::Haiku35, Self::Sonnet35, Self::Sonnet4, Self::Sonnet45, Self::Opus45]
    }
}

impl std::str::FromStr for ClaudeModel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
            .ok_or_else(|| format!("Unknown model: {}. Options: haiku, sonnet35, sonnet, opus", s))
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
        assert_eq!(ClaudeModel::parse("haiku"), Some(ClaudeModel::Haiku35));
        assert_eq!(ClaudeModel::parse("sonnet35"), Some(ClaudeModel::Sonnet35));
        assert_eq!(ClaudeModel::parse("sonnet4"), Some(ClaudeModel::Sonnet4));
        assert_eq!(ClaudeModel::parse("sonnet"), Some(ClaudeModel::Sonnet45));
        assert_eq!(ClaudeModel::parse("sonnet45"), Some(ClaudeModel::Sonnet45));
        assert_eq!(ClaudeModel::parse("opus"), Some(ClaudeModel::Opus45));
        assert_eq!(ClaudeModel::parse("HAIKU"), Some(ClaudeModel::Haiku35));
        assert_eq!(ClaudeModel::parse("unknown"), None);
    }

    #[test]
    fn create_message_request() {
        let messages = vec![Message::user("Hello")];
        let request = CreateMessageRequest::new(ClaudeModel::Haiku35, messages)
            .with_system("You are helpful")
            .with_max_tokens(1000);

        assert_eq!(request.model, "claude-3-5-haiku-20241022");
        assert_eq!(request.max_tokens, 1000);
        assert_eq!(request.system, Some("You are helpful".to_string()));
        assert!(request.stream);
    }
}

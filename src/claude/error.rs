//! Error types for Claude API integration

use thiserror::Error;

/// Errors that can occur when interacting with the Claude API
#[derive(Debug, Error)]
pub enum ClaudeError {
    /// API key is not configured
    #[error("API key not configured. Run :claude-setup to configure")]
    ApiKeyNotFound,

    /// Failed to access system keyring
    #[error("Failed to access keyring: {0}")]
    KeyringError(String),

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error ({status}): {message}")]
    ApiError {
        /// HTTP status code
        status: u16,
        /// Error message from API
        message: String,
    },

    /// Rate limited by the API
    #[error("Rate limited. Retry after {retry_after_seconds} seconds")]
    RateLimited {
        /// Seconds to wait before retrying
        retry_after_seconds: u64,
    },

    /// Failed to parse streaming response
    #[error("Stream parsing error: {0}")]
    StreamParseError(String),

    /// Request was cancelled by user
    #[error("Request cancelled")]
    Cancelled,

    /// Invalid API key format
    #[error("Invalid API key format. Key should start with 'sk-ant-'")]
    InvalidApiKey,

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl ClaudeError {
    /// Check if this error is recoverable (user can retry)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ClaudeError::RateLimited { .. } | ClaudeError::RequestError(_) | ClaudeError::Cancelled
        )
    }

    /// Check if this error requires re-authentication
    pub fn requires_reauth(&self) -> bool {
        matches!(
            self,
            ClaudeError::ApiKeyNotFound
                | ClaudeError::InvalidApiKey
                | ClaudeError::ApiError { status: 401, .. }
        )
    }
}

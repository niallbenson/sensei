//! Claude API integration module
//!
//! Provides API key management, HTTP client, and streaming support
//! for interacting with Claude's messages API.

pub mod auth;
pub mod client;
pub mod error;
pub mod models;
pub mod streaming;

// Re-export commonly used types
pub use auth::ApiKeyManager;
pub use client::ClaudeClient;
pub use error::ClaudeError;
pub use models::{ClaudeModel, CreateMessageRequest, Message, Role, StreamEvent};

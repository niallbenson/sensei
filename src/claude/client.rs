//! HTTP client for Claude API

use reqwest::Client;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::error::ClaudeError;
use super::models::{CreateMessageRequest, MessageResponse, StreamEvent};
use super::streaming;

/// Claude API client
pub struct ClaudeClient {
    /// HTTP client
    client: Client,
    /// API key for authentication
    api_key: String,
}

impl ClaudeClient {
    /// Claude API base URL
    const API_URL: &'static str = "https://api.anthropic.com/v1/messages";
    /// API version header value
    const API_VERSION: &'static str = "2023-06-01";

    /// Create a new Claude client with the given API key
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, api_key }
    }

    /// Send a streaming message request
    ///
    /// Spawns a background task that streams responses through the channel.
    /// Use the cancellation token to interrupt the request.
    pub async fn send_streaming(
        &self,
        request: CreateMessageRequest,
        tx: mpsc::Sender<StreamEvent>,
        cancel_token: CancellationToken,
    ) -> Result<(), ClaudeError> {
        let response = self
            .client
            .post(Self::API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", Self::API_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        // Handle HTTP errors
        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            return Err(ClaudeError::RateLimited { retry_after_seconds: retry_after });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClaudeError::ApiError {
                status: 401,
                message: "Invalid API key".to_string(),
            });
        }

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(ClaudeError::ApiError { status: status.as_u16(), message });
        }

        // Process the streaming response
        streaming::process_stream(response, tx, cancel_token).await
    }

    /// Send a non-streaming message request
    ///
    /// Returns the complete response. Useful for testing or simple queries.
    pub async fn send_message(
        &self,
        mut request: CreateMessageRequest,
    ) -> Result<MessageResponse, ClaudeError> {
        // Disable streaming for this request
        request.stream = false;

        let response = self
            .client
            .post(Self::API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", Self::API_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            return Err(ClaudeError::RateLimited { retry_after_seconds: retry_after });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClaudeError::ApiError {
                status: 401,
                message: "Invalid API key".to_string(),
            });
        }

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(ClaudeError::ApiError { status: status.as_u16(), message });
        }

        let body = response.text().await?;
        let message_response: MessageResponse = serde_json::from_str(&body)?;
        Ok(message_response)
    }

    /// Test the API key by sending a minimal request
    pub async fn test_connection(&self) -> Result<(), ClaudeError> {
        use super::models::{ClaudeModel, Message};

        let request = CreateMessageRequest::new(ClaudeModel::Haiku35, vec![Message::user("Hi")])
            .with_max_tokens(10)
            .without_streaming();

        self.send_message(request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_creation() {
        let client = ClaudeClient::new("sk-ant-test-key".to_string());
        assert_eq!(client.api_key, "sk-ant-test-key");
    }
}

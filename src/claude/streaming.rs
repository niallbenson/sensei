//! Server-Sent Events (SSE) parser for Claude streaming responses

use futures_util::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::error::ClaudeError;
use super::models::StreamEvent;

/// Process an SSE stream from the Claude API
///
/// Reads the response body as a stream of SSE events and sends parsed
/// events through the provided channel. Respects the cancellation token
/// for user interruption.
pub async fn process_stream(
    response: Response,
    tx: mpsc::Sender<StreamEvent>,
    cancel_token: CancellationToken,
) -> Result<(), ClaudeError> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut current_event_type = String::new();

    loop {
        tokio::select! {
            // Check for cancellation
            _ = cancel_token.cancelled() => {
                return Err(ClaudeError::Cancelled);
            }

            // Process next chunk from stream
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete lines
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim_end().to_string();
                            buffer = buffer[newline_pos + 1..].to_string();

                            // Parse SSE line
                            if let Some(event_type) = line.strip_prefix("event: ") {
                                current_event_type = event_type.to_string();
                            } else if let Some(data) = line.strip_prefix("data: ") {
                                if let Some(event) = parse_event(&current_event_type, data) {
                                    // Send event, exit if receiver dropped
                                    if tx.send(event).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                            // Ignore empty lines and comments (lines starting with :)
                        }
                    }
                    Some(Err(e)) => {
                        return Err(ClaudeError::RequestError(e));
                    }
                    None => {
                        // Stream ended
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Parse a single SSE event from event type and data
fn parse_event(event_type: &str, data: &str) -> Option<StreamEvent> {
    match event_type {
        "message_start" => {
            let parsed: serde_json::Value = serde_json::from_str(data).ok()?;
            let id = parsed["message"]["id"].as_str()?.to_string();
            Some(StreamEvent::MessageStart { id })
        }

        "content_block_start" => Some(StreamEvent::ContentBlockStart),

        "content_block_delta" => {
            let parsed: serde_json::Value = serde_json::from_str(data).ok()?;
            let text = parsed["delta"]["text"].as_str()?.to_string();
            Some(StreamEvent::ContentBlockDelta { text })
        }

        "content_block_stop" => Some(StreamEvent::ContentBlockStop),

        "message_delta" => {
            let parsed: serde_json::Value = serde_json::from_str(data).ok()?;
            let stop_reason = parsed["delta"]["stop_reason"].as_str().map(|s| s.to_string());
            Some(StreamEvent::MessageDelta { stop_reason })
        }

        "message_stop" => Some(StreamEvent::MessageStop),

        "ping" => Some(StreamEvent::Ping),

        "error" => {
            let parsed: serde_json::Value = serde_json::from_str(data).ok()?;
            let message =
                parsed["error"]["message"].as_str().unwrap_or("Unknown error").to_string();
            Some(StreamEvent::Error { message })
        }

        _ => {
            tracing::debug!("Unknown SSE event type: {}", event_type);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_block_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event = parse_event("content_block_delta", data);
        assert!(matches!(
            event,
            Some(StreamEvent::ContentBlockDelta { text }) if text == "Hello"
        ));
    }

    #[test]
    fn parse_message_start() {
        let data = r#"{"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant","content":[],"model":"claude-3-5-haiku-20241022","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":1}}}"#;
        let event = parse_event("message_start", data);
        assert!(matches!(
            event,
            Some(StreamEvent::MessageStart { id }) if id == "msg_123"
        ));
    }

    #[test]
    fn parse_message_stop() {
        let data = r#"{"type":"message_stop"}"#;
        let event = parse_event("message_stop", data);
        assert!(matches!(event, Some(StreamEvent::MessageStop)));
    }

    #[test]
    fn parse_error() {
        let data =
            r#"{"type":"error","error":{"type":"invalid_request_error","message":"Bad request"}}"#;
        let event = parse_event("error", data);
        assert!(matches!(
            event,
            Some(StreamEvent::Error { message }) if message == "Bad request"
        ));
    }
}

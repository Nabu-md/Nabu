//! Native messaging protocol implementation for Safari extension communication.
//!
//! This module provides:
//! - Message types for browser-to-app communication
//! - Validation of incoming messages
//! - Reading/writing length-prefixed JSON messages (standard native messaging format)

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use thiserror::Error;

/// Maximum payload size (1MB) to prevent memory exhaustion attacks
const MAX_PAYLOAD_SIZE: usize = 1024 * 1024;

/// Allowed capture commands
const ALLOWED_COMMANDS: &[&str] = &["capture"];

/// Errors that can occur during native messaging operations
#[derive(Debug, Error)]
pub enum NativeMessagingError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Socket error: {0}")]
    SocketError(String),

    #[error("Unknown command: {0}")]
    UnknownCommand(String),

    #[error("Payload too large: {0} bytes (max: {1})")]
    PayloadTooLarge(usize, usize),
}

/// A message sent between the Safari extension and the native host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique request identifier for matching requests with responses
    pub request_id: Option<u64>,

    /// The command to execute (e.g., "capture")
    pub command: String,

    /// The capture type (e.g., "bookmark", "note", "document")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_type: Option<String>,

    /// The message payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,

    /// Success flag for responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,

    /// Error message for failed responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Result data for successful responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Native messaging host that reads from stdin and writes to stdout
pub struct NativeMessagingHost {
    stdin: std::io::Stdin,
    stdout: std::io::Stdout,
}

impl NativeMessagingHost {
    /// Creates a new native messaging host
    pub fn new() -> Self {
        Self {
            stdin: std::io::stdin(),
            stdout: std::io::stdout(),
        }
    }

    /// Reads a length-prefixed JSON message from stdin
    pub fn read_message(&mut self) -> Result<Message, NativeMessagingError> {
        // Read 4-byte length prefix (big-endian)
        let mut length_bytes = [0u8; 4];
        self.stdin.read_exact(&mut length_bytes)?;
        let length = u32::from_be_bytes(length_bytes) as usize;

        // Validate length
        if length > MAX_PAYLOAD_SIZE {
            return Err(NativeMessagingError::PayloadTooLarge(
                length,
                MAX_PAYLOAD_SIZE,
            ));
        }

        // Read message body
        let mut buffer = vec![0u8; length];
        self.stdin.read_exact(&mut buffer)?;

        // Deserialize JSON
        let message: Message = serde_json::from_slice(&buffer)
            .map_err(|e| NativeMessagingError::DeserializationError(e.to_string()))?;

        Ok(message)
    }

    /// Writes a length-prefixed JSON message to stdout
    pub fn write_message(&mut self, message: &Message) -> Result<(), NativeMessagingError> {
        let json = serde_json::to_vec(message)
            .map_err(|e| NativeMessagingError::SerializationError(e.to_string()))?;

        let length = json.len() as u32;
        let mut length_bytes = length.to_be_bytes();

        self.stdout.write_all(&mut length_bytes)?;
        self.stdout.write_all(&json)?;
        self.stdout.flush()?;

        Ok(())
    }

    /// Validates an incoming message
    pub fn validate_message(&self, message: &Message) -> Result<Message, NativeMessagingError> {
        // Check command is present
        if message.command.is_empty() {
            return Err(NativeMessagingError::ValidationError(
                "Command is required".to_string(),
            ));
        }

        // Check command is allowed
        if !ALLOWED_COMMANDS.contains(&message.command.as_str()) {
            return Err(NativeMessagingError::UnknownCommand(
                message.command.clone(),
            ));
        }

        // For capture commands, validate payload
        if message.command == "capture" {
            if let Some(ref payload) = message.payload {
                // Validate payload size
                let payload_str = serde_json::to_string(payload)
                    .map_err(|e| NativeMessagingError::SerializationError(e.to_string()))?;
                if payload_str.len() > MAX_PAYLOAD_SIZE {
                    return Err(NativeMessagingError::PayloadTooLarge(
                        payload_str.len(),
                        MAX_PAYLOAD_SIZE,
                    ));
                }

                // Validate capture type — must match the set accepted by the
                // socket server and the CaptureEngine handler names.
                if let Some(ref capture_type) = message.capture_type {
                    let valid_capture_types = [
                        "bookmark",
                        "note",
                        "document",
                        "reader_mode",
                        "safari_reader",
                        "clipboard",
                        "screenshot",
                        "screen_capture",
                        "file_drop",
                        "watch_folder",
                        "youtube",
                        "github",
                        "email",
                        "article",
                        "browser",
                    ];
                    if !valid_capture_types.contains(&capture_type.as_str()) {
                        return Err(NativeMessagingError::ValidationError(format!(
                            "Invalid capture type: {}",
                            capture_type
                        )));
                    }
                } else {
                    return Err(NativeMessagingError::ValidationError(
                        "Capture type is required for capture command".to_string(),
                    ));
                }
            } else {
                return Err(NativeMessagingError::ValidationError(
                    "Payload is required for capture command".to_string(),
                ));
            }
        }

        Ok(message.clone())
    }
}

impl Default for NativeMessagingHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_validate_capture_message() {
        let host = NativeMessagingHost::new();

        let valid_message = Message {
            request_id: Some(1),
            command: "capture".to_string(),
            capture_type: Some("bookmark".to_string()),
            payload: Some(serde_json::json!({
                "url": "https://example.com",
                "title": "Example"
            })),
            success: None,
            error: None,
            result: None,
        };

        assert!(host.validate_message(&valid_message).is_ok());
    }

    #[test]
    fn test_reject_unknown_command() {
        let host = NativeMessagingHost::new();

        let invalid_message = Message {
            request_id: Some(1),
            command: "delete".to_string(),
            capture_type: None,
            payload: None,
            success: None,
            error: None,
            result: None,
        };

        assert!(host.validate_message(&invalid_message).is_err());
    }

    #[test]
    fn test_reject_missing_capture_type() {
        let host = NativeMessagingHost::new();

        let invalid_message = Message {
            request_id: Some(1),
            command: "capture".to_string(),
            capture_type: None,
            payload: Some(serde_json::json!({})),
            success: None,
            error: None,
            result: None,
        };

        assert!(host.validate_message(&invalid_message).is_err());
    }

    #[test]
    fn test_reject_invalid_capture_type() {
        let host = NativeMessagingHost::new();

        let invalid_message = Message {
            request_id: Some(1),
            command: "capture".to_string(),
            capture_type: Some("invalid".to_string()),
            payload: Some(serde_json::json!({})),
            success: None,
            error: None,
            result: None,
        };

        assert!(host.validate_message(&invalid_message).is_err());
    }
}

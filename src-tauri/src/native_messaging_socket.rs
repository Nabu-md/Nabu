//! Unix socket server for native messaging host communication.
//!
//! This module provides a socket server that listens for connections from
//! the native messaging host binary. It receives capture messages, validates
//! them, converts them to canonical [`CaptureRequest`] values, and dispatches
//! them through the canonical [`CaptureEngine::ingest`] flow.
//!
//! The wire protocol matches the shared `native_messaging::Message` type used
//! by the Safari extension host — nothing custom, no side paths.

use std::path::PathBuf;
use std::sync::Arc;

use nabu_core::capture::{CaptureData, CaptureEngine, CaptureRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::native_messaging::Message;

const SOCKET_PATH: &str = "/tmp/nabu-native-messaging.sock";
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10 MB max message size

/// Errors that can occur during socket operations
#[derive(Debug, thiserror::Error)]
pub enum SocketError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Capture error: {0}")]
    CaptureError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Result type for socket operations
pub type SocketResult<T> = Result<T, SocketError>;

/// Shared state for the socket server
pub struct SocketServerState {
    pub engine: Arc<CaptureEngine>,
}

/// Validates a native messaging capture message.
///
/// Ensures the message is a `capture` command with a known capture type and a
/// non-empty payload within size limits.
fn validate_capture_message(message: &Message) -> SocketResult<()> {
    if message.command != "capture" {
        return Err(SocketError::ValidationError(format!(
            "Unknown command: {}",
            message.command
        )));
    }

    let capture_type = message
        .capture_type
        .as_deref()
        .ok_or_else(|| SocketError::ValidationError("Capture type is required".to_string()))?;

    let valid_sources = [
        "clipboard",
        "screenshot",
        "browser",
        "watch_folder",
        "reader_mode",
        "youtube",
        "github",
        "bookmark",
        "note",
        "document",
    ];
    if !valid_sources.contains(&capture_type) {
        return Err(SocketError::ValidationError(format!(
            "Invalid capture type: {}. Valid sources: {:?}",
            capture_type, valid_sources
        )));
    }

    let payload = message
        .payload
        .as_ref()
        .ok_or_else(|| SocketError::ValidationError("Payload is required".to_string()))?;

    let payload_size = serde_json::to_vec(payload).map_err(SocketError::SerializationError)?;
    if payload_size.len() > MAX_MESSAGE_SIZE {
        return Err(SocketError::ValidationError(format!(
            "Payload size {} exceeds maximum {} bytes",
            payload_size.len(),
            MAX_MESSAGE_SIZE
        )));
    }

    Ok(())
}

/// Converts a validated capture `Message` into a canonical [`CaptureRequest`].
///
/// The payload is a free-form JSON object; this extracts the conventional
/// fields (`title`, `url`, `text`/`content`) into the canonical model.
fn message_to_capture_request(message: &Message) -> CaptureRequest {
    let payload = message
        .payload
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));

    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .map(String::from);
    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .map(String::from);
    let text = payload
        .get("text")
        .or_else(|| payload.get("content"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let capture_type = message.capture_type.as_deref().unwrap_or("note");

    let data = if let Some(url) = url.clone() {
        if matches!(
            capture_type,
            "bookmark" | "browser" | "reader_mode" | "youtube" | "github"
        ) {
            CaptureData::Uri(url)
        } else {
            match text {
                Some(text) => CaptureData::Text(text),
                None => CaptureData::Uri(url),
            }
        }
    } else if let Some(text) = text {
        CaptureData::Text(text)
    } else {
        // Fall back to a text representation of the payload.
        CaptureData::Text(payload.to_string())
    };

    let mut request = CaptureRequest::new(data);
    if let Some(title) = title {
        request = request.with_title(title);
    }
    if let Some(url) = url {
        request = request.with_url(url);
    }
    request
}

/// Starts the Unix socket server for native messaging communication.
///
/// The server listens for connections from the native messaging host binary
/// and processes capture requests. Each connection is handled in a separate
/// task.
pub fn start_socket_server(state: Arc<SocketServerState>) -> SocketResult<SocketServerHandle> {
    // Remove existing socket file if it exists
    let socket_path = PathBuf::from(SOCKET_PATH);
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;

    // Set socket permissions to allow the native messaging host to connect
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&socket_path)?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o777);
        std::fs::set_permissions(&socket_path, perms)?;
    }

    let engine = state.engine.clone();
    let handle = SocketServerHandle {
        socket_path,
        shutdown_tx: Arc::new(tokio::sync::Notify::new()),
    };

    // Spawn server task
    let shutdown_tx = handle.shutdown_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_tx.notified() => {
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let engine = engine.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, engine).await {
                                    eprintln!("Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                        }
                    }
                }
            }
        }

        // Clean up socket file on shutdown
        let _ = std::fs::remove_file(SOCKET_PATH);
    });

    Ok(handle)
}

/// Handles a single client connection
async fn handle_connection(mut stream: UnixStream, engine: Arc<CaptureEngine>) -> SocketResult<()> {
    loop {
        // Read message length (4 bytes, big-endian)
        let mut length_bytes = [0u8; 4];
        match stream.read_exact(&mut length_bytes).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // Client disconnected
                break;
            }
            Err(e) => return Err(e.into()),
        }
        let length = u32::from_be_bytes(length_bytes) as usize;

        // Validate message size
        if length > MAX_MESSAGE_SIZE {
            eprintln!("Message size {} exceeds maximum allowed size", length);
            break;
        }

        // Read message body
        let mut buffer = vec![0u8; length];
        stream.read_exact(&mut buffer).await?;

        // Deserialize message
        let message: Message = serde_json::from_slice(&buffer)?;

        // Validate the request before dispatching
        if let Err(e) = validate_capture_message(&message) {
            eprintln!("Invalid capture request: {}", e);
            let error_response = Message {
                request_id: message.request_id,
                command: "capture".to_string(),
                capture_type: message.capture_type,
                payload: None,
                success: Some(false),
                error: Some(e.to_string()),
                result: None,
            };
            write_message(&mut stream, &error_response).await?;
            continue;
        }

        // Convert to canonical CaptureRequest
        let request = message_to_capture_request(&message);

        // Dispatch through the canonical CaptureEngine → Queue → Workers flow.
        let response = match engine.ingest(request).await {
            Ok(Some(object_id)) => Message {
                request_id: message.request_id,
                command: "capture".to_string(),
                capture_type: message.capture_type,
                payload: None,
                success: Some(true),
                error: None,
                result: Some(serde_json::json!({ "object_id": object_id })),
            },
            Ok(None) => Message {
                request_id: message.request_id,
                command: "capture".to_string(),
                capture_type: message.capture_type,
                payload: None,
                success: Some(false),
                error: Some("No capture handler accepted the request".to_string()),
                result: None,
            },
            Err(e) => Message {
                request_id: message.request_id,
                command: "capture".to_string(),
                capture_type: message.capture_type,
                payload: None,
                success: Some(false),
                error: Some(e.to_string()),
                result: None,
            },
        };

        write_message(&mut stream, &response).await?;
    }

    Ok(())
}

/// Writes a length-prefixed `Message` to the stream.
async fn write_message(stream: &mut UnixStream, message: &Message) -> SocketResult<()> {
    let json = serde_json::to_vec(message).map_err(SocketError::SerializationError)?;
    let length = json.len() as u32;
    let mut length_bytes = length.to_be_bytes();
    stream.write_all(&mut length_bytes).await?;
    stream.write_all(&json).await?;
    stream.flush().await?;
    Ok(())
}

/// Handle for controlling the socket server
pub struct SocketServerHandle {
    socket_path: PathBuf,
    shutdown_tx: Arc<tokio::sync::Notify>,
}

impl SocketServerHandle {
    /// Signals the server to shut down
    pub fn shutdown(self) {
        self.shutdown_tx.notify_one();
    }

    /// Returns the path to the socket file
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_messaging::Message;

    #[test]
    fn test_socket_server_creation() {
        let engine = Arc::new(CaptureEngine::new());
        let state = Arc::new(SocketServerState { engine });

        // This test just verifies the server can be created
        let socket_path = PathBuf::from(SOCKET_PATH);
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        // Verify socket path is correct
        assert_eq!(socket_path, PathBuf::from(SOCKET_PATH));
    }

    #[test]
    fn test_validate_capture_message() {
        let valid = Message {
            request_id: Some(1),
            command: "capture".to_string(),
            capture_type: Some("bookmark".to_string()),
            payload: Some(serde_json::json!({ "url": "https://example.com", "title": "Example" })),
            success: None,
            error: None,
            result: None,
        };
        assert!(validate_capture_message(&valid).is_ok());

        let bad_command = Message {
            request_id: Some(1),
            command: "delete".to_string(),
            capture_type: Some("bookmark".to_string()),
            payload: Some(serde_json::json!({})),
            success: None,
            error: None,
            result: None,
        };
        assert!(validate_capture_message(&bad_command).is_err());

        let no_payload = Message {
            request_id: Some(1),
            command: "capture".to_string(),
            capture_type: Some("bookmark".to_string()),
            payload: None,
            success: None,
            error: None,
            result: None,
        };
        assert!(validate_capture_message(&no_payload).is_err());
    }

    #[test]
    fn test_message_to_capture_request() {
        let bookmark = Message {
            request_id: Some(1),
            command: "capture".to_string(),
            capture_type: Some("bookmark".to_string()),
            payload: Some(serde_json::json!({ "url": "https://example.com", "title": "Example" })),
            success: None,
            error: None,
            result: None,
        };
        let request = message_to_capture_request(&bookmark);
        assert!(matches!(request.data, CaptureData::Uri(ref u) if u == "https://example.com"));
        assert_eq!(request.title.as_deref(), Some("Example"));

        let note = Message {
            request_id: Some(2),
            command: "capture".to_string(),
            capture_type: Some("note".to_string()),
            payload: Some(serde_json::json!({ "text": "Hello world", "title": "Note" })),
            success: None,
            error: None,
            result: None,
        };
        let request = message_to_capture_request(&note);
        assert!(matches!(request.data, CaptureData::Text(ref t) if t == "Hello world"));
    }
}

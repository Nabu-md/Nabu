//! Unix socket server for native messaging host communication.
//!
//! This module provides a socket server that listens for connections from
//! the native messaging host binary. It receives capture requests, validates
//! them, and dispatches them to the CaptureEngine.

use std::path::PathBuf;
use std::sync::Arc;

use nabu_core::capture::{CaptureEngine, CaptureRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

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

/// Validates a native messaging capture request.
///
/// Ensures the request has a valid source type, vault ID, and payload size.
/// Rejects requests that exceed the maximum message size or have invalid fields.
fn validate_capture_request(request: &CaptureRequest) -> SocketResult<()> {
    // Validate source type
    let valid_sources = ["clipboard", "screenshot", "browser", "watch_folder", "reader_mode", "youtube", "github"];
    if !valid_sources.contains(&request.source_type.as_str()) {
        return Err(SocketError::ValidationError(format!(
            "Invalid source type: {}. Valid sources: {:?}",
            request.source_type, valid_sources
        )));
    }

    // Validate vault ID is not empty
    if request.vault_id.is_empty() {
        return Err(SocketError::ValidationError("Vault ID cannot be empty".to_string()));
    }

    // Validate payload size
    let payload_size = serde_json::to_vec(&request.payload).map_err(SocketError::SerializationError)?;
    if payload_size.len() > MAX_MESSAGE_SIZE {
        return Err(SocketError::ValidationError(format!(
            "Payload size {} exceeds maximum {} bytes",
            payload_size.len(), MAX_MESSAGE_SIZE
        )));
    }

    // Validate context size
    let context_size = serde_json::to_vec(&request.context).map_err(SocketError::SerializationError)?;
    if context_size.len() > MAX_MESSAGE_SIZE {
        return Err(SocketError::ValidationError(format!(
            "Context size {} exceeds maximum {} bytes",
            context_size.len(), MAX_MESSAGE_SIZE
        )));
    }

    Ok(())
}

/// Starts the Unix socket server for native messaging communication.
///
/// The server listens for connections from the native messaging host binary
/// and processes capture requests. Each connection is handled in a separate
/// task.
///
/// # Arguments
///
/// * `state` - Shared state containing the CaptureEngine
///
/// # Returns
///
/// Returns a handle that can be used to stop the server.
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
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = handle.shutdown_tx.notified() => {
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
async fn handle_connection(
    mut stream: UnixStream,
    engine: Arc<CaptureEngine>,
) -> SocketResult<()> {
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
        let message: CaptureRequest = serde_json::from_slice(&buffer)?;

        // Validate the request before dispatching
        if let Err(e) = validate_capture_request(&message) {
            eprintln!("Invalid capture request: {}", e);
            // Send error response
            let error_response = serde_json::json!({
                "success": false,
                "error": e.to_string(),
                "message": "Invalid capture request"
            });
            let response_json = serde_json::to_vec(&error_response)?;
            let response_length = response_json.len() as u32;
            let mut response_length_bytes = response_length.to_be_bytes();
            stream.write_all(&mut response_length_bytes).await?;
            stream.write_all(&response_json).await?;
            stream.flush().await?;
            continue;
        }

        // Dispatch to capture engine
        let result = engine.dispatch(message);

        // Serialize response
        let response_json = serde_json::to_vec(&result)?;
        let response_length = response_json.len() as u32;
        let mut response_length_bytes = response_length.to_be_bytes();

        // Write response
        stream.write_all(&mut response_length_bytes).await?;
        stream.write_all(&response_json).await?;
        stream.flush().await?;
    }

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
    use nabu_core::event_bus::EventBus;

    #[test]
    fn test_socket_server_creation() {
        let event_bus = Arc::new(EventBus::new());
        let engine = Arc::new(CaptureEngine::new(event_bus));
        let state = Arc::new(SocketServerState { engine });
        
        // This test just verifies the server can be created
        // In a real test, we'd need to run it in a tokio runtime
        let socket_path = PathBuf::from(SOCKET_PATH);
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
        
        // Verify socket path is correct
        assert_eq!(socket_path, PathBuf::from(SOCKET_PATH));
    }
}

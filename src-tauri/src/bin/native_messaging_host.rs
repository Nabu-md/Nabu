//! Native messaging host for Safari extension communication.
//!
//! This binary implements the macOS native messaging protocol:
//! - Reads length-prefixed JSON messages from stdin
//! - Validates and forwards them to the Tauri app via Unix socket
//! - Reads responses from the Tauri app
//! - Writes length-prefixed JSON responses to stdout
//!
//! The native messaging host is registered with Safari via a plist file
//! in ~/Library/Application Support/com.apple.Safari/NativeMessagingHosts/

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

// The native messaging protocol types live in the app library crate.
use app_lib::native_messaging::{Message, NativeMessagingError, NativeMessagingHost};

const SOCKET_PATH: &str = "/tmp/nabu-native-messaging.sock";

fn main() {
    if let Err(e) = run() {
        eprintln!("Native messaging host error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), NativeMessagingError> {
    let mut host = NativeMessagingHost::new();
    
    loop {
        // Read message from Safari (stdin)
        let message = host.read_message()?;
        
        // Validate the message
        let validated = host.validate_message(&message)?;
        
        // Forward to Tauri app via Unix socket
        let response = forward_to_tauri(&validated)?;
        
        // Write response to Safari (stdout)
        host.write_message(&response)?;
    }
}

fn forward_to_tauri(message: &Message) -> Result<Message, NativeMessagingError> {
    let socket_path = PathBuf::from(SOCKET_PATH);
    
    // Connect to Tauri app's Unix socket
    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| NativeMessagingError::SocketError(format!("Failed to connect to socket: {}", e)))?;
    
    // Serialize the message
    let message_json = serde_json::to_vec(message)
        .map_err(|e| NativeMessagingError::SerializationError(e.to_string()))?;
    
    // Write length-prefixed message
    let length = (message_json.len() as u32).to_be_bytes();
    stream.write_all(&length)
        .map_err(|e| NativeMessagingError::SocketError(format!("Failed to write length: {}", e)))?;
    stream.write_all(&message_json)
        .map_err(|e| NativeMessagingError::SocketError(format!("Failed to write message: {}", e)))?;
    stream.flush()
        .map_err(|e| NativeMessagingError::SocketError(format!("Failed to flush: {}", e)))?;
    
    // Read response length
    let mut length_bytes = [0u8; 4];
    stream.read_exact(&mut length_bytes)
        .map_err(|e| NativeMessagingError::SocketError(format!("Failed to read length: {}", e)))?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    
    // Read response
    let mut response_bytes = vec![0u8; length];
    stream.read_exact(&mut response_bytes)
        .map_err(|e| NativeMessagingError::SocketError(format!("Failed to read response: {}", e)))?;
    
    // Deserialize response
    let response: Message = serde_json::from_slice(&response_bytes)
        .map_err(|e| NativeMessagingError::SerializationError(e.to_string()))?;
    
    Ok(response)
}

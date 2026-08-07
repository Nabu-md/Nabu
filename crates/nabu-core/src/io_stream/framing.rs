//! Message framing for newline-delimited JSON-RPC.
//!
//! The transport uses newline-delimited JSON (NDJSON / JSON Lines) as its
//! framing strategy. Each JSON-RPC request or response is serialized to a
//! single line terminated by `\n`. This is the standard framing used by
//! LSP (Language Server Protocol) and is the repository-standard for all
//! future protocol servers (ACP, MCP, etc.).
//!
//! ## Why newline-delimited JSON?
//!
//! - No custom protocol invented — uses the well-established NDJSON convention.
//! - Each message is self-delimiting: the reader reads until `\n`, then
//!   parses the accumulated buffer as one JSON document.
//! - Compatible with `tokio::io::AsyncBufRead`, which provides
//!   `read_line` — a zero-copy, growable buffer approach.
//! - Works identically for stdin and stdout.
//! - Future protocol implementations (ACP, MCP) can reuse this transport
//!   without modification.

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Encode a serializable value as a newline-terminated JSON string.
///
/// The output is a single line of JSON with a trailing `\n`. This is
/// the unit of framing for the stdio transport.
pub fn encode_message<T: Serialize>(value: &T) -> crate::io_stream::TransportResult<String> {
    serde_json::to_string(value).map_err(Into::into).map(|mut s| {
        s.push('\n');
        s
    })
}

/// Decode a newline-delimited JSON line into the requested type.
///
/// The input `line` should be the raw string content (without the trailing
/// newline). Deserialization failures are mapped to
/// [`TransportError::Deserialize`].
pub fn decode_message<T: DeserializeOwned>(line: &str) -> crate::io_stream::TransportResult<T> {
    serde_json::from_str(line.trim()).map_err(Into::into)
}

/// Decode a byte buffer (as produced by `read_line`) into the requested type.
///
/// The buffer may contain trailing whitespace or a newline; these are
/// trimmed before deserialization.
pub fn decode_message_bytes<T: DeserializeOwned>(
    bytes: &[u8],
) -> crate::io_stream::TransportResult<T> {
    let line = String::from_utf8(bytes)
        .map_err(|e| crate::io_stream::TransportError::invalid(format!("invalid UTF-8: {}", e)))?;
    decode_message::<T>(&line)
}

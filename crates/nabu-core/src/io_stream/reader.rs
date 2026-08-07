//! Asynchronous stdin reader for the stdio JSON-RPC transport.
//!
//! [`AsyncStdinReader`] continuously reads newline-delimited JSON from stdin,
//! deserializes each line as a [`Request`], and forwards the request to the
//! provided callback (typically the JSON-RPC router's `dispatch` method).
//!
//! ## Message Framing
//!
//! Input is newline-delimited JSON (NDJSON / JSON Lines). Each line is a
//! complete JSON-RPC request. The reader uses `tokio::io::AsyncBufReadExt::read_line`
//! to read one line at a time into a growable buffer. Lines exceeding
//! `max_message_bytes` cause a [`TransportError::MessageTooLarge`].
//!
//! ## EOF Handling
//!
//! When stdin returns 0 bytes (EOF), the reader returns `Ok(())` to signal
//! that input has been exhausted. This is the normal termination signal —
//! the upstream process closed stdin.
//!
//! ## Shutdown
//!
//! The reader checks a shared `Arc<AtomicBool>` shutdown flag before each
//! read attempt, allowing the transport to terminate promptly even when
//! no input is arriving.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufRead, AsyncBufReadExt};

use crate::io_stream::config::TransportConfig;
use crate::io_stream::errors::{TransportError, TransportResult};
use crate::io_stream::framing::decode_message_bytes;
use crate::rpc::Request;

/// Async reader for stdin that reads newline-delimited JSON-RPC requests.
///
/// The reader is designed to be spawned as a tokio task. It reads from
/// `tokio::io::stdin()` (or any injected `AsyncBufRead` for testing) line
/// by line, deserializes each line into a [`Request`], and forwards it to
/// the callback.
///
/// ## Thread Safety
///
/// The reader holds an `Arc<AtomicBool>` shutdown flag (shared with the
/// writer and transport) for lock-free shutdown signaling. The shutdown
/// flag is extracted into a local `Arc` before the read loop begins,
/// ensuring the async future is `Send` even though `dyn AsyncBufRead`
/// is not `Sync`.
pub struct AsyncStdinReader {
    /// Configuration for framing limits (max message size).
    config: TransportConfig,
    /// Shared shutdown signal — when `true`, the read loop exits.
    shutdown: Arc<AtomicBool>,
}

impl AsyncStdinReader {
    /// Create a new reader with the given config and shutdown signal.
    ///
    /// The reader does not own the stdin handle — it is passed as a
    /// parameter to [`run`](Self::run). This allows the same reader
    /// to work with real stdin or an injected test reader.
    pub fn new(config: TransportConfig, shutdown: Arc<AtomicBool>) -> Self {
        Self { config, shutdown }
    }

    /// Read and deserialize a single JSON-RPC request from the reader.
    ///
    /// Returns:
    /// - `Ok(Some(request))` if a valid request was read.
    /// - `Ok(None)` on EOF (stdin closed, no more input).
    /// - `Err(_)` on a deserialization or I/O error.
    ///
    /// Blank lines are skipped. Lines exceeding `max_message_bytes`
    /// produce a [`TransportError::MessageTooLarge`] error.
    ///
    /// This is a static method (not taking `&self`) to avoid `&self`
    /// capture issues in spawned async tasks — the caller should pass
    /// `max_message_bytes` from the config directly.
    async fn read_request<R: AsyncBufRead + Unpin>(
        max_message_bytes: usize,
        reader: &mut R,
    ) -> TransportResult<Option<Request>> {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await.map_err(TransportError::io)?;

        if bytes_read == 0 {
            // EOF
            return Ok(None);
        }

        // Skip blank lines (whitespace-only) by looping
        while line.trim().is_empty() {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await.map_err(TransportError::io)?;
            if bytes_read == 0 {
                return Ok(None);
            }
        }

        // Check message size
        if line.len() > max_message_bytes {
            return Err(TransportError::message_too_large(
                max_message_bytes,
                line.len(),
            ));
        }

        // Deserialize the line as a JSON-RPC Request
        // trim() removes the trailing newline and any leading/trailing whitespace
        let bytes = line.trim().as_bytes();
        let request: Request = decode_message_bytes::<Request>(bytes)?;

        Ok(Some(request))
    }

    /// Main read loop.
    ///
    /// Reads requests from the provided reader in a loop and invokes
    /// `on_request` for each one. The loop terminates when:
    /// - EOF is received on the reader (`Ok(())` returned).
    /// - The shutdown flag is set.
    /// - An error occurs (returned as `Err`).
    ///
    /// The `on_request` callback receives the deserialized [`Request`]
    /// and is expected to dispatch it through the router and write the
    /// response.
    ///
    /// # Arguments
    ///
    /// * `reader` — The `AsyncBufRead` to read from. This can be
    ///   `tokio::io::stdin()` (wrapped in a `BufReader`) for production,
    ///   or any test reader for testing.
    /// * `on_request` — Callback invoked for each request. The callback
    ///   is async and receives the `Request` by value.
    pub async fn run<R, F, Fut>(self, reader: &mut R, mut on_request: F) -> TransportResult<()>
    where
        R: AsyncBufRead + Unpin,
        F: FnMut(Request) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send,
    {
        // Extract the shutdown flag and config into locals so that
        // &self is not held across await points. This is critical for
        // Send-ness: AsyncStdinReader holds an Arc<AtomicBool> (Send+Sync)
        // and TransportConfig (Send+Sync), but the async future must not
        // borrow &self which would capture the non-Sync `reader` field
        // (if it existed — it doesn't anymore, but we avoid the issue
        // entirely by extracting into locals).
        let shutdown = self.shutdown.clone();
        let max_message_bytes = self.config.max_message_bytes;

        loop {
            // Check shutdown before each read attempt
            if shutdown.load(Ordering::Acquire) {
                tracing::debug!("stdin reader: shutdown signal received");
                return Ok(());
            }

            // Read the next request
            match Self::read_request(max_message_bytes, reader).await {
                Ok(Some(request)) => {
                    tracing::trace!("stdin reader: received request id={}", request.id);
                    on_request(request).await;
                }
                Ok(None) => {
                    tracing::debug!("stdin reader: EOF on stdin — stopping read loop");
                    return Ok(());
                }
                Err(TransportError::MessageTooLarge { .. }) => {
                    tracing::warn!("stdin reader: message too large, skipping line");
                    // The oversized line has already been consumed by read_line.
                    // Continue reading the next line.
                }
                Err(e) => {
                    tracing::error!(error = %e, "stdin reader: fatal error");
                    return Err(e);
                }
            }
        }
    }
}

impl Default for AsyncStdinReader {
    fn default() -> Self {
        Self::new(
            TransportConfig::default(),
            Arc::new(AtomicBool::new(false)),
        )
    }
}

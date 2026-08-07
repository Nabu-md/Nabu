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
/// `tokio::io::stdin()` (or an injected reader for testing) line by line,
/// deserializes each line into a [`Request`], and forwards it to the
/// callback.
///
/// ## Thread Safety
///
/// The reader holds an `Arc<AtomicBool>` shutdown flag (shared with the
/// writer and transport) for lock-free shutdown signaling. The callback
/// is `Send + Sync` via `Arc<dyn Fn>` so it can be invoked from any
/// spawned task.
pub struct AsyncStdinReader {
    /// Configuration for framing limits.
    config: TransportConfig,
    /// Shared shutdown signal — when `true`, the read loop exits.
    shutdown: Arc<AtomicBool>,
    /// The underlying reader. Injected for testability.
    /// When `None`, `tokio::io::stdin()` is used (wrapped in a BufReader).
    reader: Option<Box<dyn AsyncBufRead + Unpin + Send>>,
}

impl AsyncStdinReader {
    /// Create a new reader with the given config and shutdown signal.
    ///
    /// If `reader` is `None`, the reader uses `tokio::io::stdin()` wrapped
    /// in a `BufReader` for line-buffered reading. Pass a custom reader
    /// (e.g. `tokio::io::BufReader::new(Cursor::new(...))`) for testing.
    pub fn new(
        config: TransportConfig,
        shutdown: Arc<AtomicBool>,
        reader: Option<Box<dyn AsyncBufRead + Unpin + Send>>,
    ) -> Self {
        Self {
            config,
            shutdown,
            reader,
        }
    }

    /// Returns `true` if the shutdown signal has been set.
    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
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
    async fn read_request<R: AsyncBufRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> TransportResult<Option<Request>> {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await.map_err(TransportError::io)?;

        if bytes_read == 0 {
            // EOF
            return Ok(None);
        }

        // Skip blank lines (whitespace-only)
        if line.trim().is_empty() {
            return self.read_request(reader).await;
        }

        // Check message size
        if line.len() > self.config.max_message_bytes {
            return Err(TransportError::message_too_large(
                self.config.max_message_bytes,
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
    /// Reads requests from stdin in a loop and invokes `on_request` for
    /// each one. The loop terminates when:
    /// - EOF is received on stdin (`Ok(())` returned).
    /// - The shutdown flag is set.
    /// - An error occurs (returned as `Err`).
    ///
    /// The `on_request` callback receives the deserialized [`Request`]
    /// and an [`AsyncStdoutWriter`] for writing the response.
    pub async fn run<R, F, Fut>(mut self, reader: &mut R, mut on_request: F) -> TransportResult<()>
    where
        R: AsyncBufRead + Unpin,
        F: FnMut(Request) -> Fut + Send + '_,
        Fut: std::future::Future<Output = ()> + Send,
    {
        loop {
            // Check shutdown before each read attempt
            if self.is_shutdown() {
                tracing::debug!("stdin reader: shutdown signal received");
                return Ok(());
            }

            // Read the next request
            match self.read_request(reader).await {
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
            None,
        )
    }
}

//! Asynchronous stdout writer for the stdio JSON-RPC transport.
//!
//! [`AsyncStdoutWriter`] serializes JSON-RPC [`Response`] values and writes
//! them to stdout as newline-delimited JSON. It supports receiving multiple
//! responses (for concurrent request handling) and flushes after each write
//! to ensure the client receives the response promptly.
//!
//! ## Thread Safety
//!
//! The writer uses a `tokio::sync::Mutex` around the stdout handle to
//! serialize writes. This prevents interleaved output from concurrent
//! response tasks. The mutex is never held across long-running operations —
//! only during the actual `write_all` + `flush` cycle.
//!
//! The writer is designed to be used through an `Arc` reference, shared
//! between the transport and the spawned read-loop task. All public methods
//! take `&self` so they can be called from any `Arc<AsyncStdoutWriter>`.
//!
//! ## Shutdown
//!
//! The writer checks a shared `Arc<AtomicBool>` shutdown flag before each
//! write. Once shutdown is signaled, further writes return
//! [`TransportError::Shutdown`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::io_stream::config::TransportConfig;
use crate::io_stream::errors::{TransportError, TransportResult};
use crate::io_stream::framing::encode_message;
use crate::rpc::Response;

/// Async writer for stdout that serializes JSON-RPC responses.
///
/// The writer wraps `tokio::io::stdout()` (or an injected writer for
/// testing) and writes newline-delimited JSON responses. Writes are
/// serialized through an internal `tokio::sync::Mutex`.
///
/// All public methods take `&self`, making the writer usable through
/// `Arc<AsyncStdoutWriter>` for sharing across tokio tasks.
pub struct AsyncStdoutWriter {
    /// Configuration for flush behavior.
    config: TransportConfig,
    /// Shared shutdown signal — when `true`, writes return `Shutdown` error.
    shutdown: Arc<AtomicBool>,
    /// The stdout writer, protected by a mutex for concurrent access.
    /// When `None`, `tokio::io::stdout()` is used.
    stdout: tokio::sync::Mutex<Option<Box<dyn AsyncWrite + Unpin + Send>>>>,
}

impl AsyncStdoutWriter {
    /// Create a new writer with the given config and shutdown signal.
    ///
    /// If `stdout` is `None`, the writer uses `tokio::io::stdout()`.
    pub fn new(
        config: TransportConfig,
        shutdown: Arc<AtomicBool>,
        stdout: Option<Box<dyn AsyncWrite + Uninit + Send>>,
    ) -> Self {
        Self {
            config,
            shutdown,
            stdout: tokio::sync::Mutex::new(stdout),
        }
    }

    /// Returns `true` if the shutdown flag has been set.
    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Serialize a response and write it to stdout.
    ///
    /// The response is serialized to a single JSON line with a trailing
    /// newline. If `flush_after_write` is enabled (default), the output
    /// is flushed immediately after writing.
    pub async fn write_response(&self, response: &Response) -> TransportResult<()> {
        if self.is_shutdown() {
            return Err(TransportError::Shutdown);
        }

        let encoded = encode_message(response)?;

        let mut stdout = self.stdout.lock().await;

        if self.is_shutdown() {
            return Err(TransportError::Shutdown);
        }

        let write_result = match &mut *stdout {
            Some(w) => w.write_all(encoded.as_bytes()).await,
            None => tokio::io::stdout().write_all(encoded.as_bytes()).await,
        };

        if let Err(e) = write_result {
            return Err(TransportError::WriteFailed { source: e });
        }

        if self.config.flush_after_write {
            let flush_result = match &mut *stdout {
                Some(w) => w.flush().await,
                None => tokio::io::stdout().flush().await,
            };
            if let Err(e) = flush_result {
                return Err(TransportError::WriteFailed { source: e });
            }
        }

        Ok(())
    }

    /// Write a raw JSON string (already encoded) to stdout.
    ///
    /// This is useful for writing responses that have already been serialized,
    /// avoiding duplicate serialization. The `encoded` string should already
    /// include the trailing newline.
    pub async fn write_raw(&self, encoded: &str) -> TransportResult<()> {
        if self.is_shutdown() {
            return Err(TransportError::Shutdown);
        }

        let mut stdout = self.stdout.lock().await;

        if self.is_shutdown() {
            return Err(TransportError::Shutdown);
        }

        let write_result = match &mut *stdout {
            Some(w) => w.write_all(encoded.as_bytes()).await,
            None => tokio::io::stdout().write_all(encoded.as_bytes()).await,
        };

        if let Err(e) = write_result {
            return Err(TransportError::WriteFailed { source: e });
        }

        if self.config.flush_after_write {
            let flush_result = match &mut *stdout {
                Some(w) => w.flush().await,
                None => tokio::io::stdout().flush().await,
            };
            if let Err(e) = flush_result {
                return Err(TransportError::WriteFailed { source: e });
            }
        }

        Ok(())
    }

    /// Flush any buffered output to stdout.
    ///
    /// Should be called during graceful shutdown to ensure all
    /// pending responses have been written.
    pub async fn flush(&self) -> TransportResult<()> {
        if self.is_shutdown() {
            return Err(TransportError::Shutdown);
        }

        let mut stdout = self.stdout.lock().await;

        let flush_result = match &mut *stdout {
            Some(w) => w.flush().await,
            None => tokio::io::stdout().flush().await,
        };

        flush_result.map_err(|e| TransportError::WriteFailed { source: e })
    }
}

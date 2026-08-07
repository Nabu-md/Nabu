//! Configuration for the stdio JSON-RPC transport.
//!
//! Encapsulates all tunable parameters so that future protocol servers
//! (ACP, MCP, etc.) can configure the transport without modifying its internals.

/// Maximum line length in bytes before the reader returns an error.
///
/// This protects against unbounded memory usage from malicious or
/// malformed input. The default of 1 MiB is generous for JSON-RPC
/// messages while still providing a hard ceiling.
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// How long the reader waits for stdin data before checking the shutdown
/// signal. This keeps the transport responsive to shutdown requests even
/// when no input is arriving.
pub const DEFAULT_READ_POLL_INTERVAL_MS: u64 = 100;

/// How long the writer waits for stdout to become writable before checking
/// the shutdown signal.
pub const DEFAULT_WRITE_TIMEOUT_MS: u64 = 5000;

/// Configuration for [`StdioTransport`](super::StdioTransport).
///
/// All fields have sensible defaults and can be overridden via builder-style
/// methods. The config is `Clone` so it can be cheaply shared or copied.
#[derive(Clone, Debug)]
pub struct TransportConfig {
    /// Maximum message size in bytes (per line of input).
    ///
    /// Messages exceeding this size cause a [`TransportError::MessageTooLarge`]
    /// on the reader side.
    pub max_message_bytes: usize,

    /// Shutdown timeout — how long the transport waits for the reader and
    /// writer tasks to complete during graceful shutdown.
    pub shutdown_timeout: std::time::Duration,

    /// Whether to flush stdout after every response written.
    ///
    /// In long-lived protocol servers this is typically `true` — each
    /// response must be delivered immediately so the client can read it.
    /// Disabling flushing is useful in benchmarks or batch scenarios.
    pub flush_after_write: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            shutdown_timeout: std::time::Duration::from_secs(5),
            flush_after_write: true,
        }
    }
}

impl TransportConfig {
    /// Create a new config with the given maximum message size in bytes.
    pub fn with_max_message_bytes(self, max: usize) -> Self {
        Self {
            max_message_bytes: max,
            ..self
        }
    }

    /// Set the shutdown timeout duration.
    pub fn with_shutdown_timeout(self, timeout: std::time::Duration) -> Self {
        Self {
            shutdown_timeout: timeout,
            ..self
        }
    }

    /// Enable or disable flushing stdout after each response.
    pub fn with_flush_after_write(self, flush: bool) -> Self {
        Self {
            flush_after_write: flush,
            ..self
        }
    }
}

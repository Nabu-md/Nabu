//! Structured errors for IPC socket operations.
//!
//! Every error variant corresponds to a specific failure mode in the socket
//! lifecycle, enabling callers to distinguish between (for example) a stale
//! socket that could not be removed and a permission failure.

use std::path::PathBuf;

/// Errors that can occur during socket lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum SocketError {
    /// A low-level I/O error occurred (bind, accept, read, write, etc.).
    #[error("IO error on socket '{path}': {source}")]
    Io {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },

    /// A stale socket file was found at the target path and could not be
    /// removed before binding.
    #[error("Stale socket file could not be removed at '{path}': {source}")]
    StaleSocketRemove {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },

    /// The socket file existed but was not a socket (it was a regular file,
    /// directory, or other special file).
    #[error("Path '{path}' exists but is not a socket")]
    NotASocket { path: PathBuf },

    /// Failed to apply secure file permissions (`0600`) to the socket.
    #[error("Failed to set socket permissions to 0600 at '{path}': {source}")]
    PermissionDenied {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },

    /// The socket is not in a valid lifecycle stage for the requested
    /// operation (e.g. calling `start()` before `initialize()`, or calling
    /// `shutdown()` on an already-shut-down socket).
    #[error("Invalid lifecycle transition for socket '{path}': {message}")]
    LifecycleError {
        path: PathBuf,
        message: String,
    },

    /// The tokio runtime is not available on the current thread — required
    /// for spawning the accept loop task.
    #[error("No tokio runtime available to spawn socket server task")]
    NoRuntime,

    /// The accept loop task panicked or was cancelled unexpectedly.
    #[error("Socket accept loop terminated unexpectedly: {reason}")]
    AcceptLoopError { reason: String },

    /// Attempted to shut down a socket that was never started or had
    /// already been shut down.
    #[error("Socket handle is already shut down or was never started")]
    AlreadyShutdown,
}

impl SocketError {
    /// Wraps a `std::io::Error` with the socket path context.
    pub fn io(source: std::io::Error, path: PathBuf) -> Self {
        SocketError::Io { source, path }
    }

    /// Creates a lifecycle error with a descriptive message.
    pub fn lifecycle(path: PathBuf, message: impl Into<String>) -> Self {
        SocketError::LifecycleError {
            path,
            message: message.into(),
        }
    }
}

/// Convenience type alias for results in this module.
pub type SocketResult<T> = Result<T, SocketError>;

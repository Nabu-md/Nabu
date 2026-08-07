//! Socket manager — lifecycle-managed UNIX domain socket server.
//!
//! [`SocketManager`] encapsulates the full lifecycle of a UNIX domain socket
//! server, from creation through shutdown:
//!
//! 1. **Created** — configuration loaded, no filesystem operations yet.
//! 2. **Initialized** — stale socket cleaned, path validated, directory exists.
//! 3. **Running** — listener bound with `0600` permissions, accept loop
//!    active, dispatching connections to the handler.
//! 4. **Shutdown** — accept loop stopped, listener closed, socket file
//!    removed.
//!
//! The manager is `Send + Sync` and designed to be shared via `Arc` across
//! threads, participating in the standard [`Lifecycle`] trait.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};

use super::errors::{SocketError, SocketResult};
use super::lifecycle::SocketLifecycleManager;

/// Secure permissions for socket files: owner read/write only.
///
/// `0600` = `rw-------` — the tightest meaningful permission set for a local
/// IPC socket. Blocks all other users from connecting.
pub const SECURE_SOCKET_PERMISSIONS: u32 = 0o600;

/// A handler invoked for each accepted connection.
///
/// The closure receives the [`UnixStream`] for the connection and must
/// return `true` if it took ownership and will process the stream, or `false`
/// if the connection should be immediately closed (e.g. the server is shutting
/// down). This allows the handler to participate in graceful shutdown.
pub type ConnectionHandler = Arc<
    dyn Fn(UnixStream) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

/// Configuration for [`SocketManager`].
///
/// All fields are `Clone` so the config can be cheaply duplicated for tests
/// or shared socket path constants.
#[derive(Clone)]
pub struct SocketConfig {
    /// The filesystem path for the UNIX domain socket.
    pub socket_path: PathBuf,

    /// The handler invoked for each accepted connection.
    ///
    /// This is `Option` so that the config can be constructed and validated
    /// (including stale-socket cleanup) without requiring a runtime context
    /// to provide a handler. The handler is set via [`SocketConfig::with_handler`]
    /// before [`SocketManager::start_socket`].
    pub handler: Option<ConnectionHandler>,

    /// Maximum accepted message size in bytes (for documentation/logging).
    /// The handler itself enforces size limits.
    pub max_message_size: usize,

    /// Timeout for graceful shutdown — how long to wait for the accept loop
    /// to terminate before force-closing.
    pub shutdown_timeout: std::time::Duration,

    /// Custom socket directory. If `None`, the parent directory of
    /// `socket_path` is used.
    pub socket_dir: Option<PathBuf>,
}

impl SocketConfig {
    /// Create a new config with the given socket path.
    pub fn new<P: Into<PathBuf>>(socket_path: P) -> Self {
        Self {
            socket_path: socket_path.into(),
            handler: None,
            max_message_size: 10 * 1024 * 1024,
            shutdown_timeout: std::time::Duration::from_secs(5),
            socket_dir: None,
        }
    }

    /// Attach a connection handler.
    pub fn with_handler<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(UnixStream) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = bool> + Send + 'static,
    {
        let handler: ConnectionHandler = Arc::new(move |stream| {
            Box::pin((handler)(stream))
        });
        Self {
            handler: Some(handler),
            ..self
        }
    }

    /// Set the maximum message size accepted by the server.
    pub fn with_max_message_size(self, size: usize) -> Self {
        Self {
            max_message_size: size,
            ..self
        }
    }

    /// Set the shutdown timeout.
    pub fn with_shutdown_timeout(self, duration: std::time::Duration) -> Self {
        Self {
            shutdown_timeout: duration,
            ..self
        }
    }

    /// Set a custom socket directory (parent of the socket file).
    pub fn with_socket_dir<P: Into<PathBuf>>(self, dir: P) -> Self {
        Self {
            socket_dir: Some(dir.into()),
            ..self
        }
    }

    /// Returns the directory that should contain the socket file.
    pub fn effective_socket_dir(&self) -> PathBuf {
        self.socket_dir
            .clone()
            .unwrap_or_else(|| {
                self.socket_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
            })
    }
}

/// Handle for controlling a running [`SocketManager`].
///
/// This is a lightweight handle (no owned resources) that can be held by
/// multiple callers to signal shutdown. It is `Clone`, `Send`, and `Sync`.
#[derive(Clone)]
pub struct SocketManagerHandle {
    /// Signal to stop the accept loop.
    shutdown_notify: Arc<Notify>,
    /// Set to `true` once shutdown has been initiated (prevents double-shutdown).
    shutdown_initiated: Arc<AtomicBool>,
    /// Current lifecycle stage (atomic, lock-free).
    lifecycle: SocketLifecycleManager,
    /// The socket path (for cleanup).
    socket_path: PathBuf,
}

impl SocketManagerHandle {
    /// Returns the path to the socket file.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Returns the current lifecycle stage.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage().into()
    }

    /// Returns `true` if the socket is currently running and accepting
    /// connections.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the socket has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    /// Signal the accept loop to shut down.
    ///
    /// This is safe to call multiple times — only the first call triggers
    /// the shutdown notification; subsequent calls are no-ops.
    ///
    /// Note: this signals the accept loop but does not synchronously wait
    /// for it to finish. The actual cleanup (closing the listener, removing
    /// the socket file) happens inside the accept loop task when it receives
    /// the notification.
    pub fn signal_shutdown(&self) {
        if self
            .shutdown_initiated
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            self.shutdown_notify.notify_one();
        }
    }
}

/// A lifecycle-managed UNIX domain socket server.
///
/// `SocketManager` owns the [`SocketConfig`], a handle for external control,
/// and the tokio join handle for the accept loop. It implements
/// [`Lifecycle`] so it participates in the standard service lifecycle:
///
/// ```text
/// Created → Initialized → Running → Shutdown
/// ```
///
/// ## Thread Safety
///
/// `SocketManager` is `Send + Sync`. The accept loop runs as a tokio task
/// and communicates via `Arc<Notify>` for shutdown signaling. The join handle
/// is stored behind a `std::sync::Mutex` (not held across await points),
/// and all other mutable state is protected by atomics.
///
/// ## Usage
///
/// ```no_run
/// use nabu_core::ipc_socket::{SocketConfig, SocketManager};
/// use nabu_core::registry::lifecycle::Lifecycle;
///
/// let config = SocketConfig::new("/tmp/myapp.sock")
///     .with_handler(|stream| async move {
///         // process the connection
///         true
///     });
///
/// let manager = SocketManager::new(config);
/// manager.initialize().unwrap();
/// manager.start().unwrap();
///
/// // ... app runs ...
///
/// manager.shutdown().unwrap();
/// ```
pub struct SocketManager {
    /// Configuration for this socket.
    config: SocketConfig,
    /// Handle for external control (cloneable, no owned resources).
    handle: SocketManagerHandle,
    /// The tokio JoinHandle for the accept loop task.
    /// `None` before `start()` is called.
    /// Protected by `StdMutex` (not `tokio::sync::Mutex`) because we only
    /// need to `take()` the handle during shutdown — no async operations
    /// are needed while holding the lock.
    accept_handle: Arc<StdMutex<Option<JoinHandle<()>>>>,
    /// The LifecycleManager for trait integration with ApplicationContext.
    lifecycle: LifecycleManager,
}

impl SocketManager {
    /// Create a new `SocketManager` with the given configuration.
    ///
    /// The socket is **not** yet bound or listening — call
    /// [`initialize_socket`](Self::initialize_socket) and [`start_socket`](Self::start_socket).
    pub fn new(config: SocketConfig) -> Self {
        let socket_path = config.socket_path.clone();
        let lifecycle = LifecycleManager::new();

        Self {
            config,
            handle: SocketManagerHandle {
                shutdown_notify: Arc::new(Notify::new()),
                shutdown_initiated: Arc::new(AtomicBool::new(false)),
                lifecycle: SocketLifecycleManager::new(),
                socket_path,
            },
            accept_handle: Arc::new(StdMutex::new(None)),
            lifecycle,
        }
    }

    /// Returns a clone of the [`SocketManagerHandle`] for external control.
    ///
    /// The handle can be used to signal shutdown or query lifecycle state
    /// from other threads/tasks.
    pub fn handle(&self) -> SocketManagerHandle {
        self.handle.clone()
    }

    /// Returns the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.config.socket_path
    }

    /// Returns the current lifecycle stage.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Returns `true` if the path at `socket_path` is a stale (dangling)
    /// socket — i.e. it exists as a socket but no process is listening on it.
    ///
    /// Detection strategy:
    /// 1. If the path doesn't exist, it's not stale.
    /// 2. If the path exists but is not a socket, it's not a stale socket
    ///    (it could be a regular file — handled separately).
    /// 3. If the path is a socket, we attempt a quick `connect()`. If the
    ///    connect succeeds, the socket is live and should NOT be removed.
    ///    If it fails with "connection refused", the socket is stale.
    fn is_stale_socket(socket_path: &Path) -> bool {
        use std::os::unix::fs::FileTypeExt;
        use std::os::unix::net::UnixStream as StdUnixStream;

        match std::fs::symlink_metadata(socket_path) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket() {
                    return false;
                }
            }
            Err(_) => {
                return false;
            }
        }

        match StdUnixStream::connect(socket_path) {
            Ok(_) => false,
            Err(e) => {
                tracing::debug!(
                    "Socket at '{}' appears stale (connect failed: {})",
                    socket_path.display(),
                    e
                );
                true
            }
        }
    }

    /// Removes a stale socket file at `socket_path`.
    ///
    /// Only removes the file if it is detected as a stale socket or a
    /// non-socket file (e.g. a regular file left behind by a crashed process).
    /// Does nothing if the path doesn't exist or is a live socket.
    fn cleanup_stale_socket(socket_path: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};

        if !socket_path.exists() {
            return Ok(());
        }

        let metadata = std::fs::symlink_metadata(socket_path)?;

        if metadata.file_type().is_socket() {
            if Self::is_stale_socket(socket_path) {
                tracing::info!(
                    "Removing stale socket file at '{}'",
                    socket_path.display()
                );
                std::fs::remove_file(socket_path)?;
            } else {
                tracing::debug!(
                    "Socket at '{}' is live — not removing",
                    socket_path.display()
                );
            }
        } else if metadata.file_type().is_file() {
            tracing::warn!(
                "Non-socket file found at socket path '{}' — removing",
                socket_path.display()
            );
            let perms = metadata.permissions();
            let mode = perms.mode();
            if mode & 0o200 == 0 {
                let _ = std::fs::set_permissions(
                    socket_path,
                    std::fs::Permissions::from_mode(0o600),
                );
            }
            std::fs::remove_file(socket_path)?;
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "A directory or special file already exists at the socket path '{}'",
                    socket_path.display()
                ),
            ));
        }

        Ok(())
    }

    /// Ensures the parent directory of the socket path exists.
    fn ensure_socket_dir(socket_path: &Path) -> std::io::Result<()> {
        if let Some(parent) = socket_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }

    /// Applies `0600` permissions to the socket file.
    ///
    /// On Unix, this is done via `set_permissions` with mode `0600`
    /// (`rw-------`) — owner read/write only. This blocks all other users
    /// from connecting to or impersonating the socket.
    fn apply_secure_permissions(socket_path: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let perms = std::fs::Permissions::from_mode(SECURE_SOCKET_PERMISSIONS);
        std::fs::set_permissions(socket_path, perms)?;

        tracing::debug!(
            "Socket '{}' permissions set to 0600",
            socket_path.display()
        );

        Ok(())
    }

    /// Spawns the accept loop as a tokio task.
    ///
    /// The accept loop:
    /// 1. Waits for either a new connection or a shutdown signal.
    /// 2. On connection, dispatches to the handler in a separate tokio task.
    /// 3. On shutdown signal, breaks the loop and removes the socket file.
    fn spawn_accept_loop(
        listener: UnixListener,
        handler: ConnectionHandler,
        handle: SocketManagerHandle,
        socket_path: PathBuf,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            tracing::info!(
                "Socket accept loop started on '{}'",
                socket_path.display()
            );

            loop {
                tokio::select! {
                    _ = handle.shutdown_notify.notified() => {
                        tracing::info!(
                            "Socket shutdown signal received on '{}'",
                            socket_path.display()
                        );
                        break;
                    }
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _addr)) => {
                                tracing::debug!(
                                    "Accepted connection on '{}'",
                                    socket_path.display()
                                );
                                let handler = handler.clone();
                                tokio::spawn(async move {
                                    match handler(stream).await {
                                        true => {
                                            tracing::debug!("Connection handled successfully");
                                        }
                                        false => {
                                            tracing::debug!("Handler declined connection");
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                if handle.shutdown_initiated.load(Ordering::Acquire) {
                                    tracing::debug!(
                                        "Accept error during shutdown: {}",
                                        e
                                    );
                                    break;
                                }
                                tracing::error!(
                                    "Socket accept error on '{}': {}",
                                    socket_path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
            }

            if let Err(e) = std::fs::remove_file(&socket_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::error!(
                        "Failed to remove socket file '{}': {}",
                        socket_path.display(),
                        e
                    );
                } else {
                    tracing::debug!(
                        "Socket file '{}' already removed",
                        socket_path.display()
                    );
                }
            } else {
                tracing::info!(
                    "Socket file removed at '{}'",
                    socket_path.display()
                );
            }

            tracing::info!("Socket accept loop terminated");
        })
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Initializes the socket manager.
    ///
    /// This performs:
    /// 1. Ensures the socket directory exists.
    /// 2. Cleans up any stale socket file at the target path.
    ///
    /// Does NOT bind the listener — that happens in [`start_socket`](Self::start_socket).
    ///
    /// # Errors
    ///
    /// Returns [`SocketError`] if:
    /// - The socket directory cannot be created.
    /// - A stale socket file cannot be removed.
    /// - The path exists but is not a socket and cannot be removed.
    pub fn initialize_socket(&self) -> SocketResult<()> {
        let socket_path = self.config.socket_path.clone();

        Self::ensure_socket_dir(&socket_path)
            .map_err(|e| SocketError::io(e, socket_path.clone()))?;

        Self::cleanup_stale_socket(&socket_path)
            .map_err(|e| SocketError::StaleSocketRemove {
                source: e,
                path: socket_path.clone(),
            })?;

        self.handle
            .lifecycle
            .transition_to(super::lifecycle::SocketLifecycle::Initialized)
            .map_err(|_| {
                SocketError::lifecycle(
                    socket_path.clone(),
                    "Already initialized or shut down",
                )
            })?;

        self.lifecycle
            .transition_to(LifecycleStage::Initialized)
            .map_err(|e| SocketError::lifecycle(socket_path.clone(), e.to_string()))?;

        tracing::info!("Socket '{}' initialized", socket_path.display());
        Ok(())
    }

    /// Starts the socket server.
    ///
    /// This must be called after [`initialize_socket`](Self::initialize_socket).
    ///
    /// Performs:
    /// 1. Validates lifecycle stage (must be at least `Initialized`).
    /// 2. Validates that a handler is configured.
    /// 3. Binds the `UnixListener` to the socket path.
    /// 4. Applies `0600` permissions to the socket file.
    /// 5. Spawns the accept loop as a tokio task.
    /// 6. Transitions to `Running`.
    ///
    /// # Errors
    ///
    /// Returns [`SocketError`] if:
    /// - Not in the correct lifecycle stage.
    /// - No handler is configured.
    /// - No tokio runtime is available.
    /// - `UnixListener::bind` fails.
    /// - Permission application fails.
    pub fn start_socket(&self) -> SocketResult<()> {
        let socket_path = self.config.socket_path.clone();

        if self.handle.lifecycle.is_shutdown() {
            return Err(SocketError::AlreadyShutdown);
        }

        if !self
            .handle
            .lifecycle
            .is_at_least(super::lifecycle::SocketLifecycle::Initialized)
        {
            return Err(SocketError::lifecycle(
                socket_path.clone(),
                "Socket must be initialized before starting",
            ));
        }

        if self.handle.lifecycle.is_running() {
            tracing::warn!(
                "Socket '{}' is already running",
                socket_path.display()
            );
            return Ok(());
        }

        let handler = self.config.handler.clone().ok_or_else(|| {
            SocketError::lifecycle(
                socket_path.clone(),
                "No connection handler configured — call with_handler() before start_socket()",
            )
        })?;

        // Verify we're inside a tokio runtime.
        tokio::runtime::Handle::try_current()
            .map_err(|_| SocketError::NoRuntime)?;

        tracing::info!("Starting socket server on '{}'", socket_path.display());

        let listener = UnixListener::bind(&socket_path)
            .map_err(|e| SocketError::io(e, socket_path.clone()))?;

        if let Err(e) = Self::apply_secure_permissions(&socket_path) {
            let _ = std::fs::remove_file(&socket_path);
            return Err(SocketError::PermissionDenied {
                source: e,
                path: socket_path,
            });
        }

        self.handle
            .lifecycle
            .transition_to(super::lifecycle::SocketLifecycle::Running)
            .map_err(|_| {
                SocketError::lifecycle(socket_path.clone(), "Failed to transition to Running")
            })?;

        let socket_path_for_err = socket_path.clone();
        self.lifecycle
            .transition_to(LifecycleStage::Running)
            .map_err(|e| SocketError::lifecycle(socket_path_for_err, e.to_string()))?;

        let accept_handle = Self::spawn_accept_loop(
            listener,
            handler,
            self.handle.clone(),
            socket_path.clone(),
        );

        {
            let mut guard = self.accept_handle.lock().unwrap();
            *guard = Some(accept_handle);
        }

        tracing::info!(
            "Socket server started on '{}' with 0600 permissions",
            socket_path.display()
        );

        Ok(())
    }

    /// Shuts down the socket server gracefully.
    ///
    /// This performs:
    /// 1. Signals the accept loop to stop (no new connections accepted).
    /// 2. Aborts the accept loop task if it hasn't finished.
    /// 3. Ensures the socket file is removed.
    /// 4. Transitions to `Shutdown`.
    ///
    /// This method is safe to call multiple times — subsequent calls are no-ops.
    pub fn shutdown_socket(&self) -> SocketResult<()> {
        let socket_path = self.config.socket_path.clone();

        if self.handle.lifecycle.is_shutdown() {
            tracing::debug!(
                "Socket '{}' already shut down — no-op",
                socket_path.display()
            );
            return Ok(());
        }

        tracing::info!("Shutting down socket '{}'", socket_path.display());

        self.handle.signal_shutdown();

        self.handle
            .lifecycle
            .transition_to(super::lifecycle::SocketLifecycle::Shutdown)
            .ok();

        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)
            .map_err(|e| SocketError::lifecycle(socket_path.clone(), e.to_string()))?;

        // Abort the accept loop task.
        {
            let mut guard = self.accept_handle.lock().unwrap();
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }

        // Ensure the socket file is removed even if the accept loop didn't get to it.
        if socket_path.exists() {
            if let Err(e) = std::fs::remove_file(&socket_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::error!(
                        "Failed to remove socket file '{}' during shutdown: {}",
                        socket_path.display(),
                        e
                    );
                    return Err(SocketError::io(e, socket_path));
                }
            } else {
                tracing::info!(
                    "Socket file removed during shutdown: '{}'",
                    socket_path.display()
                );
            }
        }

        tracing::info!("Socket '{}' shut down", socket_path.display());
        Ok(())
    }

    /// Returns `true` if the socket is running and accepting connections.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the socket has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }
}

impl Drop for SocketManager {
    fn drop(&mut self) {
        self.handle.shutdown_initiated.store(true, Ordering::Release);
        self.handle.shutdown_notify.notify_one();

        if let Ok(mut guard) = self.accept_handle.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }

        if self.config.socket_path.exists() {
            if let Err(e) = std::fs::remove_file(&self.config.socket_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::debug!(
                        "Failed to remove socket file '{}' in Drop: {}",
                        self.config.socket_path.display(),
                        e
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

impl Lifecycle for SocketManager {
    fn name(&self) -> &'static str {
        "ipc_socket"
    }

    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.initialize_socket()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.start_socket()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown_socket()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

impl std::fmt::Debug for SocketManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SocketManager")
            .field("socket_path", &self.config.socket_path)
            .field("lifecycle_stage", &self.lifecycle.stage())
            .field("is_running", &self.lifecycle.is_running())
            .field("is_shutdown", &self.lifecycle.is_shutdown())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn test_socket_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nabu-socket-test-{}", name));
        std::fs::create_dir_all(&dir).ok();
        dir.join("socket.sock")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn socket_lifecycle_initialized_running_shutdown() {
        let socket_path = test_socket_path("lifecycle.sock");
        let _ = std::fs::remove_file(&socket_path);

        let handler_call_count = Arc::new(AtomicUsize::new(0));
        let handler_call_count_clone = handler_call_count.clone();

        let config = SocketConfig::new(socket_path.clone()).with_handler(move |_stream| {
            handler_call_count_clone.fetch_add(1, AtomicOrdering::SeqCst);
            async move { true }
        });

        let manager = SocketManager::new(config);

        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Created);
        assert!(!manager.is_running());
        assert!(!manager.is_shutdown());

        manager.initialize_socket().expect("initialize should succeed");
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Initialized);

        manager.start_socket().expect("start should succeed");
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Running);
        assert!(manager.is_running());

        assert!(socket_path.exists());
        let metadata = std::fs::metadata(&socket_path)
            .expect("socket file should exist");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o600,
                "socket should have 0600 permissions, got {:o}",
                mode & 0o777
            );
        }

        manager.shutdown_socket().expect("shutdown should succeed");
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Shutdown);
        assert!(manager.is_shutdown());
        assert!(!manager.is_running());

        assert!(
            !socket_path.exists(),
            "socket file should be removed after shutdown"
        );

        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stale_socket_is_cleaned_up_on_initialize() {
        let socket_path = test_socket_path("stale.sock");
        let _ = std::fs::remove_file(&socket_path);

        std::fs::write(&socket_path, b"stale").expect("write fake stale socket");

        let config = SocketConfig::new(socket_path.clone()).with_handler(|_stream| async move {
            true
        });

        let manager = SocketManager::new(config);
        manager.initialize_socket().expect("initialize should succeed");
        assert!(!socket_path.exists());

        manager.start_socket().expect("start should succeed");
        manager.shutdown_socket().expect("shutdown should succeed");

        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_succeeds_without_manual_cleanup() {
        let socket_path = test_socket_path("restart.sock");
        let _ = std::fs::remove_file(&socket_path);

        let make_handler = || {
            let count = Arc::new(AtomicUsize::new(0));
            let count_clone = count.clone();
            (
                move |_stream: UnixStream| {
                    count_clone.fetch_add(1, AtomicOrdering::SeqCst);
                    async move { true }
                },
                count,
            )
        };

        {
            let (handler, _count) = make_handler();
            let config = SocketConfig::new(socket_path.clone()).with_handler(handler);
            let mgr = SocketManager::new(config);
            mgr.initialize_socket().expect("init 1");
            mgr.start_socket().expect("start 1");
            mgr.shutdown_socket().expect("shutdown 1");
        }

        {
            let (handler, _count) = make_handler();
            let config = SocketConfig::new(socket_path.clone()).with_handler(handler);
            let mgr = SocketManager::new(config);
            mgr.initialize_socket().expect("init 2");
            mgr.start_socket().expect("start 2");
            mgr.shutdown_socket().expect("shutdown 2");
        }

        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_without_handler_fails() {
        let socket_path = test_socket_path("no-handler.sock");
        let _ = std::fs::remove_file(&socket_path);

        let config = SocketConfig::new(socket_path.clone());
        let manager = SocketManager::new(config);

        manager.initialize_socket().expect("init");
        let result = manager.start_socket();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SocketError::LifecycleError { .. }));

        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_without_initialize_fails() {
        let socket_path = test_socket_path("no-init.sock");
        let _ = std::fs::remove_file(&socket_path);

        let config = SocketConfig::new(socket_path.clone()).with_handler(|_stream| async move {
            true
        });
        let manager = SocketManager::new(config);

        let result = manager.start_socket();
        assert!(result.is_err());

        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn double_shutdown_is_safe() {
        let socket_path = test_socket_path("double-shutdown.sock");
        let _ = std::fs::remove_file(&socket_path);

        let config = SocketConfig::new(socket_path.clone()).with_handler(|_stream| async move {
            true
        });
        let manager = SocketManager::new(config);

        manager.initialize_socket().expect("init");
        manager.start_socket().expect("start");
        manager.shutdown_socket().expect("shutdown 1");
        manager.shutdown_socket().expect("shutdown 2");

        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[test]
    fn is_stale_socket_detects_stale_file() {
        let dir = std::env::temp_dir().join("nabu-stale-test");
        std::fs::create_dir_all(&dir).ok();
        let socket_path = dir.join("stale.sock");
        let _ = std::fs::remove_file(&socket_path);

        assert!(!SocketManager::is_stale_socket(&socket_path));

        std::fs::write(&socket_path, b"not a socket").expect("write");
        assert!(!SocketManager::is_stale_socket(&socket_path));

        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_stale_socket_returns_false_for_live_socket() {
        use std::os::unix::net::UnixListener as StdUnixListener;

        let dir = std::env::temp_dir().join("nabu-stale-live-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        let socket_path = dir.join("live.sock");
        let _ = std::fs::remove_file(&socket_path);

        let listener = StdUnixListener::bind(&socket_path).expect("bind");

        assert!(
            !SocketManager::is_stale_socket(&socket_path),
            "live socket should not be detected as stale"
        );

        drop(listener);
        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connection_handler_receives_data() {
        let socket_path = test_socket_path("connect-test.sock");
        let _ = std::fs::remove_file(&socket_path);

        let received_data = Arc::new(StdMutex::new(String::new()));
        let received_data_clone = received_data.clone();

        let config = SocketConfig::new(socket_path.clone()).with_handler(move |mut stream| {
            let received = received_data_clone.clone();
            async move {
                let mut buf = vec![0u8; 1024];
                match stream.read(&mut buf).await {
                    Ok(n) => {
                        let s = String::from_utf8_lossy(&buf[..n]).to_string();
                        received.lock().unwrap().push_str(&s);
                        let _ = stream.write_all(b"ok").await;
                    }
                    Err(e) => {
                        tracing::error!("Read error: {}", e);
                    }
                }
                true
            }
        });

        let manager = SocketManager::new(config);
        manager.initialize_socket().expect("init");
        manager.start_socket().expect("start");

        let mut stream = tokio::net::UnixStream::connect(&socket_path)
            .await
            .expect("connect");
        stream.write_all(b"hello socket").await.expect("write");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut response = vec![0u8; 10];
        let _ = stream.read(&mut response).await;

        let received = received_data.lock().unwrap().clone();
        assert_eq!(received, "hello socket");

        manager.shutdown_socket().expect("shutdown");
        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shutdown_prevents_new_connections() {
        let socket_path = test_socket_path("shutdown-block.sock");
        let _ = std::fs::remove_file(&socket_path);

        let config = SocketConfig::new(socket_path.clone()).with_handler(|_stream| async move {
            true
        });
        let manager = SocketManager::new(config);
        manager.initialize_socket().expect("init");
        manager.start_socket().expect("start");
        manager.shutdown_socket().expect("shutdown");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let result = tokio::net::UnixStream::connect(&socket_path).await;
        assert!(
            result.is_err(),
            "Should not be able to connect after shutdown"
        );

        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }

    #[test]
    fn config_new_sets_defaults() {
        let config = SocketConfig::new("/tmp/test.sock");
        assert_eq!(config.socket_path, PathBuf::from("/tmp/test.sock"));
        assert!(config.handler.is_none());
        assert_eq!(config.max_message_size, 10 * 1024 * 1024);
        assert_eq!(config.shutdown_timeout, std::time::Duration::from_secs(5));
    }

    #[test]
    fn config_with_handler_sets_handler() {
        let config = SocketConfig::new("/tmp/test.sock").with_handler(|_stream| async move {
            true
        });
        assert!(config.handler.is_some());
    }

    #[test]
    fn handle_is_cloneable() {
        let socket_path = test_socket_path("handle-clone.sock");
        let _ = std::fs::remove_file(&socket_path);

        let config = SocketConfig::new(socket_path.clone()).with_handler(|_stream| async move {
            true
        });
        let manager = SocketManager::new(config);
        let h1 = manager.handle();
        let h2 = manager.handle();

        assert_eq!(h1.socket_path(), h2.socket_path());
        assert!(!h1.is_shutdown());
        assert!(!h2.is_shutdown());

        h1.signal_shutdown();
        h2.signal_shutdown();
    }

    #[test]
    fn lifecycle_trait_name() {
        let config = SocketConfig::new("/tmp/test.sock");
        let manager = SocketManager::new(config);
        assert_eq!(manager.name(), "ipc_socket");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn drop_removes_socket_file() {
        let socket_path = test_socket_path("drop-cleanup.sock");
        let _ = std::fs::remove_file(&socket_path);

        let manager = SocketManager::new(
            SocketConfig::new(socket_path.clone()).with_handler(|_stream| async move {
                true
            }),
        );
        manager.initialize_socket().expect("init");
        manager.start_socket().expect("start");
        assert!(socket_path.exists());

        drop(manager);

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(!socket_path.exists(), "Drop should remove socket file");

        let _ = std::fs::remove_dir_all(socket_path.parent().unwrap());
    }
}

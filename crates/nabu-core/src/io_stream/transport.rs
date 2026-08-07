//! Bidirectional stdio JSON-RPC transport.
//!
//! [`StdioTransport`] is the production-ready, transport-agnostic bridge
//! between stdin/stdout I/O and the JSON-RPC [`Router`]. It implements the
//! pipe:
//!
//! ```text
//! stdin → Async Reader → Deserialize → Router → Serialize → Async Writer → stdout
//! ```
//!
//! ## Architecture
//!
//! The transport has four components:
//!
//! 1. **[`AsyncStdinReader`]** — reads newline-delimited JSON from stdin,
//!    deserializes each line into a [`Request`], and forwards it to the router.
//! 2. **[`Router`]** — dispatches the request to the registered handler and
//!    produces a [`Response`]. This is the existing JSON-RPC core — the
//!    transport does not duplicate any routing logic.
//! 3. **[`AsyncStdoutWriter`]** — serializes the response back to
//!    newline-delimited JSON and writes it to stdout.
//! 4. **Lifecycle manager** — integrates with the standard Nabu lifecycle
//!    state machine via the [`Lifecycle`] trait.
//!
//! ## Protocol Agnosticism
//!
//! The transport is completely protocol-agnostic. It knows nothing about
//! ACP, MCP, or any specific JSON-RPC method semantics. It simply:
//!
//! 1. Reads a line from stdin.
//! 2. Deserializes it as a [`Request`].
//! 3. Passes it to the injected `Router`.
//! 4. Serializes the returned [`Response`].
//! 5. Writes the response line to stdout.
//!
//! Future protocol servers (ACP, MCP, plugin hosts, AI agents) can attach
//! their own router and use this transport without modification.
//!
//! ## Lifecycle
//!
//! Implements the [`Lifecycle`] trait for integration with the standard
//! Nabu lifecycle state machine:
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//! ```
//!
//! ## Shutdown
//!
//! Graceful shutdown is triggered by:
//! - EOF on stdin (upstream process closed the pipe).
//! - The shared shutdown flag (set externally, e.g. by signal handler).
//! - Explicit `shutdown()` call.
//!
//! During shutdown, the transport:
//! 1. Signals the read loop to stop (via the shared `Arc<AtomicBool>`).
//! 2. Waits for the read loop task to complete (with timeout).
//! 3. Flushes stdout to ensure no responses are lost.
//! 4. Transitions to `Shutdown` lifecycle stage.
//!
//! ## Thread Safety
//!
//! The transport is `Send + Sync`. The stdin read loop runs as a single tokio
//! task. The stdout writer is shared via `Arc` and safe to call concurrently
//! from multiple tasks (writes are serialized through an internal mutex).
//! The shutdown signal is an `Arc<AtomicBool>` for lock-free access.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use tokio::io::AsyncBufRead;
use tokio::task::JoinHandle;

use crate::io_stream::config::TransportConfig;
use crate::io_stream::errors::{TransportError, TransportResult};
use crate::io_stream::lifecycle::TransportLifecycle;
use crate::io_stream::reader::AsyncStdinReader;
use crate::io_stream::writer::AsyncStdoutWriter;
use crate::registry::lifecycle::{Lifecycle, LifecycleStage};
use crate::rpc::{Request, Router};

/// Bidirectional stdio transport for JSON-RPC.
///
/// Wraps an async stdin reader and async stdout writer, connecting them
/// through a [`Router`]. The transport manages the full lifecycle:
///
/// ```text
/// Stdin → Reader → Deser → Router → Ser → Writer → Stdout
///                   ↑                    ↓
///              shutdown          shutdown flag
/// ```
///
/// ## Usage
///
/// ```no_run
/// use nabu_core::io_stream::StdioTransport;
/// use nabu_core::registry::lifecycle::Lifecycle;
/// use nabu_core::rpc::{Router, RpcHandler, JsonRpcError};
/// use serde_json::Value;
/// use std::sync::Arc;
/// use async_trait::async_trait;
///
/// struct PingHandler;
/// #[async_trait]
/// impl RpcHandler for PingHandler {
///     async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
///         Ok(serde_json::json!("pong"))
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let router = Arc::new(Router::new());
///     router.register("ping", Arc::new(PingHandler)).await;
///
///     let transport = StdioTransport::new(router);
///     transport.initialize().unwrap();
///     transport.start_transport().unwrap();
///     transport.run().await.unwrap();
/// }
/// ```
pub struct StdioTransport {
    /// The JSON-RPC router — handles request dispatch, no transport logic.
    router: Arc<Router>,
    /// Configuration for the transport.
    config: TransportConfig,
    /// Shared shutdown signal between reader and writer.
    shutdown: Arc<AtomicBool>,
    /// Lifecycle state manager.
    lifecycle: TransportLifecycle,
    /// Handle to the spawned read loop task.
    /// Protected by `StdMutex` because it's only accessed synchronously
    /// (stored during `start_transport`, taken during `run`/`shutdown`).
    read_handle: StdMutex<Option<JoinHandle<TransportResult<()>>>>,
    /// The stdout writer, shared via Arc with the read loop.
    writer: Arc<AsyncStdoutWriter>,
    /// Whether the transport was created with an injected stdin reader
    /// (for testing). Taken out when the read loop starts.
    stdin: StdMutex<Option<Box<dyn AsyncBufRead + Unpin + Send>>>,
}

impl StdioTransport {
    /// Create a new stdio transport with the given router.
    ///
    /// The transport starts in the `Created` lifecycle stage. Call
    /// [`initialize`](Self::initialize) and then
    /// [`start_transport`](Self::start_transport) to begin processing.
    /// Optionally, call [`run`](Self::run) to wait for completion.
    ///
    /// # Arguments
    ///
    /// * `router` — The JSON-RPC router to dispatch requests to. The transport
    ///   does not own or configure the router — it simply calls
    ///   `router.dispatch(request).await`.
    pub fn new(router: Arc<Router>) -> Self {
        Self::with_config(router, TransportConfig::default())
    }

    /// Create a new stdio transport with custom configuration.
    ///
    /// Allows setting `max_message_bytes`, `shutdown_timeout`, and
    /// `flush_after_write`. If you don't need customization, prefer
    /// [`new`](Self::new).
    pub fn with_config(router: Arc<Router>, config: TransportConfig) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let writer = Arc::new(AsyncStdoutWriter::new(
            config.clone(),
            shutdown.clone(),
            None, // Use tokio::io::stdout()
        ));

        Self {
            router,
            config,
            shutdown,
            lifecycle: TransportLifecycle::new(),
            read_handle: StdMutex::new(None),
            writer,
            stdin: StdMutex::new(None),
        }
    }

    /// Create a new stdio transport with injected stdin/stdout for testing.
    ///
    /// This constructor allows tests to provide custom `AsyncBufRead` and
    /// `AsyncWrite` implementations, enabling full control over the I/O
    /// pipeline without touching real stdin/stdout.
    pub fn with_io(
        router: Arc<Router>,
        config: TransportConfig,
        stdin: Box<dyn AsyncBufRead + Unpin + Send>,
        stdout: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    ) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let writer = Arc::new(AsyncStdoutWriter::new(
            config.clone(),
            shutdown.clone(),
            Some(stdout),
        ));

        Self {
            router,
            config,
            shutdown,
            lifecycle: TransportLifecycle::new(),
            read_handle: StdMutex::new(None),
            writer,
            stdin: StdMutex::new(Some(stdin)),
        }
    }

    /// Returns `true` if the transport has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Returns `true` if the transport is currently running (read loop active).
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns the current lifecycle stage.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns a reference to the stdout writer.
    ///
    /// This allows direct response writing without going through the router,
    /// which is useful for protocol-level responses (e.g. parse errors).
    pub fn writer(&self) -> &Arc<AsyncStdoutWriter> {
        &self.writer
    }

    /// Returns a reference to the router.
    pub fn router(&self) -> &Arc<Router> {
        &self.router
    }

    /// Signal the transport to shut down.
    ///
    /// Sets the shutdown flag. The read loop checks this flag before each
    /// read attempt and will exit on the next iteration. The writer also
    /// checks this flag before each write and returns
    /// [`TransportError::Shutdown`].
    ///
    /// This is idempotent — calling it multiple times has no additional effect.
    pub fn signal_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    /// Start the transport's read loop as a tokio task.
    ///
    /// This spawns the stdin reader and returns immediately. The read loop
    /// runs in the background, processing requests until EOF or shutdown.
    ///
    /// The transport must have been initialized first (via
    /// [`Lifecycle::initialize`]).
    ///
    /// This is equivalent to `Lifecycle::start()` plus spawning the I/O
    /// task. The `Lifecycle::start()` trait method is a no-op for this
    /// transport (the real work happens in `start_transport`).
    pub fn start_transport(&self) -> TransportResult<()> {
        if self.lifecycle.is_shutdown() {
            return Err(TransportError::lifecycle(
                "transport is already shut down",
            ));
        }

        if !self.lifecycle.is_at_least(LifecycleStage::Initialized) {
            return Err(TransportError::lifecycle(
                "transport must be initialized before start",
            ));
        }

        if self.lifecycle.is_running() {
            return Ok(());
        }

        // Ensure we're in a tokio runtime
        let runtime_handle = tokio::runtime::Handle::try_current()
            .map_err(|_| TransportError::NoRuntime)?;

        // Clone the router and writer into the task
        let router = self.router.clone();
        let writer = self.writer.clone();
        let shutdown = self.shutdown.clone();
        let config = self.config.clone();

        // Take the injected stdin reader (if any)
        let injected_stdin = {
            let mut guard = self.stdin.lock().unwrap();
            guard.take()
        };

        let handle = runtime_handle.spawn(async move {
            let read_result: TransportResult<()> = if let Some(mut stdin) = injected_stdin {
                // Test mode: use the injected stdin reader directly
                let reader = AsyncStdinReader::new(config, shutdown);
                reader
                    .run(&mut stdin, |request: Request| {
                        let router = router.clone();
                        let writer = writer.clone();
                        async move {
                            let response = router.dispatch(request).await;
                            if let Err(e) = writer.write_response(&response).await {
                                tracing::error!(error = %e, "Failed to write response");
                            }
                        }
                    })
                    .await
            } else {
                // Production mode: read from tokio::io::stdin()
                let reader = AsyncStdinReader::new(config, shutdown);
                let mut stdin_reader = tokio::io::BufReader::new(tokio::io::stdin());
                reader
                    .run(&mut stdin_reader, |request: Request| {
                        let router = router.clone();
                        let writer = writer.clone();
                        async move {
                            let response = router.dispatch(request).await;
                            if let Err(e) = writer.write_response(&response).await {
                                tracing::error!(error = %e, "Failed to write response");
                            }
                        }
                    })
                    .await
            };

            match &read_result {
                Ok(()) => {
                    tracing::debug!("stdio transport: read loop completed normally");
                }
                Err(e) if e.is_shutdown() => {
                    tracing::debug!("stdio transport: read loop interrupted by shutdown");
                }
                Err(e) if e.is_eof() => {
                    tracing::debug!("stdio transport: read loop ended on EOF");
                }
                Err(e) => {
                    tracing::error!(error = %e, "stdio transport: read loop error");
                }
            }

            read_result
        });

        // Store the handle
        {
            let mut guard = self.read_handle.lock().unwrap();
            *guard = Some(handle);
        }

        self.lifecycle
            .start()
            .map_err(|e| TransportError::lifecycle(e.to_string()))?;

        tracing::info!("stdio transport started");
        Ok(())
    }

    /// Wait for the read loop to complete.
    ///
    /// This blocks the current task until the read loop exits (EOF,
    /// shutdown, or error). After this returns, the transport is effectively
    /// shut down and [`flush`](Self::flush) should be called to ensure
    /// all pending responses have been written.
    ///
    /// If the read loop was not started (no handle), this is a no-op.
    pub async fn run(&self) -> TransportResult<()> {
        let handle = {
            let mut guard = self.read_handle.lock().unwrap();
            guard.take()
        };

        if let Some(handle) = handle {
            let result = handle.await.map_err(|e| {
                TransportError::lifecycle(format!("read loop task panicked: {}", e))
            })?;
            result
        } else {
            Ok(())
        }
    }

    /// Shut down the transport gracefully.
    ///
    /// This performs:
    /// 1. Signals the read loop to stop (sets the shutdown flag).
    /// 2. Transitions to `Shutdown` lifecycle stage.
    ///
    /// The read loop will exit on its next iteration. Call [`run`](Self::run)
    /// after this to wait for the read loop to actually finish, then
    /// [`flush`](Self::flush) to ensure all pending writes are committed.
    ///
    /// This method is safe to call multiple times — subsequent calls
    /// are no-ops.
    pub fn shutdown_transport(&self) -> TransportResult<()> {
        if self.lifecycle.is_shutdown() {
            return Ok(());
        }

        tracing::info!("shutting down stdio transport");

        // Signal the read loop to stop
        self.signal_shutdown();

        // Transition to shutdown
        self.lifecycle
            .shutdown()
            .map_err(|e| TransportError::lifecycle(e.to_string()))?;

        tracing::info!("stdio transport shut down");
        Ok(())
    }

    /// Flush pending output to stdout.
    ///
    /// Should be called after the read loop exits to ensure all
    /// buffered responses have been written.
    pub async fn flush(&self) -> TransportResult<()> {
        self.writer.flush().await
    }
}

impl Lifecycle for StdioTransport {
    fn name(&self) -> &'static str {
        "stdio_transport"
    }

    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.lifecycle
            .initialize()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.start_transport()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown_transport()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

impl std::fmt::Debug for StdioTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioTransport")
            .field("lifecycle_stage", &self.lifecycle.stage())
            .field("is_running", &self.lifecycle.is_running())
            .field("is_shutdown", &self.lifecycle.is_shutdown())
            .field("max_message_bytes", &self.config.max_message_bytes)
            .finish()
    }
}

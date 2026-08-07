//! # IPC Socket Lifecycle & Security Hardening
//!
//! This module provides production-ready lifecycle management for UNIX domain
//! sockets used by the Nabu Capability Platform. It enforces:
//!
//! - **Secure permissions** (`0600`) on socket files — owner read/write only.
//! - **Stale socket cleanup** — detects and removes leftover socket files from
//!   crashed or improperly shut-down runs before binding.
//! - **Graceful shutdown** — stops accepting new connections, closes active
//!   handles, and removes the socket file deterministically.
//! - **Lifecycle integration** — implements the [`Lifecycle`] trait so the
//!   socket participates in the standard `Created → Initialized → Running →
//!   Shutdown` state machine alongside every other service.
//! - **Thread safety** — all operations are safe under concurrent shutdown and
//!   restart scenarios (no double-close, no duplicate cleanup, no dangling
//!   handles).
//!
//! ## Lifecycle Diagram
//!
//! ```text
//! Application Startup
//!   │
//!   ▼
//! SocketManager::new()          ── LifecycleStage::Created
//!   │
//!   ├── initialize()            ── validates path, cleans stale socket, applies 0600
//!   │                              LifecycleStage::Initialized
//!   │
//!   ├── start()                 ── binds listener, spawns accept loop
//!   │                              LifecycleStage::Running
//!   │
//!   ▼
//! Normal Operation              ── accepts connections, dispatches to handler
//!   │
//!   ▼
//! shutdown()                    ── signals accept loop to stop, closes listener,
//!   │                              removes socket file
//!   │                              LifecycleStage::Shutdown
//!   ▼
//! Socket Closed                 ── OS resources released, no orphaned files
//!   ▼
//! Socket Removed                ── filesystem clean
//! ```
//!
//! ## Permission Model
//!
//! Socket files are created with `0600` permissions (owner read/write only).
//! This is the tightest meaningful permission set for a local IPC socket —
//! it prevents other users from connecting to or impersonating the socket.
//!
//! On Unix-like systems, permissions are set via `fcntl(F_SETACL)` or
//! `chmod` after binding. On macOS, `unlink` + rebind is used if the
//! filesystem does not support changing permissions on a bound socket.
//!
//! ## Restart Handling
//!
//! On startup, [`SocketManager::cleanup_stale_socket`] checks whether a
//! socket file already exists at the target path. If it does, the socket
//! is removed so that `UnixListener::bind` can succeed without manual
//! intervention. This makes restarts safe — the application never fails to
//! start because a stale socket file from a previous run is blocking the
//! bind.
//!
//! ## Future Compatibility
//!
//! `SocketManager` is designed to be transport-agnostic. While it currently
//! only manages UNIX domain sockets, the `Lifecycle` trait and the
//! `SocketManagerConfig` struct make it straightforward to add support for
//! additional local transports (e.g. named pipes on Windows) in the future.
//!
//! Future IPC consumers — MCP servers, ACP servers, plugin hosts, sync
//! services — can simply obtain a `SocketManager` from the
//! [`ApplicationContext`](crate::registry::context::ApplicationContext)
//! and call `start()` / `shutdown()` through the existing lifecycle
//! infrastructure. No new architecture is required.

pub mod errors;
pub mod manager;
pub mod lifecycle;

pub use errors::{SocketError, SocketResult};
pub use lifecycle::SocketLifecycle;
pub use manager::{SocketConfig, SocketManager, SocketManagerHandle};

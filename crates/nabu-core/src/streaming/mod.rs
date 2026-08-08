//! # Streaming Pipeline — Real-Time Token Streaming via EventBus
//!
//! This module provides the production-ready, protocol-agnostic streaming
//! infrastructure for the Nabu Capability Platform. It enables external agent
//! processes to stream incremental output (tokens) to the frontend through the
//! existing [`EventBus`](crate::event_bus::EventBus).
//!
//! ## Architecture
//!
//! ```text
//! Agent Process
//!     │  (JSON-RPC over stdin/stdout)
//!     ▼
//! Streaming Session
//!     │  (token events)
//!     ▼
//! EventBus  ← canonical transport for all streaming events
//!     │
//!     ▼
//! Frontend Listener
//!     │
//!     ▼
//! Incremental UI Updates
//! ```
//!
//! The EventBus is the **single** transport for streaming events — no bypass,
//! no direct callbacks, no duplicate mechanisms.
//!
//! ## Key Types
//!
//! | Type | Role |
//! |------|------|
//! | [`StreamId`] | Unique identifier for a streaming session. |
//! | [`StreamState`] | Lifecycle state of a stream (Active, Streaming, Completed, Cancelled, Failed). |
//! | [`StreamingSession`] | Runtime record for a single stream — tracks state, partial content, token count. |
//! | [`StreamingPipeline`] | The core publish/complete/cancel API — the entry point for token events. |
//! | [`StreamSessionHandle`] | Handle returned to callers for cancellation and queries. |
//! | [`StreamManager`] | Thread-safe registry of all active streaming sessions. |
//! | [`StreamManagerError`] | Structured error type for all streaming operations. |
//!
//! ## Lifecycle
//!
//! ```text
//! Start → Streaming → Complete
//!                 ↘ Cancel
//!                 ↘ Fail
//! ```
//!
//! 1. **`start_stream`** — Creates a `StreamingSession`, publishes a
//!    `StreamEvent::Started` + `StreamSessionEvent::SessionCreated` through
//!    the EventBus.
//! 2. **`publish_token`** — Publishes a `StreamEvent::Token` through the
//!    EventBus for each incremental token. Ordering is guaranteed: tokens
//!    are published strictly in sequence order.
//! 3. **`complete_stream`** — Publishes a `StreamEvent::Completed` terminal
//!    event and marks the session as `Completed`.
//! 4. **`cancel_stream`** — Publishes a `StreamEvent::Cancelled` terminal
//!    event and marks the session as `Cancelled`.
//! 5. **`fail_stream`** — Publishes a `StreamEvent::Failed` terminal event
//!    and marks the session as `Failed`.
//!
//! ## Thread Safety
//!
//! - `StreamManager` is `Send + Sync`, designed to be shared as
//!   `Arc<StreamManager>` across threads.
//! - Each `StreamingSession` is protected by a `Mutex`.
//! - Token publication is atomic: the session is locked, the token is
//!   appended to partial content, the event is published, and the lock is
//!   released — all in one operation to prevent interleaved ordering.
//! - Multiple simultaneous streams are fully isolated — each has its own
//!   `StreamingSession` with independent state and sequence numbering.
//!
//! ## Future Compatibility
//!
//! The streaming pipeline is designed to support future:
//! - ACP streaming
//! - MCP streaming
//! - AI provider integrations
//! - Plugin streaming
//! - Multimodal output
//! - Progress updates
//! - Tool execution streaming
//!
//! Future protocol implementations will call `pipeline.start_stream()` and
//! `pipeline.publish_token()` rather than managing EventBus event construction
//! directly.

#![cfg(not(target_arch = "wasm32"))]

pub mod errors;
pub mod manager;
pub mod pipeline;
pub mod session;

pub use errors::{StreamManagerError, StreamResult};
pub use manager::StreamManager;
pub use pipeline::StreamingPipeline;
pub use session::{StreamSessionHandle, StreamState, StreamingSession};
pub use crate::event_bus::StreamId;
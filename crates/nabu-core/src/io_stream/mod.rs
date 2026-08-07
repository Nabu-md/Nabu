//! # Stdio JSON-RPC Transport — Bidirectional stdin/stdout I/O
//!
//! This module provides a production-ready, protocol-agnostic transport
//! for bidirectional JSON-RPC communication over stdin and stdout.
//!
//! ## Architecture
//!
//! The transport implements the standard stdin → router → stdout pipe:
//!
//! ```text
//! stdin
//!     ↓
//! Async Reader (AsyncStdinReader)
//!     ↓
//! Deserialize (newline-delimited JSON → Request)
//!     ↓
//! JSON-RPC Router (Router::dispatch)
//!     ↓
//! Serialize (Response → newline-delimited JSON)
//!     ↓
//! Async Writer (AsyncStdoutWriter)
//!     ↓
//! stdout
//! ```
//!
//! ## Components
//!
//! | Component | Module | Role |
//! |-----------|--------|------|
//! | [`AsyncStdinReader`] | [`reader`] | Reads newline-delimited JSON from stdin, deserializes to `Request`. |
//! | [`AsyncStdoutWriter`] | [`writer`] | Serializes `Response` to newline-delimited JSON, writes to stdout. |
//! | [`StdioTransport`] | [`transport`] | Orchestrates reader + writer + lifecycle. |
//! | [`TransportConfig`] | [`config`] | Tunable parameters (buffer sizes, timeouts). |
//! | [`TransportError`] | [`errors`] | Structured error types for all failure modes. |
//! | [`TransportLifecycle`] | [`lifecycle`] | Lifecycle integration with the Nabu registry. |
//!
//! ## Protocol Agnosticism
//!
//! The transport knows nothing about ACP, MCP, or any specific JSON-RPC
//! method semantics. It simply reads a line, deserializes it as a
//! [`Request`], dispatches through the injected [`Router`], serializes the
//! [`Response`], and writes the line back out. Future protocol servers can
//! plug in their own router and use this transport as-is.
//!
//! ## Usage
//!
//! ```no_run
//! use nabu_core::io_stream::StdioTransport;
//! use nabu_core::registry::lifecycle::Lifecycle;
//! use nabu_core::rpc::{Router, RpcHandler, JsonRpcError};
//! use serde_json::Value;
//! use std::sync::Arc;
//! use async_trait::async_trait;
//!
//! struct PingHandler;
//! #[async_trait]
//! impl RpcHandler for PingHandler {
//!     async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
//!         Ok(serde_json::json!("pong"))
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let router = Arc::new(Router::new());
//!     router.register("ping", Arc::new(PingHandler)).await;
//!
//!     let transport = StdioTransport::new(router);
//!     transport.initialize().unwrap();
//!     transport.start_transport().unwrap();
//!     transport.run().await.unwrap();
//! }
//! ```
//!
//! ## Framing Strategy
//!
//! Messages are framed as newline-delimited JSON (NDJSON / JSON Lines).
//! Each JSON-RPC request and response is serialized as a single line of
//! JSON, terminated by a `\n` character. This is the same framing used by
//! the Language Server Protocol (LSP) and is the standard for stdio-based
//! protocol servers.
//!
//! ## Lifecycle
//!
//! The transport implements the [`Lifecycle`] trait:
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//! ```
//!
//! - `initialize()` — validates configuration, prepares I/O handles.
//! - `start()` / `start_transport()` — spawns the async read loop.
//! - `run()` — blocks until the read loop exits (EOF or shutdown).
//! - `shutdown()` / `shutdown_transport()` — signals the read loop to stop.
//! - `flush()` — ensures all pending responses are written to stdout.

#![cfg(not(target_arch = "wasm32"))]

pub mod config;
pub mod errors;
pub mod framing;
pub mod lifecycle;
pub mod reader;
pub mod transport;
pub mod writer;

pub use config::TransportConfig;
pub use errors::{TransportError, TransportResult};
pub use framing::{decode_message, decode_message_bytes, encode_message};
pub use lifecycle::TransportLifecycle;
pub use reader::AsyncStdinReader;
pub use transport::StdioTransport;
pub use writer::AsyncStdoutWriter;

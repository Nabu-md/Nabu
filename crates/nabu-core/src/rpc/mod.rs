//! # JSON-RPC Core — Transport-Independent Protocol Foundation
//!
//! This module provides the reusable, transport-independent JSON-RPC 2.0
//! infrastructure for the Nabu Capability Platform. It implements the
//! protocol-level abstractions that future higher-layer protocols (ACP, MCP,
//! plugin protocols, external service bridges) build upon — without each
//! integration re-implementing request/response/error/routing machinery.
//!
//! ## Architecture
//!
//! ```text
//! Transport (stdin/stdout, sockets, IPC, HTTP, …)
//!     │
//!     ▼
//! JSON-RPC Core   ← this module
//!     │
//!     ▼
//! Protocol Implementations (ACP, MCP, plugins, …)
//! ```
//!
//! The core operates purely on protocol messages: it knows nothing about
//! Unix sockets, TCP, WebSockets, Tauri IPC, stdin/stdout, or child processes.
//! Any transport that can serialize a [`Request`] and deserialize a
//! [`Response`] can use this router.
//!
//! ## Key Types
//!
//! | Type | Role |
//! |------|------|
//! | [`RequestId`] | Strongly-typed JSON-RPC request identifier (string, numeric, or null). |
//! | [`Request`] | A JSON-RPC 2.0 request (version, id, method, params). |
//! | [`Response`] | A JSON-RPC 2.0 response — success or error, never both. |
//! | [`JsonRpcError`] | Structured protocol-level error with standard codes. |
//! | [`RpcHandler`] | Async handler trait — receives params, returns a result. |
//! | [`Router`] | Method registration and dispatch. |
//!
//! ## Transport Independence
//!
//! The router dispatches on [`Request`] values and returns [`Response`] values.
//! It does not read from or write to any transport. A transport layer (added
//! in a later phase) reads bytes, serializes them into a [`Request`], calls
//! [`Router::dispatch`], serializes the returned [`Response`], and writes
//! bytes back out.
//!
//! ## Usage
//!
//! ```
//! use nabu_core::rpc::{Request, Response, Router, RpcHandler, JsonRpcError};
//! use serde_json::json;
//! use std::sync::Arc;
//! use async_trait::async_trait;
//!
//! struct EchoHandler;
//! #[async_trait]
//! impl RpcHandler for EchoHandler {
//!     async fn handle(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, JsonRpcError> {
//!         Ok(params.unwrap_or(json!(null)))
//!     }
//! }
//!
//! let mut router = Router::new();
//! router.register("echo", Arc::new(EchoHandler));
//!
//! let req = Request::new(1, "echo", Some(json!({ "msg": "hi" })));
//! let resp = router.dispatch(req).await;
//! assert!(resp.is_success());
//! ```

pub mod error;
pub mod router;
pub mod types;

pub use error::{ErrorCode, JsonRpcError};
pub use router::{Router, RpcHandler};
pub use types::{Request, RequestId, Response};

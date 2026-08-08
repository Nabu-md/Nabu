//! # Tool Calling Framework
//!
//! A discoverable, executable framework for tools that can be invoked through
//! the Nabu Capability Platform. Tools are registered with a [`ToolRegistry`],
//! which dispatches [`ToolCall`]s to the appropriate [`Tool`] implementation
//! and returns structured [`ToolResult`]s.
//!
//! ## Architecture
//!
//! ```text
//! Caller
//!   │  (serializes ToolCall as JSON)
//!   ▼
//! ToolRegistry
//!   │
//!   ├── validate_against(ToolSpec)
//!   ├── lookup tool by ToolId
//!   ├── Tool::call(ToolCall)
//!   ▼
//! ToolResult
//!   │  (serialized by IPC layer)
//!   ▼
//! Caller
//! ```
//!
//! ## Key Types
//!
//! | Type | Role |
//! |------|------|
//! | [`ToolId`] | Strongly-typed tool identifier. |
//! | [`ToolSpec`] | Declarative description (name, params, schema). |
//! | [`ToolCall`] | An invocation request (tool ID + arguments). |
//! | [`ToolResult`] | The execution result (success, error, etc.). |
//! | [`Tool`] | Async trait for executable tools. |
//! | [`ToolRegistry`] | Registration, discovery, validation, dispatch. |
//! | [`ToolError`] | Structured error type for tool execution. |
//!
//! ## Thread Safety
//!
//! The [`ToolRegistry`] uses a `tokio::sync::RwLock<HashMap<...>>` for tool
//! storage. This makes `register` (write) and `call` (read) safe to call
//! concurrently from multiple threads. The [`Tool`] trait requires
//! `Send + Sync`, so tools can be shared across threads.
//!
//! ## Design Notes
//!
//! - The `Tool` trait is **not** a protocol implementation. It does not
//!   implement ACP, MCP, or any specific agent protocol. It is the platform
//!   abstraction for discoverable, executable functionality.
//! - `ToolCall` arguments are raw `serde_json::Value` — tools are responsible
//!   for deserializing their own arguments.
//! - All public types derive `Serialize` + `Deserialize` for IPC/wire transport.
//!
//! ## Usage
//!
//! ```no_run
//! use nabu_core::tool_calling::{ToolRegistry, Tool, ToolCall, ToolSpec, ToolParam, ToolParamSchema};
//! use std::sync::Arc;
//!
//! let registry = ToolRegistry::new();
//! // Register a tool implementing the Tool trait
//! // registry.register(Arc::new(MyTool)).await;
//!
//! // Discover available tools
//! let specs = registry.specs().await;
//!
//! // Call a tool
//! // let result = registry.call(ToolCall::without_args("nabu:ping")).await;
//! ```

pub mod models;
pub mod registry;
pub mod tool;

pub use models::{
    ToolCall, ToolError, ToolExecutionMeta, ToolId, ToolParam, ToolParamSchema, ToolResult,
    ToolResultStatus, ToolSpec,
};
pub use registry::ToolRegistry;
pub use tool::{shared, SharedTool, Tool};

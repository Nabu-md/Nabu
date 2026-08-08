//! The `Tool` trait definition.
//!
//! A `Tool` is a unit of executable functionality discoverable through the
//! [`ToolRegistry`](super::registry::ToolRegistry). Implementations receive a
//! [`ToolCall`](crate::tool_calling::ToolCall) and return a
//! [`ToolResult`](crate::tool_calling::ToolResult).
//!
//! ## Design
//!
//! - The trait is **async** (via `async_trait`) to be compatible with the
//!   project's tokio-based async runtime.
//! - The trait is **not** `Send + Sync` itself, but the `SharedTool` type alias
//!   is `Arc<dyn Tool + Send + Sync>` — concrete implementations must be
//!   `Send + Sync` so tools can be shared across threads and invoked from
//!   concurrent IPC handlers.
//! - `ToolCall` arguments are passed as a raw `serde_json::Value` so that the
//!   framework does not impose a specific deserialization strategy on tool
//!   implementations. Tools are responsible for deserializing their own arguments.
//! - Errors are returned as structured [`ToolError`](crate::tool_calling::ToolError)
//!   values — the framework never panics.

use super::models::{ToolCall, ToolError, ToolId, ToolResult, ToolSpec};
use async_trait::async_trait;
use std::sync::Arc;

/// Async trait for tools that can be invoked through the Tool Calling Framework.
///
/// Implementations receive a [`ToolCall`] (which contains the tool ID and
/// arguments) and the [`ToolSpec`] describing the tool's interface. The spec
/// is provided so tools can perform argument validation against their declared
/// schema if they choose, but they are not required to — the
/// [`ToolRegistry`](super::registry::ToolRegistry) validates required
/// parameters before dispatching.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` so they can be stored as
/// [`SharedTool`] (`Arc<dyn Tool + Send + Sync>`) and invoked from
/// concurrent async contexts.
///
/// # Error Handling
///
/// Tools must return `Err(ToolError)` for structured failures or `Ok(ToolResult)`
/// for completion (including error results that carry a `ToolResultStatus::Error`).
/// Panics inside tool execution are caught by the framework and never
/// propagated to the caller.
///
/// # Example
///
/// ```no_run
/// use async_trait::async_trait;
/// use nabu_core::tool_calling::{Tool, ToolCall, ToolResult, ToolSpec, ToolError, ToolParam, ToolParamSchema};
/// use serde_json::Value;
/// use std::sync::Arc;
///
/// struct EchoTool;
///
/// #[async_trait]
/// impl Tool for EchoTool {
///     fn spec(&self) -> ToolSpec {
///         ToolSpec::new("echo", "Echo", "Echoes back the input")
///             .with_param(ToolParam::required("msg", ToolParamSchema::of_type("string")))
///     }
///
///     async fn call(&self, call: ToolCall) -> Result<ToolResult, ToolError> {
///         let msg = call.arguments
///             .as_ref()
///             .and_then(|v| v.get("msg"))
///             .and_then(|v| v.as_str())
///             .unwrap_or("");
///         Ok(ToolResult::success(
///             Some(Value::String(msg.to_string())),
///             None,
///         ))
///     }
/// }
///
/// let tool: Arc<dyn Tool + Send + Sync> = Arc::new(EchoTool);
/// ```
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    /// Returns the declarative specification of this tool.
    ///
    /// The spec includes the tool's ID, name, description, and parameter
    /// declarations. It is used by the
    /// [`ToolRegistry`](super::registry::ToolRegistry) for discovery and
    /// validation.
    fn spec(&self) -> ToolSpec;

    /// Execute the tool with the given call.
    ///
    /// The `call` contains the tool ID and the raw JSON arguments. The spec
    /// (available via [`spec`](Self::spec)) describes the expected argument
    /// shape. The tool is responsible for deserializing its own arguments
    /// from the `serde_json::Value`.
    ///
    /// # Return Values
    ///
    /// - `Ok(ToolResult)` — the tool completed (success or a tool-level error
    ///   result with `ToolResultStatus::Error` or `Cancelled`).
    /// - `Err(ToolError)` — a framework-level error that prevented the tool
    ///   from executing (e.g. argument deserialization failure). The
    ///   framework converts this into a `ToolResult::error` response.
    ///
    /// # Panics
    ///
    /// Implementations must not panic. The framework does not catch panics
    /// from within the `call` method — it is the tool's responsibility to
    /// handle internal errors gracefully and return a `ToolError` instead.
    async fn call(&self, call: ToolCall) -> Result<ToolResult, ToolError>;

    /// Returns the tool's ID.
    ///
    /// This is a convenience method equivalent to
    /// `self.spec().id`. It is provided as a trait method so that tools
    /// can be looked up by ID without constructing a full `ToolSpec`.
    fn id(&self) -> ToolId {
        self.spec().id.clone()
    }

    /// Returns the tool's name.
    ///
    /// Convenience method equivalent to `self.spec().name`.
    fn name(&self) -> String {
        self.spec().name.clone()
    }

    /// Returns the tool's description.
    ///
    /// Convenience method equivalent to `self.spec().description`.
    fn description(&self) -> String {
        self.spec().description.clone()
    }
}

/// A type alias for shared, thread-safe, async-exposed tools.
///
/// Tools are stored as `Arc<dyn Tool + Send + Sync>` so they can
/// be freely cloned and shared across threads without ownership concerns.
pub type SharedTool = Arc<dyn Tool + Send + Sync>;

/// Convert a `Tool` implementation into a `SharedTool`.
///
/// This is a convenience function for creating an `Arc<dyn Tool + Send + Sync>`
/// from a concrete `Tool` implementation.
pub fn shared<T: Tool>(tool: Arc<T>) -> SharedTool {
    tool as SharedTool
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_calling::models::ToolParamSchema;
    use serde_json::json;

    struct PingTool;

    #[async_trait]
    impl Tool for PingTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:ping", "Ping", "Returns pong")
        }

        async fn call(&self, _call: ToolCall) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::success(Some(json!("pong")), None))
        }
    }

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:echo", "Echo", "Echoes input")
                .with_param(ToolParam::required("msg", ToolParamSchema::of_type("string")))
        }

        async fn call(&self, call: ToolCall) -> Result<ToolResult, ToolError> {
            let msg = call.arguments
                .as_ref()
                .and_then(|v| v.get("msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(ToolResult::success(Some(json!(msg.to_string())), None))
        }
    }

    #[test]
    fn tool_id_returns_spec_id() {
        let tool = PingTool;
        assert_eq!(tool.id(), ToolId::new("nabu:ping"));
    }

    #[test]
    fn tool_name_returns_spec_name() {
        let tool = PingTool;
        assert_eq!(tool.name(), "Ping");
    }

    #[test]
    fn tool_description_returns_spec_description() {
        let tool = PingTool;
        assert_eq!(tool.description(), "Returns pong");
    }

    #[tokio::test]
    async fn tool_call_returns_success() {
        let tool = PingTool;
        let call = ToolCall::without_args("nabu:ping");
        let result = tool.call(call).await.unwrap();
        assert!(result.is_success());
        assert_eq!(result.result, Some(json!("pong")));
    }

    #[tokio::test]
    async fn echo_tool_returns_argument() {
        let tool = EchoTool;
        let call = ToolCall::with_args("nabu:echo", json!({ "msg": "hello" }));
        let result = tool.call(call).await.unwrap();
        assert_eq!(result.result, Some(json!("hello")));
    }

    #[tokio::test]
    async fn echo_tool_handles_missing_arg_gracefully() {
        let tool = EchoTool;
        let call = ToolCall::without_args("nabu:echo");
        let result = tool.call(call).await.unwrap();
        assert_eq!(result.result, Some(json!("")));
    }

    #[test]
    fn shared_wraps_arc() {
        let tool: SharedTool = shared(Arc::new(PingTool));
        assert_eq!(tool.id(), ToolId::new("nabu:ping"));
    }

    #[tokio::test]
    async fn shared_tool_can_call() {
        let tool: SharedTool = shared(Arc::new(PingTool));
        let call = ToolCall::without_args("nabu:ping");
        let result = tool.call(call).await.unwrap();
        assert_eq!(result.result, Some(json!("pong")));
    }
}

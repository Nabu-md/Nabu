//! Tool Registry — registration, discovery, validation, and execution.
//!
//! The [`ToolRegistry`] is the central dispatch point for tool invocations.
//! It maps tool IDs to registered `Tool` implementations, validates incoming
//! calls against the tool's spec, and produces [`ToolResult`] values — including
//! structured errors for unknown tools or invalid parameters.
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
//! ## Thread Safety
//!
//! The [`ToolRegistry`] uses an internal `tokio::sync::RwLock<HashMap<...>>`
//! for tool storage, mirroring the [`Router`](crate::rpc::Router) pattern.
//! This makes `register` (write) and `dispatch` (read) safe to call
//! concurrently from multiple threads. The `Tool` trait requires `Send + Sync`,
//! so tools can be shared across threads.
//!
//! ## Design
//!
//! This registry mirrors the design of [`Router`](crate::rpc::Router) —
//! using `tokio::sync::RwLock` and async trait handlers. Registration
//! is async (acquires write lock), dispatch is async (acquires read lock
//! then awaits the tool's `call` method).
//!
//! Unknown tool calls return `ToolResultStatus::ToolNotFound`. Invalid
//! parameters return `ToolResultStatus::InvalidParams`. Tool panics are
//! **not** caught here — they propagate to the caller. (See
//! [`ToolRegistry::call`] for notes on panic handling.)

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::models::{ToolCall, ToolId, ToolResult, ToolSpec};
#[allow(unused_imports)]
use super::tool::{SharedTool, Tool};

/// A type alias for shared, thread-safe tools.
///
/// Tools are stored as `Arc<dyn Tool + Send + Sync>` so they can
/// be freely cloned and shared across threads without ownership concerns.
pub type SharedToolAlias = SharedTool;

/// The Tool Registry — central registration and dispatch for tools.
///
/// The registry stores tools keyed by their `ToolId`. It supports:
///
/// - **Registration** — `register` adds a tool. If a tool with the same ID
///   is already registered, it is replaced (non-panicking overwrite).
/// - **Discovery** — `list`, `has_tool`, `specs`, and `spec` allow
///   enumeration and lookup.
/// - **Validation** — `call` validates arguments against the tool's spec
///   before dispatching.
/// - **Execution** — `call` dispatches to the registered tool's
///   `call` method and produces a `ToolResult`.
///
/// # Concurrency
///
/// The internal tool map is protected by a `tokio::sync::RwLock`. Multiple
/// `call` / `list` / `has_tool` / `spec` / `specs` calls can execute
/// simultaneously (read lock). `register` / `unregister` acquire a write lock.
/// Tool execution itself is not serialized — the registry awaits each tool
/// independently, allowing concurrent execution.
///
/// # Duplicate Registration
///
/// Registering a tool with an ID that already exists replaces the previous
/// tool. This is an explicit, non-panicking overwrite — callers that need to
/// detect conflicts should check [`has_tool`](Self::has_tool) before
/// registering.
///
/// # Example
///
/// ```ignore
/// use nabu_core::tool_calling::{ToolRegistry, ToolSpec};
/// use std::sync::Arc;
///
/// let registry = ToolRegistry::new();
/// registry.register(Arc::new(MyTool)).await;
///
/// let result = registry.call(ToolCall::without_args("my:tool")).await;
/// assert!(result.is_success());
/// ```
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, SharedTool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &"<async tool map>")
            .finish()
    }
}

impl ToolRegistry {
    /// Create a new, empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool.
    ///
    /// If a tool with the same ID is already registered, it is replaced.
    /// This is an explicit overwrite — the old tool is dropped and the new
    /// one takes its place. No panic or error is raised.
    ///
    /// # Thread Safety
    ///
    /// This method acquires a write lock on the internal tool map. It is
    /// safe to call concurrently with `call`, `list`, `has_tool`, `spec`,
    /// `specs`, and other `register` calls.
    pub async fn register(&self, tool: SharedTool) {
        let id = tool.id();
        tracing::debug!(tool_id = %id, "Registering tool");
        self.tools.write().await.insert(id.0.clone(), tool);
    }

    /// Returns `true` if a tool is registered for the given ID.
    pub async fn has_tool(&self, id: &str) -> bool {
        self.tools.read().await.contains_key(id)
    }

    /// Returns the number of registered tools.
    pub async fn tool_count(&self) -> usize {
        self.tools.read().await.len()
    }

    /// List all registered tool IDs (sorted for deterministic output).
    pub async fn list(&self) -> Vec<ToolId> {
        let tools = self.tools.read().await;
        let mut ids: Vec<String> = tools.keys().cloned().collect();
        ids.sort();
        ids.into_iter().map(ToolId::from).collect()
    }

    /// List all registered tool specs (sorted by ID for deterministic output).
    pub async fn specs(&self) -> Vec<ToolSpec> {
        let tools = self.tools.read().await;
        let mut specs: Vec<ToolSpec> = tools.values().map(|t| t.spec()).collect();
        specs.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        specs
    }

    /// Returns the spec for a specific tool, if registered.
    pub async fn spec(&self, id: &str) -> Option<ToolSpec> {
        let tools = self.tools.read().await;
        tools.get(id).map(|t| t.spec())
    }

    /// Returns the spec for a tool by `ToolId`, if registered.
    pub async fn spec_by_id(&self, id: &ToolId) -> Option<ToolSpec> {
        self.spec(&id.0).await
    }

    /// Unregister a tool by ID.
    ///
    /// Returns `true` if a tool was removed.
    /// Returns `false` if no tool was registered under the given ID.
    pub async fn unregister(&self, id: &str) -> bool {
        let removed = self.tools.write().await.remove(id).is_some();
        if removed {
            tracing::debug!(tool_id = %id, "Tool unregistered");
        }
        removed
    }

    /// Unregister all tools whose ID starts with the given prefix.
    ///
    /// Returns the number of tools removed.
    /// Useful for cleaning up a namespace (e.g. all `"nabu:"` tools).
    pub async fn unregister_prefix(&self, prefix: &str) -> usize {
        let mut tools = self.tools.write().await;
        let to_remove: Vec<String> = tools
            .keys()
            .filter(|id| id.starts_with(prefix))
            .cloned()
            .collect();
        let count = to_remove.len();
        for id in &to_remove {
            tools.remove(id);
            tracing::debug!(tool_id = %id, "Tool unregistered (prefix removal)");
        }
        count
    }

    // -----------------------------------------------------------------------
    // Execution
    // -----------------------------------------------------------------------

    /// Dispatch a tool call to the registered tool and produce a result.
    ///
    /// The dispatch lifecycle is:
    ///
    /// 1. **Look up** the tool by `ToolCall::tool_id`.
    ///    - If not found → `ToolResult::tool_not_found` response.
    /// 2. **Validate** the arguments against the tool's `ToolSpec`.
    ///    - If required parameters are missing → `ToolResult::invalid_params`.
    /// 3. **Execute** the tool via `Tool::call`.
    ///    - `Ok(ToolResult)` → the result is returned as-is.
    ///    - `Err(ToolError)` → wrapped in a `ToolResult::error` response.
    ///
    /// # Panics
    ///
    /// This method does **not** catch panics from within the tool's `call`
    /// method. If a tool panics, the panic propagates to the caller. This is
    /// by design — tools are expected to handle their own errors gracefully
    /// and return `Err(ToolError)`. Callers that need panic isolation should
    /// use `tokio::task::spawn` / `tokio::task::spawn_blocking` to execute
    /// the `call` in a separate task.
    ///
    /// # Arguments
    ///
    /// - `call` — the tool call to dispatch.
    ///
    /// # Returns
    ///
    /// A [`ToolResult`] that is always `Ok` from this method's perspective —
    /// tool-level failures and errors are encoded in the `ToolResult`'s
    /// `status` and `error` fields.
    pub async fn call(&self, call: ToolCall) -> ToolResult {
        let tool_id = call.tool_id.clone();
        let tool = {
            let tools = self.tools.read().await;
            tools.get(&tool_id.0).cloned()
        };

        let tool = match tool {
            Some(t) => t,
            None => {
                tracing::debug!(tool_id = %tool_id, "Tool not found");
                return ToolResult::tool_not_found(tool_id);
            }
        };

        // Validate arguments against the tool's spec.
        let spec = tool.spec();
        if let Err(missing) = call.validate_against(&spec) {
            tracing::warn!(
                tool_id = %tool_id,
                missing = ?missing,
                "Tool call has missing required parameters"
            );
            return ToolResult::invalid_params(tool_id, missing);
        }

        tracing::debug!(tool_id = %tool_id, "Dispatching tool call");

        // Execute the tool.
        let start = std::time::Instant::now();
        let result = tool.call(call).await;
        let duration = start.elapsed();

        match result {
            Ok(tool_result) => {
                tracing::debug!(
                    tool_id = %tool_id,
                    duration_ms = %duration.as_millis(),
                    "Tool executed successfully"
                );
                tool_result
            }
            Err(err) => {
                tracing::warn!(
                    tool_id = %tool_id,
                    error_code = %err.code,
                    duration_ms = %duration.as_millis(),
                    "Tool returned error"
                );
                ToolResult::error(
                    err,
                    Some(super::models::ToolExecutionMeta::from_duration(tool_id, duration)),
                )
            }
        }
    }

    /// Calls a tool by ID with optional arguments, returning the result.
    ///
    /// This is a convenience method that constructs a `ToolCall` and calls
    /// [`call`](Self::call). If the tool ID is not found, returns a
    /// `ToolResult::tool_not_found`.
    ///
    /// # Arguments
    ///
    /// - `id` — the tool ID to invoke.
    /// - `arguments` — optional JSON arguments for the tool.
    pub async fn call_with_args(
        &self,
        id: impl Into<ToolId>,
        arguments: Option<Value>,
    ) -> ToolResult {
        let id = id.into();
        self.call(ToolCall::new(id, arguments)).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_calling::models::{ToolError, ToolParam, ToolParamSchema, ToolSpec};
    use async_trait::async_trait;
    use serde_json::json;

    struct PingTool;
    struct EchoTool;
    struct FailTool;
    struct AddTool;

    #[async_trait]
    impl Tool for PingTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:ping", "Ping", "Returns pong")
        }
        async fn call(&self, _call: ToolCall) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::success(Some(json!("pong")), None))
        }
    }

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

    #[async_trait]
    impl Tool for FailTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:fail", "Fail", "Always fails")
        }
        async fn call(&self, _call: ToolCall) -> Result<ToolResult, ToolError> {
            Err(ToolError::new("TOOL_FAILED", "intentional failure"))
        }
    }

    #[async_trait]
    impl Tool for AddTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:add", "Add", "Adds two numbers")
                .with_param(ToolParam::required("a", ToolParamSchema::of_type("number")))
                .with_param(ToolParam::required("b", ToolParamSchema::of_type("number")))
        }
        async fn call(&self, call: ToolCall) -> Result<ToolResult, ToolError> {
            let a = call.arguments.as_ref().and_then(|v| v.get("a")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let b = call.arguments.as_ref().and_then(|v| v.get("b")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(ToolResult::success(Some(json!(a + b)), None))
        }
    }

    #[tokio::test]
    async fn new_registry_is_empty() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.tool_count().await, 0);
        assert!(registry.list().await.is_empty());
    }

    #[tokio::test]
    async fn register_and_lookup() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;
        assert_eq!(registry.tool_count().await, 1);
        assert!(registry.has_tool("nabu:ping").await);
    }

    #[tokio::test]
    async fn register_replaces_existing() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;
        registry.register(Arc::new(PingTool)).await;
        assert_eq!(registry.tool_count().await, 1);
    }

    #[tokio::test]
    async fn unregister_removes_tool() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;
        assert!(registry.unregister("nabu:ping").await);
        assert!(!registry.has_tool("nabu:ping").await);
        assert!(!registry.unregister("nabu:ping").await);
    }

    #[tokio::test]
    async fn unregister_prefix_removes_matching() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;
        registry.register(Arc::new(EchoTool)).await;
        registry.register(Arc::new(AddTool)).await;

        let removed = registry.unregister_prefix("nabu:").await;
        assert_eq!(removed, 3);
        assert_eq!(registry.tool_count().await, 0);
    }

    #[tokio::test]
    async fn list_returns_sorted_ids() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;
        registry.register(Arc::new(AddTool)).await;
        registry.register(Arc::new(EchoTool)).await;

        let ids = registry.list().await;
        let names: Vec<&str> = ids.iter().map(|id| id.0.as_str()).collect();
        assert_eq!(names, vec!["nabu:add", "nabu:echo", "nabu:ping"]);
    }

    #[tokio::test]
    async fn specs_returns_sorted_specs() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;
        registry.register(Arc::new(AddTool)).await;

        let specs = registry.specs().await;
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].id.0, "nabu:add");
        assert_eq!(specs[1].id.0, "nabu:ping");
    }

    #[tokio::test]
    async fn spec_returns_tool_spec() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).await;

        let spec = registry.spec("nabu:echo").await.unwrap();
        assert_eq!(spec.id, ToolId::new("nabu:echo"));
        assert_eq!(spec.name, "Echo");
        assert_eq!(spec.parameters.len(), 1);
        assert_eq!(spec.parameters[0].name, "msg");
        assert!(spec.parameters[0].required);
    }

    #[tokio::test]
    async fn spec_returns_none_for_unknown() {
        let registry = ToolRegistry::new();
        assert!(registry.spec("nabu:missing").await.is_none());
    }

    #[tokio::test]
    async fn call_dispatches_to_tool() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;

        let result = registry.call(ToolCall::without_args("nabu:ping")).await;
        assert!(result.is_success());
        assert_eq!(result.result, Some(json!("pong")));
    }

    #[tokio::test]
    async fn call_with_args_dispatches() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool)).await;

        let result = registry.call_with_args(
            "nabu:add",
            Some(json!({ "a": 3.0, "b": 4.0 })),
        ).await;
        assert!(result.is_success());
        assert_eq!(result.result, Some(json!(7.0)));
    }

    #[tokio::test]
    async fn call_unknown_tool_returns_not_found() {
        let registry = ToolRegistry::new();
        let result = registry.call(ToolCall::without_args("nabu:missing")).await;
        assert_eq!(result.status, ToolResultStatus::ToolNotFound);
        assert!(result.is_error());
    }

    #[tokio::test]
    async fn call_missing_required_params_returns_invalid_params() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool)).await;

        let result = registry
            .call_with_args("nabu:add", Some(json!({ "a": 1.0 })))
            .await;
        assert_eq!(result.status, ToolResultStatus::InvalidParams);
        assert_eq!(result.error.as_ref().unwrap().code, "INVALID_PARAMS");
        let err_msg = result.error.as_ref().unwrap().message.clone();
        assert!(err_msg.contains("b"));
    }

    #[tokio::test]
    async fn call_with_all_required_params_succeeds() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool)).await;

        let result = registry
            .call_with_args("nabu:add", Some(json!({ "a": 3.0, "b": 4.0 })))
            .await;
        assert!(result.is_success());
        assert_eq!(result.result, Some(json!(7.0)));
    }
}

// ---------------------------------------------------------------------------
// Integration tests (module: tool_calling)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tool_calling {
    use super::*;
    use crate::tool_calling::models::{ToolCall, ToolError, ToolParam, ToolParamSchema, ToolSpec};
    use crate::tool_calling::{ToolRegistry, ToolResult};
    use async_trait::async_trait;
    use serde_json::json;

    struct CounterTool {
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl Tool for CounterTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:counter", "Counter", "Counts invocations")
        }
        async fn call(&self, _call: ToolCall) -> Result<ToolResult, ToolError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(ToolResult::success(
                Some(json!({ "count": self.calls.load(std::sync::atomic::Ordering::SeqCst) })),
                None,
            ))
        }
    }

    struct ErrorTool;

    #[async_trait]
    impl Tool for ErrorTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:error", "Error", "Always returns an error result")
        }
        async fn call(&self, _call: ToolCall) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::error(
                ToolError::new("ERR_INTERNAL", "internal failure"),
                None,
            ))
        }
    }

    struct ParamsTool;

    #[async_trait]
    impl Tool for ParamsTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nabu:params", "Params", "Requires named params")
                .with_param(ToolParam::required("name", ToolParamSchema::of_type("string")))
                .with_param(ToolParam::optional("greeting", ToolParamSchema::of_type("string")))
        }
        async fn call(&self, call: ToolCall) -> Result<ToolResult, ToolError> {
            let name = call.arguments
                .as_ref()
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let greeting = call.arguments
                .as_ref()
                .and_then(|v| v.get("greeting"))
                .and_then(|v| v.as_str())
                .unwrap_or("Hello");
            Ok(ToolResult::success(
                Some(json!({ "message": format!("{}, {}!", greeting, name) })),
                None,
            ))
        }
    }

    #[tokio::test]
    async fn full_lifecycle_register_call_unregister() {
        let registry = ToolRegistry::new();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let tool = Arc::new(CounterTool { calls: counter });

        // Register
        registry.register(tool).await;
        assert_eq!(registry.tool_count().await, 1);

        // Call
        let result = registry.call(ToolCall::without_args("nabu:counter")).await;
        assert!(result.is_success());
        assert_eq!(result.result, Some(json!({ "count": 1 })));

        // Call again
        let result = registry.call(ToolCall::without_args("nabu:counter")).await;
        assert_eq!(result.result, Some(json!({ "count": 2 })));
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);

        // Unregister
        assert!(registry.unregister("nabu:counter").await);
        assert_eq!(registry.tool_count().await, 0);
    }

    #[tokio::test]
    async fn tool_not_found_returns_not_found_status() {
        let registry = ToolRegistry::new();
        let result = registry.call(ToolCall::without_args("nabu:nonexistent")).await;
        assert_eq!(result.status, ToolResultStatus::ToolNotFound);
        assert_eq!(result.error.as_ref().unwrap().code, "TOOL_NOT_FOUND");
    }

    #[tokio::test]
    async fn missing_required_param_returns_invalid_params() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(ParamsTool)).await;

        // Missing "name" (required)
        let result = registry
            .call_with_args("nabu:params", Some(json!({ "greeting": "Hi" })))
            .await;
        assert_eq!(result.status, ToolResultStatus::InvalidParams);
        assert_eq!(result.error.as_ref().unwrap().code, "INVALID_PARAMS");
        // Error message should mention the missing param
        let err_msg = result.error.as_ref().unwrap().message.clone();
        assert!(err_msg.contains("name"));
    }

    #[tokio::test]
    async fn valid_params_with_optional_omitted() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(ParamsTool)).await;

        // Only provide "name" (required), omit "greeting" (optional)
        let result = registry
            .call_with_args("nabu:params", Some(json!({ "name": "World" })))
            .await;
        assert!(result.is_success());
        assert_eq!(
            result.result,
            Some(json!({ "message": "Hello, World!" }))
        );
    }

    #[tokio::test]
    async fn valid_params_with_optional_provided() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(ParamsTool)).await;

        let result = registry
            .call_with_args("nabu:params", Some(json!({ "name": "World", "greeting": "Hi" })))
            .await;
        assert!(result.is_success());
        assert_eq!(
            result.result,
            Some(json!({ "message": "Hi, World!" }))
        );
    }

    #[tokio::test]
    async fn tool_returning_error_result_has_error_status() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(ErrorTool)).await;

        let result = registry.call(ToolCall::without_args("nabu:error")).await;
        assert_eq!(result.status, ToolResultStatus::Error);
        assert!(!result.is_success());
        assert_eq!(result.error.as_ref().unwrap().code, "ERR_INTERNAL");
    }

    #[tokio::test]
    async fn tool_returning_err_converts_to_error_result() {
        struct ErroredTool;

        #[async_trait]
        impl Tool for ErroredTool {
            fn spec(&self) -> ToolSpec {
                ToolSpec::new("nabu:errored", "Errored", "Returns Err")
            }
            async fn call(&self, _call: ToolCall) -> Result<ToolResult, ToolError> {
                Err(ToolError::new("ERR_EXEC", "execution failed"))
            }
        }

        let registry = ToolRegistry::new();
        registry.register(Arc::new(ErroredTool)).await;

        let result = registry.call(ToolCall::without_args("nabu:errored")).await;
        assert_eq!(result.status, ToolResultStatus::Error);
        assert_eq!(result.error.as_ref().unwrap().code, "ERR_EXEC");
        assert!(result.execution.is_some());
    }

    #[tokio::test]
    async fn call_records_execution_metadata() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;

        let result = registry.call(ToolCall::without_args("nabu:ping")).await;
        assert!(result.execution.is_none()); // PingTool doesn't provide execution meta
    }

    #[tokio::test]
    async fn register_replaces_existing_tool() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;

        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let replacement = Arc::new(CounterTool { calls: counter });
        registry.register(replacement).await;

        assert_eq!(registry.tool_count().await, 1);
        assert!(!registry.has_tool("nabu:ping").await);
        assert!(registry.has_tool("nabu:counter").await);
    }

    #[tokio::test]
    async fn specs_lists_all_registered() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;
        registry.register(Arc::new(ErrorTool)).await;

        let specs = registry.specs().await;
        assert_eq!(specs.len(), 2);
        let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Ping"));
        assert!(names.contains(&"Error"));
    }

    #[tokio::test]
    async fn call_with_args_no_arguments_for_tool_without_params() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(PingTool)).await;

        // Tool with no params; arguments = None should work
        let result = registry
            .call_with_args("nabu:ping", None)
            .await;
        assert!(result.is_success());
        assert_eq!(result.result, Some(json!("pong")));
    }

    #[tokio::test]
    async fn concurrent_calls_are_supported() {
        let registry = Arc::new(ToolRegistry::new());
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        registry.register(Arc::new(CounterTool { calls: counter.clone() })).await;

        // Spawn 10 concurrent calls
        let calls: Vec<_> = (0..10)
            .map(|_| {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    registry
                        .call(ToolCall::without_args("nabu:counter"))
                        .await
                })
            })
            .collect();

        let mut count = 0;
        for handle in calls {
            let result = handle.await.unwrap();
            assert!(result.is_success());
            count += 1;
        }
        assert_eq!(count, 10);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 10);
    }
}

//! Core data models for the Tool Calling Framework.
//!
//! These types are the canonical wire-format representations for tool
//! registration, discovery, and invocation. They are pure data models —
//! no runtime behavior, no I/O, no transport. All types derive
//! `Serialize` + `Deserialize` so they can be sent across any boundary
//! (IPC, sockets, event bus).
//!
//! ## Architecture
//!
//! ```text
//! ToolSpec ── identifies ──▶ ToolId
//!   │                            │
//!   │  (params, required, etc.)  │  (name, description)
//!   ▼                            ▼
//! ToolCall ── invokes ──▶ Tool (trait) ── returns ──▶ ToolResult
//! ```
//!
//! ## Design Principles
//!
//! - **Strongly typed**: `ToolId` wraps a `String` to prevent confusion with
//!   other identifiers (method names, capability IDs, etc.).
//! - **Forward compatible**: all structs use `#[serde(default)]` and `Option<T>`
//!   for future fields, so newer requests can be deserialized by older hosts.
//! - **Serializable**: every type derives `Serialize` + `Deserialize`.
//! - **Thread-safe**: all types are `Send + Sync + Clone`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// ToolId — strong identifier
// ---------------------------------------------------------------------------

/// A unique tool identifier.
///
/// Tools are identified by a string name within a namespace. This wrapper
/// type prevents accidental confusion with other string-based identifiers
/// (capability IDs, method names, etc.).
///
/// # Serialization
///
/// Uses `#[serde(transparent)]` so it serializes as a plain string, keeping
/// the wire format clean and compatible with existing JSON-RPC conventions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct ToolId(pub String);

impl ToolId {
    /// Create a new `ToolId` from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for ToolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ToolId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ToolId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// ToolSpec — declarative description
// ---------------------------------------------------------------------------

/// JSON Schema type for a tool parameter.
///
/// This is a simplified subset of JSON Schema sufficient for tool parameter
/// validation and UI generation. It does not attempt to be a complete
/// JSON Schema implementation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ToolParamSchema {
    /// The JSON type (e.g. "string", "number", "boolean", "object", "array").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Human-readable description of this parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// For string params, an optional enum of allowed values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    /// For object params, a map of property names to sub-schemas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, ToolParamSchema>>,
    /// For object params, which properties are required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

impl ToolParamSchema {
    /// Create a simple type-only schema.
    pub fn of_type(r#type: impl Into<String>) -> Self {
        Self {
            r#type: Some(r#type.into()),
            ..Default::default()
        }
    }
}

/// A parameter declaration for a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolParam {
    /// The parameter name (used as the key in the `arguments` map).
    pub name: String,
    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,
    /// The schema / type description for this parameter.
    #[serde(default)]
    pub schema: ToolParamSchema,
}

impl ToolParam {
    /// Create a required parameter with the given schema.
    pub fn required(name: impl Into<String>, schema: ToolParamSchema) -> Self {
        Self {
            name: name.into(),
            required: true,
            schema,
        }
    }

    /// Create an optional parameter with the given schema.
    pub fn optional(name: impl Into<String>, schema: ToolParamSchema) -> Self {
        Self {
            name: name.into(),
            required: false,
            schema,
        }
    }
}

/// Declarative description of a tool.
///
/// `ToolSpec` describes a tool's identity and interface without tying it to
/// any implementation. The `ToolRegistry` stores specs alongside the
/// executable `Tool` implementation so they can be discovered and enumerated.
///
/// # Example JSON
///
/// ```json
/// {
///   "id": "nabu:read_note",
///   "name": "Read Note",
///   "description": "Read a note by path from the vault.",
///   "parameters": [
///     { "name": "path", "required": true, "schema": { "type": "string" } }
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Unique tool identifier (e.g. `"nabu:read_note"`).
    pub id: ToolId,
    /// Human-readable name for display.
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// Parameters the tool accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ToolParam>,
}

impl ToolSpec {
    /// Create a new tool spec with the given id, name, and description.
    pub fn new(id: impl Into<ToolId>, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            parameters: Vec::new(),
        }
    }

    /// Add a parameter to this tool spec.
    pub fn with_param(mut self, param: ToolParam) -> Self {
        self.parameters.push(param);
        self
    }

    /// Returns the IDs of required parameters.
    pub fn required_param_names(&self) -> Vec<&str> {
        self.parameters
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect()
    }

    /// Returns a reference to a parameter by name, if it exists.
    pub fn param(&self, name: &str) -> Option<&ToolParam> {
        self.parameters.iter().find(|p| p.name == name)
    }
}

// ---------------------------------------------------------------------------
// ToolCall — invocation request
// ---------------------------------------------------------------------------

/// A request to invoke a tool.
///
/// A `ToolCall` identifies which tool to call (by `ToolId`) and provides
/// the raw JSON arguments. Validation of arguments against the tool's
/// `ToolSpec` is the responsibility of the `Tool` implementation or the
/// `ToolRegistry` before execution.
///
/// # Example JSON
///
/// ```json
/// {
///   "tool_id": "nabu:read_note",
///   "arguments": { "path": "Inbox.md" }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The ID of the tool to invoke.
    pub tool_id: ToolId,
    /// The arguments to pass to the tool, as a JSON object.
    ///
    /// The shape is defined by the tool's `ToolSpec`. May be `null` for
    /// tools that take no parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

impl ToolCall {
    /// Create a new tool call with the given tool ID and optional arguments.
    pub fn new(tool_id: impl Into<ToolId>, arguments: Option<serde_json::Value>) -> Self {
        Self {
            tool_id: tool_id.into(),
            arguments,
        }
    }

    /// Create a new tool call with no arguments.
    pub fn without_args(tool_id: impl Into<ToolId>) -> Self {
        Self {
            tool_id: tool_id.into(),
            arguments: None,
        }
    }

    /// Create a new tool call with the given arguments as a JSON object.
    pub fn with_args(tool_id: impl Into<ToolId>, args: serde_json::Value) -> Self {
        Self {
            tool_id: tool_id.into(),
            arguments: Some(args),
        }
    }

    /// Validate that all required parameters (per the spec) are present in
    /// the arguments.
    ///
    /// Returns `Err` with a list of missing parameter names.
    pub fn validate_against(&self, spec: &ToolSpec) -> Result<(), Vec<String>> {
        if self.tool_id != spec.id {
            return Err(vec![format!(
                "tool_id '{}' does not match spec id '{}'",
                self.tool_id, spec.id
            )]);
        }

        let required = spec.required_param_names();
        if required.is_empty() {
            return Ok(());
        }

        let args = match &self.arguments {
            Some(serde_json::Value::Object(map)) => map,
            _ => {
                // No arguments provided — all required params are missing.
                return Err(required.iter().map(|s| s.to_string()).collect());
            }
        };

        let missing: Vec<String> = required
            .iter()
            .filter(|name| !args.contains_key(&name.to_string()))
            .map(|s| s.to_string())
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

// ---------------------------------------------------------------------------
// ToolResult — invocation result
// ---------------------------------------------------------------------------

/// The status of a tool execution result.
///
/// Distinguishes success from structured failure, cancellation, and errors
/// that occurred during validation (before the tool was executed).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolResultStatus {
    /// The tool executed and returned a result.
    #[serde(rename = "success")]
    Success,
    /// The tool returned an error.
    #[serde(rename = "error")]
    Error,
    /// The tool call was cancelled (e.g. by timeout).
    #[serde(rename = "cancelled")]
    Cancelled,
    /// The tool was not found in the registry.
    #[serde(rename = "tool_not_found")]
    ToolNotFound,
    /// The arguments failed validation.
    #[serde(rename = "invalid_params")]
    InvalidParams,
}

impl Default for ToolResultStatus {
    fn default() -> Self {
        Self::Success
    }
}

/// Error information for a failed tool result.
///
/// The `code` field provides a stable, machine-parseable error category
/// that future SDK clients can match on. The `message` field provides a
/// human-readable description for UI display. Internal implementation
/// details are never leaked — errors are normalized at the framework level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolError {
    /// Machine-parseable error code (e.g. `"TOOL_EXECUTION_FAILED"`).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional detailed error info for debugging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ToolError {
    /// Create a new tool error with the given code and message.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    /// Create a new tool error with additional detail.
    pub fn with_detail(
        code: impl Into<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: Some(detail.into()),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(detail) = &self.detail {
            write!(f, "[{}] {} ({})", self.code, self.message, detail)
        } else {
            write!(f, "[{}] {}", self.code, self.message)
        }
    }
}

impl std::error::Error for ToolError {}

/// Execution metadata for a tool result.
///
/// Records which tool handled the request and how long it took, enabling
/// observability and performance monitoring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ToolExecutionMeta {
    /// The tool ID that was invoked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<ToolId>,
    /// How long the tool execution took, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Optional trace/correlation ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl ToolExecutionMeta {
    /// Create execution metadata from a tool ID and duration.
    pub fn from_duration(tool_id: ToolId, duration: Duration) -> Self {
        Self {
            tool_id: Some(tool_id),
            duration_ms: Some(duration.as_millis() as u64),
            trace_id: None,
        }
    }
}

/// The result of a tool invocation.
///
/// `ToolResult` is the canonical response for all tool calls. It carries
/// the result data (on success) or structured error information (on failure).
/// The `status` field distinguishes success, error, cancellation, and
/// validation failures.
///
/// # Example JSON (success)
///
/// ```json
/// {
///   "status": "success",
///   "result": { "path": "note.md", "content": "..." },
///   "execution": { "tool_id": "nabu:read_note", "duration_ms": 12 }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// The high-level status of the tool execution.
    pub status: ToolResultStatus,
    /// The result data, present on success. An opaque JSON value whose
    /// schema is defined by the tool implementation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Structured error information, present when `status` is `Error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolError>,
    /// Execution metadata — which tool ran and how long it took.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ToolExecutionMeta>,
}

impl ToolResult {
    /// Construct a successful result with the given result data.
    pub fn success(
        result: Option<serde_json::Value>,
        execution: Option<ToolExecutionMeta>,
    ) -> Self {
        Self {
            status: ToolResultStatus::Success,
            result,
            error: None,
            execution,
        }
    }

    /// Construct an error result.
    pub fn error(
        error: ToolError,
        execution: Option<ToolExecutionMeta>,
    ) -> Self {
        Self {
            status: ToolResultStatus::Error,
            result: None,
            error: Some(error),
            execution,
        }
    }

    /// Construct a cancelled result.
    pub fn cancelled(execution: Option<ToolExecutionMeta>) -> Self {
        Self {
            status: ToolResultStatus::Cancelled,
            result: None,
            error: None,
            execution,
        }
    }

    /// Construct a "tool not found" result.
    pub fn tool_not_found(tool_id: ToolId) -> Self {
        Self {
            status: ToolResultStatus::ToolNotFound,
            result: None,
            error: Some(ToolError::new(
                "TOOL_NOT_FOUND",
                format!("Tool not found: {}", tool_id),
            )),
            execution: Some(ToolExecutionMeta {
                tool_id: Some(tool_id),
                ..Default::default()
            }),
        }
    }

    /// Construct an "invalid params" result.
    pub fn invalid_params(tool_id: ToolId, missing: Vec<String>) -> Self {
        Self {
            status: ToolResultStatus::InvalidParams,
            result: None,
            error: Some(ToolError::new(
                "INVALID_PARAMS",
                format!("Missing required parameters: {}", missing.join(", ")),
            )),
            execution: Some(ToolExecutionMeta {
                tool_id: Some(tool_id),
                ..Default::default()
            }),
        }
    }

    /// Returns `true` if this result indicates success.
    pub fn is_success(&self) -> bool {
        self.status == ToolResultStatus::Success
    }

    /// Returns `true` if this result indicates an error.
    pub fn is_error(&self) -> bool {
        matches!(
            self.status,
            ToolResultStatus::Error | ToolResultStatus::ToolNotFound | ToolResultStatus::InvalidParams
        ) || !self.is_success()
    }
}

// ---------------------------------------------------------------------------
// Serialization round-trip tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_id_round_trips() {
        let id = ToolId::new("nabu:read_note");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"nabu:read_note\"");
        let back: ToolId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn tool_spec_round_trips() {
        let spec = ToolSpec::new("nabu:read_note", "Read Note", "Read a note")
            .with_param(ToolParam::required("path", ToolParamSchema::of_type("string")));

        let json = serde_json::to_string(&spec).unwrap();
        let back: ToolSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
        assert_eq!(back.id, ToolId::new("nabu:read_note"));
        assert_eq!(back.name, "Read Note");
        assert_eq!(back.parameters.len(), 1);
        assert_eq!(back.parameters[0].name, "path");
        assert!(back.parameters[0].required);
    }

    #[test]
    fn tool_spec_empty_parameters_serialize() {
        let spec = ToolSpec::new("nabu:ping", "Ping", "No params tool");
        let json = serde_json::to_string(&spec).unwrap();
        let back: ToolSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back.parameters, Vec::new());
    }

    #[test]
    fn tool_spec_required_param_names() {
        let spec = ToolSpec::new("nabu:test", "Test", "test")
            .with_param(ToolParam::required("a", ToolParamSchema::of_type("string")))
            .with_param(ToolParam::optional("b", ToolParamSchema::of_type("number")));

        let required = spec.required_param_names();
        assert_eq!(required, vec!["a"]);
    }

    #[test]
    fn tool_call_without_args_round_trips() {
        let call = ToolCall::without_args("nabu:ping");
        let json = serde_json::to_string(&call).unwrap();
        let back: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(call, back);
        assert_eq!(back.arguments, None);
    }

    #[test]
    fn tool_call_with_args_round_trips() {
        let call = ToolCall::with_args("nabu:read_note", serde_json::json!({ "path": "test.md" }));
        let json = serde_json::to_string(&call).unwrap();
        let back: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(call, back);
        assert!(back.arguments.is_some());
    }

    #[test]
    fn tool_call_validates_required_params_present() {
        let spec = ToolSpec::new("nabu:read_note", "Read", "read")
            .with_param(ToolParam::required("path", ToolParamSchema::of_type("string")));

        let call = ToolCall::with_args("nabu:read_note", serde_json::json!({ "path": "note.md" }));
        assert!(call.validate_against(&spec).is_ok());
    }

    #[test]
    fn tool_call_validates_required_params_missing() {
        let spec = ToolSpec::new("nabu:read_note", "Read", "read")
            .with_param(ToolParam::required("path", ToolParamSchema::of_type("string")));

        let call = ToolCall::without_args("nabu:read_note");
        let err = call.validate_against(&spec).unwrap_err();
        assert_eq!(err, vec!["path".to_string()]);
    }

    #[test]
    fn tool_call_validates_missing_some_required() {
        let spec = ToolSpec::new("nabu:test", "Test", "test")
            .with_param(ToolParam::required("a", ToolParamSchema::of_type("string")))
            .with_param(ToolParam::required("b", ToolParamSchema::of_type("number")));

        let call = ToolCall::with_args("nabu:test", serde_json::json!({ "a": "val" }));
        let err = call.validate_against(&spec).unwrap_err();
        assert_eq!(err, vec!["b".to_string()]);
    }

    #[test]
    fn tool_call_validation_wrong_tool_id() {
        let spec = ToolSpec::new("nabu:tool_a", "A", "desc");
        let call = ToolCall::without_args("nabu:tool_b");
        let err = call.validate_against(&spec).unwrap_err();
        assert!(err[0].contains("tool_id"));
    }

    #[test]
    fn tool_result_success_round_trips() {
        let result = ToolResult::success(
            Some(serde_json::json!({ "content": "hello" })),
            Some(ToolExecutionMeta::from_duration(
                ToolId::new("nabu:test"),
                Duration::from_millis(42),
            )),
        );
        let json = serde_json::to_string(&result).unwrap();
        let back: ToolResult = serde_json::from_str(&json).unwrap();
        assert!(back.is_success());
        assert_eq!(back.result, Some(serde_json::json!({ "content": "hello" })));
        assert_eq!(back.execution.as_ref().unwrap().duration_ms, Some(42));
    }

    #[test]
    fn tool_result_error_round_trips() {
        let result = ToolResult::error(
            ToolError::new("TEST_ERROR", "something went wrong"),
            Some(ToolExecutionMeta::from_duration(
                ToolId::new("nabu:test"),
                Duration::from_millis(5),
            )),
        );
        let json = serde_json::to_string(&result).unwrap();
        let back: ToolResult = serde_json::from_str(&json).unwrap();
        assert!(!back.is_success());
        assert!(back.is_error());
        assert_eq!(back.error.as_ref().unwrap().code, "TEST_ERROR");
    }

    #[test]
    fn tool_result_tool_not_found() {
        let result = ToolResult::tool_not_found(ToolId::new("nabu:missing"));
        assert_eq!(result.status, ToolResultStatus::ToolNotFound);
        assert!(result.is_error());
        assert_eq!(result.error.as_ref().unwrap().code, "TOOL_NOT_FOUND");
    }

    #[test]
    fn tool_result_invalid_params() {
        let result = ToolResult::invalid_params(ToolId::new("nabu:test"), vec!["path".to_string()]);
        assert_eq!(result.status, ToolResultStatus::InvalidParams);
        assert!(result.is_error());
    }

    #[test]
    fn tool_error_with_detail_round_trips() {
        let err = ToolError::with_detail("ERR_1", "message", "detail text");
        let json = serde_json::to_string(&err).unwrap();
        let back: ToolError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, "ERR_1");
        assert_eq!(back.message, "message");
        assert_eq!(back.detail.as_deref(), Some("detail text"));
    }

    #[test]
    fn tool_error_without_detail_omits_field() {
        let err = ToolError::new("ERR_1", "message");
        let json = serde_json::to_string(&err).unwrap();
        assert!(!json.contains("detail"));
    }

    #[test]
    fn tool_result_status_default_is_success() {
        assert_eq!(ToolResultStatus::default(), ToolResultStatus::Success);
    }

    #[test]
    fn tool_result_is_error_for_all_failure_statuses() {
        assert!(ToolResult::tool_not_found(ToolId::new("x")).is_error());
        assert!(ToolResult::invalid_params(ToolId::new("x"), vec![]).is_error());
        assert!(ToolResult::cancelled(None).is_error());
        let err_result = ToolResult::error(
            ToolError::new("E", "m"),
            None,
        );
        assert!(err_result.is_error());
    }

    #[test]
    fn tool_param_schema_of_type() {
        let schema = ToolParamSchema::of_type("string");
        assert_eq!(schema.r#type.as_deref(), Some("string"));
        assert!(schema.description.is_none());
    }

    #[test]
    fn tool_call_id_matches_spec_id() {
        let spec = ToolSpec::new("nabu:test", "Test", "desc");
        let call = ToolCall::without_args("nabu:test");
        assert!(call.validate_against(&spec).is_ok());
    }
}

//! Plugin Invocation Models — the structured request/response contracts for
//! the `plugin_call` IPC command.
//!
//! ## Architecture
//!
//! These types are the canonical wire format between the frontend and the
//! host application's plugin invocation bridge. All plugin invocations follow
//! the same path:
//!
//! ```text
//! Frontend
//!   │  (serializes PluginInvocationRequest as JSON)
//!   ▼
//! plugin_call IPC command
//!   │  (deserializes, validates)
//!   ▼
//! PluginManager.invoke_capability()
//!   │  (locates provider, validates capability)
//!   ▼
//! CapabilityProvider::invoke()
//!   │  (plugin-specific execution)
//!   ▼
//! PluginInvocationResponse
//!   │  (serialized by the IPC layer)
//!   ▼
//! Frontend
//! ```
//!
//! ## Design Principles
//!
//! - **Strongly typed**: `PluginId`, `CapabilityId`, and `MethodId` are
//!   distinct wrapper types to prevent accidental confusion between
//!   identifiers that happen to share the same string representation.
//! - **Forward compatible**: all structs use `#[serde(default)]` and
//!   `Option<T>` for future fields, so newer requests can be deserialized by
//!   older hosts and vice versa.
//! - **Serializable**: every type derives `Serialize` + `Deserialize` so that
//!   future remote plugins and SDKs can use the same models over a JSON wire
//!   protocol.
//! - **Plugin-agnostic**: the response model does not leak provider-specific
//!   internals. The `result` field carries an opaque `serde_json::Value`.
//! - **Thread-safe**: all types are `Send + Sync + Clone`, supporting
//!   concurrent invocations from the IPC layer.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Identification wrappers
// ---------------------------------------------------------------------------

/// A plugin's unique identifier (reverse-domain notation).
///
/// Wraps a `String` to provide a strongly typed identifier that cannot be
/// confused with other string-based IDs (capability IDs, method names, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(pub String);

impl PluginId {
    /// Create a new `PluginId` from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for PluginId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PluginId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A capability identifier in `{namespace}:{name}` form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(pub String);

impl CapabilityId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for CapabilityId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CapabilityId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A method name within a capability (e.g. `"read"`, `"search"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MethodId(pub String);

impl MethodId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for MethodId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MethodId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MethodId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Optional invocation metadata — timing, trace context, caller info.
///
/// Fields are optional so that callers can omit metadata that is not
/// relevant to a specific invocation. The host fills in defaults
/// (request ID, timestamp) when not provided.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InvocationMetadata {
    /// Unique ID for this invocation — used for tracing and correlation.
    /// If not provided by the caller, the host generates one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Uuid>,
    /// When the invocation was initiated. Populated by the host if absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional timeout hint from the caller. The provider MAY enforce this
    /// but is not required to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Optional caller identity (e.g. `"nabu-ui"`, `"nabu-cli"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    /// API version requested by the caller for version negotiation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    /// Additional arbitrary metadata key-value pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Map<String, serde_json::Value>>,
}

// ---------------------------------------------------------------------------
// Request & Response
// ---------------------------------------------------------------------------

/// A request to invoke a plugin capability.
///
/// This is the structured request model that the frontend serializes and
/// sends via the `plugin_call` IPC command. The host deserializes it,
/// validates it, locates the target provider through the `PluginManager`,
/// and dispatches to the provider's `invoke` method.
///
/// # Example JSON
///
/// ```json
/// {
///   "plugin_id": "com.example.ocr",
///   "capability": "ocr:tesseract",
///   "method": "recognize",
///   "input": { "image_data": "..." },
///   "metadata": {
///     "request_id": "550e8400-e29b-41d4-a716-446655440000",
///     "timeout_ms": 5000,
///     "caller": "nabu-ui"
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInvocationRequest {
    /// The plugin identifier (reverse-domain notation).
    pub plugin_id: String,
    /// The capability identifier (`{namespace}:{name}`).
    pub capability: String,
    /// The method or operation to invoke on the capability.
    pub method: String,
    /// Arbitrary input payload as JSON. Providers interpret this according
    /// to their capability contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    /// Invocation metadata (request ID, timeout, caller, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<InvocationMetadata>,
}

impl PluginInvocationRequest {
    /// Create a new invocation request with the minimum required fields.
    pub fn new(plugin_id: impl Into<String>, capability: impl Into<String>, method: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            capability: capability.into(),
            method: method.into(),
            input: None,
            metadata: None,
        }
    }

    /// Set the input payload (builder pattern).
    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Set invocation metadata (builder pattern).
    pub fn with_metadata(mut self, metadata: InvocationMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Validate that the request has all required fields populated.
    pub fn validate(&self) -> Result<(), String> {
        if self.plugin_id.is_empty() {
            return Err("plugin_id must not be empty".to_string());
        }
        if self.capability.is_empty() {
            return Err("capability must not be empty".to_string());
        }
        if self.method.is_empty() {
            return Err("method must not be empty".to_string());
        }
        Ok(())
    }

    /// Generate a request ID if one is not already present in the metadata.
    /// Returns the request ID to use for this invocation.
    pub fn ensure_request_id(&mut self) -> Uuid {
        let md = self.metadata.get_or_insert_with(InvocationMetadata::default);
        if let Some(id) = md.request_id {
            id
        } else {
            let id = Uuid::new_v4();
            md.request_id = Some(id);
            id
        }
    }

    /// Returns the effective timeout for this invocation.
    /// If not specified in metadata, defaults to 30 seconds.
    pub fn timeout(&self) -> Duration {
        self.metadata
            .as_ref()
            .and_then(|m| m.timeout_ms)
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_secs(30))
    }
}

/// The result of a plugin invocation.
///
/// `PluginInvocationStatus` distinguishes success from structured failure,
/// and `PluginInvocationError` provides additional context on errors without
/// leaking internal implementation details.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginInvocationStatus {
    /// The invocation completed successfully.
    #[serde(rename = "success")]
    Success,
    /// The invocation failed — see the `error` field for details.
    #[serde(rename = "error")]
    Error,
    /// The invocation was cancelled (e.g. by a timeout or the caller).
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl Default for PluginInvocationStatus {
    fn default() -> Self {
        Self::Success
    }
}

/// Structured error returned by a plugin invocation.
///
/// The `code` field provides a stable, machine-parseable error category
/// that future SDK clients can match on. The `message` field provides a
/// human-readable description for UI display. Internal implementation
/// details are never leaked — errors are normalized at the IPC boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInvocationError {
    /// Machine-parseable error code (e.g. `"PLUGIN_NOT_FOUND"`).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional detailed error info for debugging (may be stripped in
    /// non-dev builds by the IPC layer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl PluginInvocationError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(code: impl Into<String>, message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: Some(detail.into()),
        }
    }
}

impl std::fmt::Display for PluginInvocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(detail) = &self.detail {
            write!(f, "[{}] {} ({})", self.code, self.message, detail)
        } else {
            write!(f, "[{}] {}", self.code, self.message)
        }
    }
}

impl std::error::Error for PluginInvocationError {}

/// The structured response returned by the `plugin_call` IPC command.
///
/// This is the canonical response model for all plugin invocations. It is
/// plugin-agnostic: the `result` field carries an opaque JSON value that the
/// provider defines, while `error`, `status`, and `execution` provide the
/// host-level metadata about how the invocation was processed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInvocationResponse {
    /// Whether the invocation succeeded.
    pub success: bool,
    /// The high-level status (success / error / cancelled).
    pub status: PluginInvocationStatus,
    /// The invocation result, if successful. An opaque JSON value whose
    /// schema is defined by the capability provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Structured error information, present when `success` is `false`
    /// and `status` is `Error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<PluginInvocationError>,
    /// Execution metadata — which provider handled the request and how long
    /// it took.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionMetadata>,
}

/// Metadata about how an invocation was executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ExecutionMetadata {
    /// The unique request ID used for tracing and correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<uuid::Uuid>,
    /// The provider ID that handled the invocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The capability ID that was invoked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    /// How long the invocation took, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// The API version negotiated for this invocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
}

impl PluginInvocationResponse {
    /// Construct a successful response with the given result.
    pub fn success(result: Option<serde_json::Value>, execution: Option<ExecutionMetadata>) -> Self {
        Self {
            success: true,
            status: PluginInvocationStatus::Success,
            result,
            error: None,
            execution,
        }
    }

    /// Construct an error response.
    pub fn error(error: PluginInvocationError, execution: Option<ExecutionMetadata>) -> Self {
        Self {
            success: false,
            status: PluginInvocationStatus::Error,
            result: None,
            error: Some(error),
            execution,
        }
    }

    /// Construct a cancelled response.
    pub fn cancelled(execution: Option<ExecutionMetadata>) -> Self {
        Self {
            success: false,
            status: PluginInvocationStatus::Cancelled,
            result: None,
            error: None,
            execution,
        }
    }

    /// Convenience: did the invocation produce an error?
    pub fn is_error(&self) -> bool {
        !self.success
    }
}

// ---------------------------------------------------------------------------
// Serialization round-trip tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod serialization {
    use super::*;

    #[test]
    fn request_round_trips() {
        let req = PluginInvocationRequest::new("com.example.ocr", "ocr:tesseract", "recognize")
            .with_input(serde_json::json!({ "image": "base64data" }))
            .with_metadata(InvocationMetadata {
                request_id: Some(Uuid::nil()),
                timeout_ms: Some(5000),
                caller: Some("nabu-ui".to_string()),
                ..Default::default()
            });

        let json = serde_json::to_string(&req).unwrap();
        let back: PluginInvocationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
        assert_eq!(back.plugin_id, "com.example.ocr");
        assert_eq!(back.capability, "ocr:tesseract");
        assert_eq!(back.method, "recognize");
        assert!(back.input.is_some());
    }

    #[test]
    fn response_success_round_trips() {
        let resp = PluginInvocationResponse::success(
            Some(serde_json::json!({ "text": "hello" })),
            Some(ExecutionMetadata {
                request_id: Some(Uuid::nil()),
                provider: Some("com.example.ocr".to_string()),
                capability: Some("ocr:tesseract".to_string()),
                duration_ms: Some(42),
                api_version: Some("1.0".to_string()),
            }),
        );

        let json = serde_json::to_string(&resp).unwrap();
        let back: PluginInvocationResponse = serde_json::from_str(&json).unwrap();
        assert!(back.success);
        assert_eq!(back.result, Some(serde_json::json!({ "text": "hello" })));
        assert_eq!(back.execution.as_ref().unwrap().duration_ms, Some(42));
    }

    #[test]
    fn response_error_round_trips() {
        let err = PluginInvocationError::new("PLUGIN_NOT_FOUND", "Plugin not found");
        let resp = PluginInvocationResponse::error(
            err,
            Some(ExecutionMetadata {
                request_id: None,
                provider: None,
                capability: Some("ocr:tesseract".to_string()),
                duration_ms: Some(5),
                api_version: None,
            }),
        );

        let json = serde_json::to_string(&resp).unwrap();
        let back: PluginInvocationResponse = serde_json::from_str(&json).unwrap();
        assert!(!back.success);
        assert_eq!(back.error.as_ref().unwrap().code, "PLUGIN_NOT_FOUND");
    }

    #[test]
    fn request_validates() {
        let req = PluginInvocationRequest::new("", "ocr:tesseract", "recognize");
        assert!(req.validate().is_err());

        let req = PluginInvocationRequest::new("plugin", "", "recognize");
        assert!(req.validate().is_err());

        let req = PluginInvocationRequest::new("plugin", "ocr:tesseract", "");
        assert!(req.validate().is_err());

        let req = PluginInvocationRequest::new("plugin", "ocr:tesseract", "recognize");
        assert!(req.validate().is_ok());
    }

    #[test]
    fn request_ensure_request_id_generates_uuid() {
        let mut req = PluginInvocationRequest::new("p", "ns:m", "do");
        assert!(req.metadata.is_none());
        let id = req.ensure_request_id();
        assert!(req.metadata.is_some());
        assert_eq!(req.metadata.as_ref().unwrap().request_id, Some(id));
    }

    #[test]
    fn capability_id_from_string() {
        let id: CapabilityId = "nabu:storage".into();
        assert_eq!(id.0, "nabu:storage");
        assert_eq!(id.to_string(), "nabu:storage");
    }

    #[test]
    fn empty_request_minimal_fields() {
        // A request with no input or metadata should still serialize.
        let req = PluginInvocationRequest::new("p", "ns:m", "method");
        let json = serde_json::to_string(&req).unwrap();
        let back: PluginInvocationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.input, None);
        assert_eq!(back.metadata, None);
    }
}

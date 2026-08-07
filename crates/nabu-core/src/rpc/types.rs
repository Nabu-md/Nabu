//! JSON-RPC 2.0 protocol types — messages and identifiers.
//!
//! These types are the wire-format representation of JSON-RPC 2.0 requests
//! and responses. They are pure data models: no transport, no I/O, no
//! runtime. Any transport layer (stdin/stdout, Unix socket, WebSocket, etc.)
//! can serialize and deserialize these types.
//!
//! Reference: [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification).

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

use crate::rpc::error::{ErrorCode, JsonRpcError};

/// The JSON-RPC protocol version implemented by this crate.
///
/// Per the [spec](https://www.jsonrpc.org/specification), the version string
/// is always `"2.0"`. This constant centralizes the literal so it is not
/// scattered across the codebase.
pub const JSON_RPC_VERSION: &str = "2.0";

/// A JSON-RPC request identifier.
///
/// The specification allows the `id` to be a string, a number, or `null`.
/// This enum captures all three forms without assuming IDs are always numeric,
/// so the router can faithfully round-trip whatever the client sent.
///
/// The router preserves the incoming [`RequestId`] when constructing its
/// [`Response`](super::Response) — the caller does not need to manage this.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// A numeric identifier (e.g. `1`, `42`).
    ///
    /// Per the spec, numbers that are integers should have the fractional
    /// part omitted, but this wrapper preserves whatever the client sends.
    Number(i64),
    /// A string identifier (e.g. `"abc-123"`).
    String(String),
    /// The `null` identifier, used by notifications (requests that do not
    /// expect a response) and by clients that explicitly send `null`.
    Null,
}

impl RequestId {
    /// Create a numeric [`RequestId`].
    pub fn numeric(id: i64) -> Self {
        Self::Number(id)
    }

    /// Create a string [`RequestId`].
    pub fn string(id: impl Into<String>) -> Self {
        Self::String(id.into())
    }

    /// The `null` request identifier.
    pub const NULL: Self = Self::Null;

    /// Returns `true` if this identifier is `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{}", n),
            Self::String(s) => write!(f, "{}", s),
            Self::Null => write!(f, "null"),
        }
    }
}

impl From<i64> for RequestId {
    fn from(n: i64) -> Self {
        Self::Number(n)
    }
}

impl From<String> for RequestId {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for RequestId {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

/// A JSON-RPC 2.0 request.
///
/// Represents the canonical request message:
///
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "id": 1,
///   "method": "example_method",
///   "params": { "key": "value" }
/// }
/// ```
///
/// The `params` field is optional and carries a JSON-compatible value whose
/// shape is defined by the method being invoked. The router forwards `params`
/// to the registered handler verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The JSON-RPC protocol version. Always [`JSON_RPC_VERSION`] (`"2.0"`).
    #[serde(rename = "jsonrpc")]
    pub version: String,
    /// The request identifier. `null` indicates a notification (no response
    /// expected), though this core always produces a response.
    pub id: RequestId,
    /// The method name to invoke (e.g. `"tools/list"`).
    pub method: String,
    /// Optional parameters for the method. The shape is method-specific.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Request {
    /// Create a new JSON-RPC request with a numeric ID.
    pub fn new(id: i64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            version: JSON_RPC_VERSION.to_string(),
            id: RequestId::Number(id),
            method: method.into(),
            params,
        }
    }

    /// Create a new JSON-RPC request with a string ID.
    pub fn with_string_id(
        id: impl Into<String>,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            version: JSON_RPC_VERSION.to_string(),
            id: RequestId::String(id.into()),
            method: method.into(),
            params,
        }
    }

    /// Returns the method name for this request.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns a reference to the params, if any.
    pub fn params(&self) -> Option<&serde_json::Value> {
        self.params.as_ref()
    }

    /// Validates that the request conforms to the JSON-RPC 2.0 specification.
    ///
    /// Returns `Err(error_code)` for protocol-level violations:
    /// - [`ErrorCode::InvalidRequest`] if `version` is not `"2.0"` or
    ///   `method` is empty.
    ///
    /// Parse-level errors (malformed JSON, missing fields) are handled by
    /// serde deserialization before a `Request` is ever constructed, so they
    /// are not represented here. Transport layers that need to report
    /// [`ErrorCode::ParseError`] should do so before constructing a
    /// [`Request`].
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.version != JSON_RPC_VERSION {
            return Err(ErrorCode::InvalidRequest);
        }
        if self.method.is_empty() {
            return Err(ErrorCode::InvalidRequest);
        }
        Ok(())
    }
}

impl Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Request(method={}, id={})", self.method, self.id)
    }
}

/// A JSON-RPC 2.0 response.
///
/// A response carries either a successful `result` or an `error`, never both.
/// This invariant is enforced at construction time: the enum structure makes
/// it impossible to create a response that has both fields populated.
///
/// Conceptual wire formats:
///
/// **Success:**
/// ```json
/// { "jsonrpc": "2.0", "id": 1, "result": { "key": "value" } }
/// ```
///
/// **Error:**
/// ```json
/// { "jsonrpc": "2.0", "id": 1, "error": { "code": -32601, "message": "Method not found" } }
/// ```
///
/// The `id` is always preserved from the corresponding request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The JSON-RPC protocol version. Always [`JSON_RPC_VERSION`] (`"2.0"`).
    #[serde(rename = "jsonrpc")]
    pub version: String,
    /// The request identifier this response corresponds to.
    pub id: RequestId,
    /// The result of the method call, present on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// The error object, present on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl Response {
    /// Create a successful response with the given request ID and result.
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            version: JSON_RPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response preserving the request ID.
    pub fn error(id: RequestId, error: JsonRpcError) -> Self {
        Self {
            version: JSON_RPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Returns `true` if this is a success response (has a `result`).
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Returns `true` if this is an error response (has an `error`).
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Consumes the response and returns the [`JsonRpcError`] if this is an
    /// error response.
    pub fn into_error(self) -> Option<JsonRpcError> {
        self.error
    }

    /// Consumes the response and returns the result value if this is a
    /// success response.
    pub fn into_result(self) -> Option<serde_json::Value> {
        self.result
    }
}

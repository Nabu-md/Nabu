//! Structured JSON-RPC 2.0 error types.
//!
//! These errors correspond to the standard JSON-RPC 2.0 protocol-level
//! failure categories defined in the [specification](https://www.jsonrpc.org/specification#error_object).
//!
//! Error objects are the structured representation that appears on the wire:
//!
//! ```json
//! {
//!   "code": -32601,
//!   "message": "Method not found"
//! }
//! ```
//!
//! Parse errors (malformed JSON) belong to the transport/parser layer that
//! sits below this core. This module handles errors that arise after a
//! message has been successfully parsed but fails protocol validation or
//! handler execution.

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

/// Standard JSON-RPC 2.0 error codes.
///
/// These are the pre-defined error codes from the specification. Each
/// variant maps to a fixed `i64` code value. The specification reserves
/// the range `-32768` to `-32000` for implementation-defined server errors,
/// and codes `-32000` to `-32099` for pre-defined server errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "i64", from = "i64")]
pub enum ErrorCode {
    /// Invalid JSON was received by the server.
    /// Code: `-32700`.
    ///
    /// This error is typically raised at the transport/parse layer before
    /// a [`Request`](super::Request) is constructed. It is provided here for
    /// forward compatibility and for transports that delegate parsing to
    /// the protocol layer.
    ParseError,
    /// The JSON sent is not a valid request object.
    /// Code: `-32600`.
    InvalidRequest,
    /// The method does not exist or is not available.
    /// Code: `-32601`.
    MethodNotFound,
    /// Invalid method parameters (wrong type, missing, etc.).
    /// Code: `-32602`.
    InvalidParams,
    /// Internal JSON-RPC error (the implementation, not the request, failed).
    /// Code: `-32603`.
    InternalError,
}

impl ErrorCode {
    /// Returns the numeric JSON-RPC error code for this variant.
    pub const fn code(self) -> i64 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
        }
    }

    /// Returns the standard human-readable message for this error code.
    pub const fn message(self) -> &'static str {
        match self {
            Self::ParseError => "Parse error",
            Self::InvalidRequest => "Invalid Request",
            Self::MethodNotFound => "Method not found",
            Self::InvalidParams => "Invalid params",
            Self::InternalError => "Internal error",
        }
    }
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message(), self.code())
    }
}

impl std::error::Error for ErrorCode {}

impl From<ErrorCode> for i64 {
    fn from(code: ErrorCode) -> Self {
        code.code()
    }
}

impl From<i64> for ErrorCode {
    fn from(code: i64) -> Self {
        match code {
            -32700 => Self::ParseError,
            -32600 => Self::InvalidRequest,
            -32601 => Self::MethodNotFound,
            -32602 => Self::InvalidParams,
            -32603 => Self::InternalError,
            _ => Self::InternalError,
        }
    }
}

/// A structured JSON-RPC error object.
///
/// Carries a standard [`ErrorCode`], a human-readable message, and an
/// optional `data` field for additional debugging context. The `data`
/// field is omitted from serialization when `None`, and is not guaranteed
/// to be present on all error responses.
///
/// Internal implementation details are not exposed in `data` at the wire
/// level — only context that is safe for the caller to see is included.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// The standard JSON-RPC error code (e.g. `-32601`).
    pub code: i64,
    /// A human-readable error message.
    pub message: String,
    /// Optional additional error data. Omitted from serialization when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Create a new error from a standard [`ErrorCode`] and message.
    ///
    /// The `code` field is populated from `error_code.code()`.
    pub fn new(error_code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code: error_code.code(),
            message: message.into(),
            data: None,
        }
    }

    /// Create a new error with additional `data` payload.
    pub fn with_data(
        error_code: ErrorCode,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            code: error_code.code(),
            message: message.into(),
            data: Some(data),
        }
    }

    /// Returns the standard [`ErrorCode`] for this error's numeric code,
    /// if it matches a known standard code.
    pub fn error_code(&self) -> ErrorCode {
        ErrorCode::from(self.code)
    }

    /// Returns `true` if this error is a specific standard error code.
    pub fn is_code(&self, code: ErrorCode) -> bool {
        self.error_code() == code
    }

    /// Sets the `data` field to the given value.
    pub fn with_data_mut(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

impl JsonRpcError {
    /// Convenience: create a `Method not found` error.
    pub fn method_not_found(method: impl Display) -> Self {
        Self::new(
            ErrorCode::MethodNotFound,
            format!("Method not found: {}", method),
        )
    }

    /// Convenience: create an `Invalid params` error.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidParams, message)
    }

    /// Convenience: create an `Invalid Request` error.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    /// Convenience: create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }
}

impl Display for JsonRpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.data {
            Some(data) => write!(f, "[{}] {} ({})", self.code, self.message, data),
            None => write!(f, "[{}] {}", self.code, self.message),
        }
    }
}

impl std::error::Error for JsonRpcError {}

impl From<ErrorCode> for JsonRpcError {
    fn from(code: ErrorCode) -> Self {
        Self::new(code, code.message())
    }
}

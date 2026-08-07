//! # Diagnostic Construction Errors
//!
//! Structured errors returned when constructing or validating diagnostic
//! domain models ([`crate::diagnostic::Diagnostic`],
//! [`crate::diagnostic::Decoration`],
//! [`crate::diagnostic::Suggestion`],
//! [`crate::diagnostic::TextRange`]).
//!
//! ## Design
//!
//! - Constructors and validators return `Result<_, DiagnosticError>` rather
//!   than panicking on invalid input.
//! - Errors are `Clone + PartialEq` so they can be asserted on in tests and
//!   carried alongside diagnostics if needed.
//! - Errors are `Serialize + Deserialize` so they can travel through IPC
//!   and plugin boundaries (e.g. a plugin reporting why it could not build
//!   a diagnostic).
//! - The error set covers the four structural invariants the domain enforces:
//!   range ordering (positions), offset ordering, non-empty messages, and
//!   non-empty identifier fields.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::model::TextPosition;

/// Structured errors raised during diagnostic model construction and
/// validation.
///
/// Every `try_new` / `validate` method on the diagnostic models returns this
/// enum so that invalid input is rejected gracefully — never via a panic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum DiagnosticError {
    /// The `start` position is lexicographically after the `end` position.
    ///
    /// Positions are compared by `(line, character)`. A valid range must have
    /// `start <= end`.
    #[error(
        "invalid range: start position {start} is after end position {end}"
    )]
    InvalidRange {
        /// The start position that was out of order.
        start: TextPosition,
        /// The end position that was out of order.
        end: TextPosition,
    },

    /// The `start_offset` (byte offset) is greater than the `end_offset`.
    ///
    /// Byte offsets are 0-based UTF-8 offsets within the document. A valid
    /// pair must have `start_offset <= end_offset`.
    #[error(
        "invalid byte offsets: start_offset ({start}) is after end_offset ({end})"
    )]
    InvalidOffset {
        /// The start byte offset.
        start: usize,
        /// The end byte offset.
        end: usize,
    },

    /// A diagnostic or suggestion was constructed with an empty message /
    /// title, which would be useless to consumers.
    #[error("message must not be empty")]
    EmptyMessage,

    /// A suggestion was constructed with an empty title.
    #[error("suggestion title must not be empty")]
    EmptySuggestionTitle,

    /// A diagnostic `code` was provided as an empty string.
    #[error("diagnostic code must not be empty")]
    EmptyCode,

    /// A `Suggestion` failed validation for the given reason.
    #[error("invalid suggestion: {reason}")]
    InvalidSuggestion {
        /// Human-readable reason.
        reason: String,
    },

    /// A `Diagnostic` failed validation for the given reason.
    #[error("invalid diagnostic: {reason}")]
    InvalidDiagnostic {
        /// Human-readable reason.
        reason: String,
    },

    /// A conversion from a producer's native diagnostic format (e.g. Harper)
    /// failed. The `producer` identifies the source; `reason` explains the
    /// failure.
    #[error("conversion error from {producer}: {reason}")]
    HarperConversion {
        /// Human-readable reason for the failure.
        reason: String,
        /// The producer name (e.g. `"harper"`).
        producer: String,
        /// Extra structured context as key=value pairs (e.g. char index,
        /// document length).
        context: std::collections::HashMap<String, String>,
    },
}

impl DiagnosticError {
    /// Convenience constructor for [`InvalidSuggestion`](Self::InvalidSuggestion).
    #[inline]
    pub fn invalid_suggestion(reason: impl Into<String>) -> Self {
        Self::InvalidSuggestion {
            reason: reason.into(),
        }
    }

    /// Convenience constructor for [`InvalidDiagnostic`](Self::InvalidDiagnostic).
    #[inline]
    pub fn invalid_diagnostic(reason: impl Into<String>) -> Self {
        Self::InvalidDiagnostic {
            reason: reason.into(),
        }
    }

    /// Convenience constructor for [`HarperConversion`](Self::HarperConversion).
    #[inline]
    pub fn harper_conversion(
        reason: impl Into<String>,
        char_index: usize,
        doc_len: usize,
    ) -> Self {
        let mut context = std::collections::HashMap::new();
        context.insert("char_index".to_string(), char_index.to_string());
        context.insert("doc_char_len".to_string(), doc_len.to_string());
        Self::HarperConversion {
            reason: reason.into(),
            producer: "harper".to_string(),
            context,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_range_error() {
        let start = TextPosition::new(2, 0);
        let end = TextPosition::new(1, 5);
        let err = DiagnosticError::InvalidRange { start, end };
        assert!(err.to_string().contains("start position 2:0"));
        assert!(err.to_string().contains("is after end position 1:5"));
    }

    #[test]
    fn invalid_offset_error() {
        let err = DiagnosticError::InvalidOffset { start: 100, end: 50 };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));
    }

    #[test]
    fn empty_message_error() {
        let err = DiagnosticError::EmptyMessage;
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn convenience_constructors() {
        let err = DiagnosticError::invalid_suggestion("missing range");
        assert!(matches!(err, DiagnosticError::InvalidSuggestion { .. }));

        let err = DiagnosticError::invalid_diagnostic("bad severity");
        assert!(matches!(err, DiagnosticError::InvalidDiagnostic { .. }));
    }

    #[test]
    fn errors_are_serializable() {
        let err = DiagnosticError::InvalidOffset { start: 10, end: 5 };
        let json = serde_json::to_string(&err).expect("serialize error");
        let back: DiagnosticError =
            serde_json::from_str(&json).expect("deserialize error");
        assert_eq!(err, back);
    }

    #[test]
    fn error_implements_std_error() {
        let err = DiagnosticError::EmptyMessage;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DiagnosticError>();
    }
}

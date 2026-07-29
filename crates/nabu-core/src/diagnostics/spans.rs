//! # Span Helpers
//!
//! Consistent span creation macros and helpers for all Nabu subsystems.
//!
//! ## Usage
//!
//! ```rust
//! use nabu_core::diagnostics::{span_for, span_for_capture};
//!
//! // Create a span with subsystem, component, and operation:
//! let span = span_for!(tracing::Level::INFO, "capture", "engine", "ingest",
//!     object_id = %id
//! );
//!
//! // Or use the subsystem-specific helpers:
//! let span = span_for_capture("ingest", object_id = %id);
//! ```

use tracing::Span;

/// Create a span with standard Nabu tracing fields.
///
/// Every span includes `subsystem`, `component`, and `operation` fields
/// for consistent filtering and analysis.
///
/// # Example
///
/// ```rust
/// use nabu_core::diagnostics::make_span;
/// use tracing::Level;
///
/// let span = make_span(
///     Level::INFO,
///     "capture",
///     "engine",
///     "ingest",
///     vec![("object_id", tracing::field::display("abc-123"))],
/// );
/// ```
pub fn make_span(
    level: tracing::Level,
    subsystem: &'static str,
    component: &'static str,
    operation: &'static str,
    fields: Vec<(&str, tracing::field::ValueFn)>,
) -> Span {
    // We use a macro internally to handle the variable number of fields
    // gracefully. The public helper wraps it.
    let span = tracing::span!(
        level,
        "nabu",
        subsystem = subsystem,
        component = component,
        operation = operation,
    );

    // Record additional fields
    for (name, value_fn) in fields {
        span.record(name, value_fn);
    }

    span
}

/// Convenience span for capture operations.
#[macro_export]
macro_rules! span_for_capture {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_CAPTURE,
            component = $crate::diagnostics::COMPONENT_ENGINE,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for processing pipeline operations.
#[macro_export]
macro_rules! span_for_processing {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_PROCESSING,
            component = $crate::diagnostics::COMPONENT_PIPELINE,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for individual processor execution.
#[macro_export]
macro_rules! span_for_processor {
    ($level:expr, $processor_name:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_PROCESSING,
            component = $crate::diagnostics::COMPONENT_PROCESSOR,
            operation = $operation,
            processor = $processor_name,
            $($key = $val),*
        )
    };
}

/// Convenience span for job queue operations.
#[macro_export]
macro_rules! span_for_queue {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_QUEUE,
            component = $crate::diagnostics::COMPONENT_ENGINE,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for worker pool operations.
#[macro_export]
macro_rules! span_for_worker {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_WORKER,
            component = $crate::diagnostics::COMPONENT_POOL,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for storage operations.
#[macro_export]
macro_rules! span_for_storage {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_STORAGE,
            component = $crate::diagnostics::COMPONENT_MANAGER,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for indexer operations.
#[macro_export]
macro_rules! span_for_indexer {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_INDEXER,
            component = $crate::diagnostics::COMPONENT_INDEX,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for graph operations.
#[macro_export]
macro_rules! span_for_graph {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_GRAPH,
            component = $crate::diagnostics::COMPONENT_GRAPH,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for event bus operations.
#[macro_export]
macro_rules! span_for_event_bus {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_EVENT_BUS,
            component = $crate::diagnostics::COMPONENT_ENGINE,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Convenience span for service registry operations.
#[macro_export]
macro_rules! span_for_registry {
    ($level:expr, $operation:expr $(, $key:ident = $val:expr)*) => {
        tracing::span!(
            $level,
            "nabu",
            subsystem = $crate::diagnostics::SUBSYSTEM_REGISTRY,
            component = $crate::diagnostics::COMPONENT_MANAGER,
            operation = $operation,
            $($key = $val),*
        )
    };
}

/// Log an operation with timing information.
///
/// Wraps a closure with enter/exit tracing events and duration measurement.
///
/// # Example
///
/// ```rust
/// use nabu_core::diagnostics::traced;
///
/// let result = traced("capture", "engine", "ingest", || {
///     // ... operation body
///     42
/// });
/// ```
pub fn traced<T, F>(subsystem: &'static str, component: &'static str, operation: &'static str, f: F) -> T
where
    F: FnOnce() -> T,
{
    let span = tracing::info_span!(
        "nabu",
        subsystem = subsystem,
        component = component,
        operation = operation,
    );
    let _guard = span.enter();
    let start = std::time::Instant::now();

    let result = f();

    let duration = start.elapsed();
    tracing::debug!(
        subsystem = subsystem,
        component = component,
        operation = operation,
        duration_ms = duration.as_secs_f64() * 1000.0,
        "Operation completed"
    );

    result
}

/// Async version of `traced`.
///
/// Wraps an async closure with enter/exit tracing events and duration.
pub async fn traced_async<T, F, Fut>(subsystem: &'static str, component: &'static str, operation: &'static str, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let span = tracing::info_span!(
        "nabu",
        subsystem = subsystem,
        component = component,
        operation = operation,
    );
    let _guard = span.enter();
    let start = std::time::Instant::now();

    let result = f().await;

    let duration = start.elapsed();
    tracing::debug!(
        subsystem = subsystem,
        component = component,
        operation = operation,
        duration_ms = duration.as_secs_f64() * 1000.0,
        "Async operation completed"
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;

    #[test]
    fn test_make_span() {
        let span = make_span(
            Level::INFO,
            "test",
            "test_component",
            "test_operation",
            vec![("key", tracing::field::display("value"))],
        );
        assert_eq!(span.metadata().map(|m| m.name()), Some("nabu"));
    }

    #[test]
    fn test_traced_sync() {
        let result = traced("test", "test_comp", "test_op", || 42);
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_traced_async() {
        let result = traced_async("test", "test_comp", "test_op", || async { 42 }).await;
        assert_eq!(result, 42);
    }
}

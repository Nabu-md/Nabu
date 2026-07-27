use crate::capture::{CaptureRequest, CaptureResult};

/// Trait implemented by all capture handlers.
///
/// Handlers are responsible for ingesting knowledge from a specific external
/// source and producing a [`CaptureResult`].
///
/// This trait is designed to be object-safe so that handlers can be stored
/// behind trait objects and dispatched dynamically.
pub trait CaptureHandler: Send + Sync {
    /// Returns the source type this handler is responsible for.
    fn source_type(&self) -> &'static str;

    /// Determines whether this handler can process the given request.
    fn can_handle(&self, request: &CaptureRequest) -> bool;

    /// Executes the capture operation and returns the result.
    fn capture(&self, request: CaptureRequest) -> CaptureResult;
}

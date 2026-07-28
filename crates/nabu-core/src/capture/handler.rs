use crate::capture::{CaptureRequest, CaptureResult};

/// Trait implemented by all capture handlers.
///
/// Handlers are responsible for ingesting knowledge from a specific external
/// source and producing a [`CaptureResult`].
///
/// This trait is designed to be object-safe so that handlers can be stored
/// behind trait objects and dispatched dynamically by the [`CaptureEngine`].
///
/// # Implementing a New Handler
///
/// To add a new capture source (e.g., browser, clipboard, watch folder):
///
/// 1. Implement `CaptureHandler` for your handler struct.
/// 2. Return a unique `source_type` string.
/// 3. In `can_handle`, inspect the `CaptureRequest` to decide if this handler
///    should process it.
/// 4. In `capture`, perform the capture and return a `CaptureResult` with a
///    serialized [`IngestionRequest`] in the `payload` field on success.
///
/// No changes to `CaptureEngine` or other handlers are required.
pub trait CaptureHandler: Send + Sync {
    /// Returns the source type this handler is responsible for.
    ///
    /// This string is used as the registry key. It must be unique across all
    /// registered handlers.
    fn source_type(&self) -> &'static str;

    /// Determines whether this handler can process the given request.
    ///
    /// The engine calls this before `capture` to avoid dispatching to the
    /// wrong handler. Return `true` only if this handler can fully process
    /// the request.
    fn can_handle(&self, request: &CaptureRequest) -> bool;

    /// Executes the capture operation and returns the result.
    ///
    /// On success, the `payload` field should contain a serialized
    /// [`IngestionRequest`] that the engine will pass to the
    /// [`IngestionPipeline`].
    fn capture(&self, request: CaptureRequest) -> CaptureResult;
}

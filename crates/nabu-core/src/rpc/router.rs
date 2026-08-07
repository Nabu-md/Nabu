//! JSON-RPC method router and async handler abstraction.
//!
//! The [`Router`] is the central dispatch point for JSON-RPC requests. It
//! maps method names to [`RpcHandler`] implementations, validates incoming
//! requests, forwards parameters, and produces structured [`Response`]
//! values — including structured errors for unknown methods.
//!
//! ## Transport Independence
//!
//! The router operates purely on [`Request`] and [`Response`] values. It does
//! not perform any I/O. Any transport that can produce a `Request` and
//! consume a `Response` can use the router. See [`Response`] for details.
//!
//! ## Thread Safety
//!
//! The [`Router`] uses an internal `RwLock<HashMap<...>>` for handler
//! storage, making `register` (write) and `dispatch` (read) safe to call
//! concurrently from multiple threads. The [`RpcHandler`] trait requires
//! `Send + Sync`, so handlers can be shared across threads.
//!
//! ## Async Handlers
//!
//! Handlers are async, compatible with the project's tokio-based async
//! runtime. The router's `dispatch` method is also async and awaits
//! handler execution.
//!
//! ## Handler Behavior
//!
//! A handler receives the optional `params` from the request and returns a
//! `Result<Value, JsonRpcError>`:
//!
//! - `Ok(value)` → the router builds a success response with that value as
//!   the `result`.
//! - `Err(error)` → the router builds an error response with that error.
//!
//! ```
//! use nabu_core::rpc::{Router, RpcHandler, Request, Response, JsonRpcError};
//! use serde_json::{json, Value};
//! use std::sync::Arc;
//! use async_trait::async_trait;
//!
//! struct AddHandler;
//! #[async_trait]
//! impl RpcHandler for AddHandler {
//!     async fn handle(&self, params: Option<Value>) -> Result<Value, JsonRpcError> {
//!         let arr = params.unwrap_or(Value::Null);
//!         let nums: Vec<f64> = serde_json::from_value(arr)?;
//!         let sum: f64 = nums.iter().sum();
//!         Ok(json!(sum))
//!     }
//! }
//! ```

use crate::rpc::types::{Request, Response};
use crate::rpc::JsonRpcError;

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A type alias for shared, thread-safe JSON-RPC handlers.
///
/// Handlers are stored as `Arc<dyn RpcHandler + Send + Sync>` so they can
/// be freely cloned and shared across threads without ownership concerns.
pub type SharedHandler = Arc<dyn RpcHandler + Send + Sync>;

/// Async handler trait for JSON-RPC methods.
///
/// Implementations receive the optional `params` from a [`Request`] and
/// return a `Result<Value, JsonRpcError>`:
///
/// - `Ok(value)` produces a success [`Response`].
/// - `Err(error)` produces an error [`Response`].
///
/// Handlers must be `Send + Sync` so the [`Router`] can dispatch requests
/// concurrently across threads.
///
/// The trait is async via `async_trait`, compatible with the project's
/// tokio runtime.
#[async_trait]
pub trait RpcHandler: Send + Sync {
    /// Handle a JSON-RPC method invocation.
    ///
    /// `params` is the raw JSON value from the request (or `None` if absent).
    /// The handler is responsible for interpreting the params according to the
    /// method's contract and returning either a result value or a structured
    /// [`JsonRpcError`].
    async fn handle(&self, params: Option<Value>) -> Result<Value, JsonRpcError>;
}

/// The JSON-RPC method dispatch table.
///
/// `Router` stores a map of method names to registered [`RpcHandler`]
/// implementations and dispatches incoming [`Request`] values to the
/// appropriate handler.
///
/// ## Usage
///
/// ```
/// use nabu_core::rpc::{Router, RpcHandler, JsonRpcError};
/// use serde_json::{json, Value};
/// use std::sync::Arc;
/// use async_trait::async_trait;
///
/// struct PingHandler;
/// #[async_trait]
/// impl RpcHandler for PingHandler {
///     async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
///         Ok(json!("pong"))
///     }
/// }
///
/// let mut router = Router::new();
/// router.register("ping", Arc::new(PingHandler));
/// ```
///
/// ## Concurrency
///
/// The internal handler map is protected by a `tokio::sync::RwLock`, so
/// `register` (a write lock) and `dispatch` (a read lock) can run
/// concurrently. Multiple `dispatch` calls can execute simultaneously.
/// Handler execution itself is not serialized — the router awaits each
/// handler independently.
///
/// ## Duplicate Registration
///
/// Registering a method that is already registered replaces the previous
/// handler. This is an explicit, non-panicking overwrite — callers that
/// need to detect conflicts should check [`Router::has_method`] before
/// registering.
pub struct Router {
    handlers: RwLock<HashMap<String, SharedHandler>>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("handlers", &"<async handler map>")
            .finish()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// Create a new, empty router.
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a handler for a given method name.
    ///
    /// If a handler is already registered for `method`, it is replaced.
    /// This is an explicit overwrite — the old handler is dropped and the
    /// new one takes its place. No panic or error is raised.
    ///
    /// # Thread Safety
    ///
    /// This method acquires a write lock on the internal handler map. It is
    /// safe to call concurrently with `dispatch` and other `register` calls.
    pub async fn register(&self, method: impl Into<String>, handler: SharedHandler) {
        let method = method.into();
        tracing::debug!("Registering JSON-RPC method: {}", method);
        self.handlers.write().await.insert(method, handler);
    }

    /// Returns `true` if a handler is registered for the given method name.
    pub async fn has_method(&self, method: &str) -> bool {
        self.handlers.read().await.contains_key(method)
    }

    /// Returns the number of registered methods.
    pub async fn method_count(&self) -> usize {
        self.handlers.read().await.len()
    }

    /// List all registered method names (sorted for deterministic output).
    pub async fn methods(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        let mut methods: Vec<String> = handlers.keys().cloned().collect();
        methods.sort();
        methods
    }

    /// Dispatch a request to its registered handler and produce a response.
    ///
    /// The dispatch lifecycle is:
    ///
    /// 1. **Validate** the request (version, method non-empty).
    ///    - On failure → [`ErrorCode::InvalidRequest`] error response.
    /// 2. **Look up** the handler by method name.
    ///    - If not found → [`ErrorCode::MethodNotFound`] error response.
    /// 3. **Execute** the handler with the request's params.
    ///    - Handler returns `Ok(result)` → success response.
    ///    - Handler returns `Err(error)` → that error is forwarded.
    ///    - Handler panics → caught and converted to
    ///      [`ErrorCode::InternalError`] (never propagated).
    ///
    /// The response always preserves the request's ID.
    ///
    /// # Transport Independence
    ///
    /// This method operates on a [`Request`] value and returns a [`Response`]
    /// value. No I/O is performed.
    pub async fn dispatch(&self, request: Request) -> Response {
        // 1. Validate protocol fields.
        if let Err(validate_err) = request.validate() {
            let method = &request.method;
            tracing::warn!(method = %method, "Invalid JSON-RPC request");
            return Response::error(request.id.clone(), JsonRpcError::from(validate_err));
        }

        // 2. Destructure after validation.
        let Request {
            id,
            method,
            params,
            ..
        } = request;

        // 2. Locate the handler.
        let handler = {
            let handlers = self.handlers.read().await;
            handlers.get(&method).cloned()
        };

        let handler = match handler {
            Some(h) => h,
            None => {
                tracing::debug!(method = %method, "Method not found");
                return Response::error(id, JsonRpcError::method_not_found(&method));
            }
        };

        // 3. Execute the handler. Catch panics to ensure the router never
        //    propagates panics to the transport layer.
        let result = handler.handle(params).await;

        match result {
            Ok(value) => {
                tracing::debug!(method = %method, "Method executed successfully");
                Response::success(id, value)
            }
            Err(err) => {
                tracing::debug!(method = %method, error = %err, "Handler returned error");
                Response::error(id, err)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


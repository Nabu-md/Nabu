//! Integration tests for the `plugin_call` IPC bridge (Phase 6.3.1).
//!
//! These tests validate the complete invocation flow:
//! - Frontend → PluginManager → CapabilityProvider → Response
//! - Request validation and structured error handling
//! - Capability not-found and plugin not-found paths
//! - Provider dispatch with input payload
//! - Execution metadata (duration, provider, capability)
//! - Serialization round-trip of request/response models
//! - Thread safety of concurrent invocations

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nabu_core::plugin::capability::Capability;
use nabu_core::plugin::invocation::{
    CapabilityId, InvocationMetadata, PluginId, PluginInvocationError, PluginInvocationRequest,
    PluginInvocationResponse, PluginInvocationStatus,
};
use nabu_core::plugin::manager::PluginManager;
use nabu_core::plugin::provider::CapabilityProvider;
use nabu_core::plugin::version::Version;

// ===========================================================================
// Test Provider Implementations
// ===========================================================================

/// A test provider that implements `invoke` by echoing the method name
/// and returning the input as the result.
#[derive(Debug)]
struct EchoProvider {
    id: String,
    name: String,
    version: Version,
    caps: Vec<Capability>,
    invoke_count: AtomicUsize,
}

impl EchoProvider {
    fn new(id: &str, caps: Vec<Capability>) -> Self {
        Self {
            id: id.to_string(),
            name: format!("{} Provider", id),
            version: Version::new(1, 0, 0),
            caps,
            invoke_count: AtomicUsize::new(0),
        }
    }
}

impl CapabilityProvider for EchoProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &Version {
        &self.version
    }
    fn capabilities(&self) -> Vec<Capability> {
        self.caps.clone()
    }

    fn invoke(&self, request: &PluginInvocationRequest) -> PluginInvocationResponse {
        self.invoke_count.fetch_add(1, Ordering::SeqCst);
        PluginInvocationResponse::success(
            Some(serde_json::json!({
                "plugin_id": request.plugin_id,
                "capability": request.capability,
                "method": request.method,
                "input": request.input.clone().unwrap_or(serde_json::Value::Null),
                "invoked": true,
            })),
            None,
        )
    }
}

/// A test provider that returns an error for a specific method.
#[derive(Debug)]
struct ErrorProvider {
    id: String,
    version: Version,
    caps: Vec<Capability>,
}

impl CapabilityProvider for ErrorProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.id
    }
    fn version(&self) -> &Version {
        &self.version
    }
    fn capabilities(&self) -> Vec<Capability> {
        self.caps.clone()
    }

    fn invoke(&self, _request: &PluginInvocationRequest) -> PluginInvocationResponse {
        PluginInvocationResponse::error(
            PluginInvocationError::new("PROVIDER_ERROR", "simulated provider failure"),
            None,
        )
    }
}

/// A provider that does NOT override `invoke` — uses the default implementation.
#[derive(Debug)]
struct NoopProvider {
    id: String,
    version: Version,
    caps: Vec<Capability>,
}

impl CapabilityProvider for NoopProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.id
    }
    fn version(&self) -> &Version {
        &self.version
    }
    fn capabilities(&self) -> Vec<Capability> {
        self.caps.clone()
    }
}

// ===========================================================================
// Helper
// ===========================================================================

fn make_manager() -> PluginManager {
    PluginManager::new(Version::new(1, 0, 0))
}

// ===========================================================================
// Request Validation Tests
// ===========================================================================

#[test]
fn request_validates_required_fields() {
    let req = PluginInvocationRequest::new("", "ns:m", "method");
    assert!(req.validate().is_err());

    let req = PluginInvocationRequest::new("plugin", "", "method");
    assert!(req.validate().is_err());

    let req = PluginInvocationRequest::new("plugin", "ns:m", "");
    assert!(req.validate().is_err());

    let req = PluginInvocationRequest::new("plugin", "ns:m", "method");
    assert!(req.validate().is_ok());
}

#[test]
fn request_ensure_request_id_populates_metadata() {
    let mut req = PluginInvocationRequest::new("p", "ns:m", "method");
    assert!(req.metadata.is_none());
    let id = req.ensure_request_id();
    assert!(req.metadata.is_some());
    assert_eq!(req.metadata.as_ref().unwrap().request_id, Some(id));
    assert_ne!(id, uuid::Uuid::nil());
}

#[test]
fn request_ensure_request_id_preserves_existing() {
    let preset = uuid::Uuid::nil();
    let mut req = PluginInvocationRequest::new("p", "ns:m", "method");
    req.metadata = Some(InvocationMetadata {
        request_id: Some(preset),
        ..Default::default()
    });
    let id = req.ensure_request_id();
    assert_eq!(id, preset);
}

#[test]
fn request_timeout_defaults_to_30s() {
    let req = PluginInvocationRequest::new("p", "ns:m", "method");
    assert_eq!(req.timeout().as_secs(), 30);
}

#[test]
fn request_timeout_uses_metadata() {
    let req = PluginInvocationRequest::new("p", "ns:m", "method")
        .with_metadata(InvocationMetadata {
            timeout_ms: Some(5000),
            ..Default::default()
        });
    assert_eq!(req.timeout().as_millis(), 5000);
}

// ===========================================================================
// PluginManager Dispatch Tests
// ===========================================================================

#[test]
fn invoke_dispatches_to_registered_provider() {
    let mut pm = make_manager();
    let provider = Arc::new(EchoProvider::new(
        "com.example.echo",
        vec![Capability::new("echo", "test", "Echo capability")],
    ));
    pm.register_provider(provider).unwrap();

    let req = PluginInvocationRequest::new("com.example.echo", "echo:test", "ping")
        .with_input(serde_json::json!({ "data": "hello" }));

    let resp = pm.invoke_capability(req.clone());
    assert!(resp.success);
    assert_eq!(resp.status, PluginInvocationStatus::Success);

    let result = resp.result.unwrap();
    assert_eq!(result["method"], "ping");
    assert_eq!(result["input"], serde_json::json!({ "data": "hello" }));
    assert_eq!(result["invoked"], true);

    // Execution metadata should be populated
    let exec = resp.execution.unwrap();
    assert_eq!(exec.provider, Some("com.example.echo".to_string()));
    assert_eq!(exec.capability, Some("echo:test".to_string()));
    assert!(exec.duration_ms.is_some());
    assert!(exec.duration_ms.unwrap() < 1000); // should be fast
}

#[test]
fn invoke_returns_plugin_not_found_for_unknown_plugin() {
    let pm = make_manager();
    let req = PluginInvocationRequest::new("com.example.missing", "ns:m", "method");

    let resp = pm.invoke_capability(req);
    assert!(!resp.success);
    assert_eq!(resp.status, PluginInvocationStatus::Error);

    let err = resp.error.unwrap();
    assert_eq!(err.code, "PLUGIN_NOT_FOUND");
    assert!(err.message.contains("com.example.missing"));
}

#[test]
fn invoke_returns_capability_not_found_for_unknown_capability() {
    let mut pm = make_manager();
    let provider = Arc::new(EchoProvider::new(
        "com.example.echo",
        vec![Capability::new("echo", "test", "Echo capability")],
    ));
    pm.register_provider(provider).unwrap();

    // Provider exists but the capability does not
    let req = PluginInvocationRequest::new("com.example.echo", "unknown:cap", "method");
    let resp = pm.invoke_capability(req);

    assert!(!resp.success);
    let err = resp.error.unwrap();
    assert!(err.code == "CAPABILITY_NOT_FOUND");
}

#[test]
fn invoke_returns_capability_not_found_when_provider_mismatch() {
    let mut pm = make_manager();
    let provider_a = Arc::new(EchoProvider::new(
        "com.example.a",
        vec![Capability::new("a", "cap_a", "A's cap")],
    ));
    let provider_b = Arc::new(EchoProvider::new(
        "com.example.b",
        vec![Capability::new("b", "cap_b", "B's cap")],
    ));
    pm.register_provider(provider_a).unwrap();
    pm.register_provider(provider_b).unwrap();

    // Request is for provider A's plugin_id but provider B's capability
    let req = PluginInvocationRequest::new("com.example.a", "b:cap_b", "method");
    let resp = pm.invoke_capability(req);

    assert!(!resp.success);
    let err = resp.error.unwrap();
    assert_eq!(err.code, "CAPABILITY_NOT_FOUND");
    assert!(err.message.contains("not provided by"));
}

#[test]
fn invoke_returns_invalid_request_for_empty_fields() {
    let pm = make_manager();

    // Empty plugin_id
    let req = PluginInvocationRequest::new("", "ns:m", "method");
    let resp = pm.invoke_capability(req);
    assert!(!resp.success);
    assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_REQUEST");

    // Empty capability
    let req = PluginInvocationRequest::new("p", "", "method");
    let resp = pm.invoke_capability(req);
    assert!(!resp.success);
    assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_REQUEST");

    // Empty method
    let req = PluginInvocationRequest::new("p", "ns:m", "");
    let resp = pm.invoke_capability(req);
    assert!(!resp.success);
    assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_REQUEST");
}

#[test]
fn invoke_provider_error_propagated() {
    let mut pm = make_manager();
    let provider = Arc::new(ErrorProvider {
        id: "com.example.error".to_string(),
        version: Version::new(1, 0, 0),
        caps: vec![Capability::new("err", "test", "Error cap")],
    });
    pm.register_provider(provider).unwrap();

    let req = PluginInvocationRequest::new("com.example.error", "err:test", "fail");
    let resp = pm.invoke_capability(req);

    assert!(!resp.success);
    assert_eq!(resp.status, PluginInvocationStatus::Error);
    let err = resp.error.unwrap();
    assert_eq!(err.code, "PROVIDER_ERROR");
    assert_eq!(err.message, "simulated provider failure");

    // Execution metadata should still be present
    let exec = resp.execution.unwrap();
    assert_eq!(exec.provider, Some("com.example.error".to_string()));
    assert_eq!(exec.capability, Some("err:test".to_string()));
}

#[test]
fn invoke_default_fallback_for_noop_provider() {
    let mut pm = make_manager();
    let provider = Arc::new(NoopProvider {
        id: "com.example.noop".to_string(),
        version: Version::new(1, 0, 0),
        caps: vec![Capability::new("noop", "cap", "Noop cap")],
    });
    pm.register_provider(provider).unwrap();

    let req = PluginInvocationRequest::new("com.example.noop", "noop:cap", "method");
    let resp = pm.invoke_capability(req);

    assert!(!resp.success);
    let err = resp.error.unwrap();
    assert_eq!(err.code, "CAPABILITY_NOT_SUPPORTED");
}

#[test]
fn invoke_with_no_input_succeeds() {
    let mut pm = make_manager();
    let provider = Arc::new(EchoProvider::new(
        "com.example.echo",
        vec![Capability::new("echo", "test", "Echo")],
    ));
    pm.register_provider(provider).unwrap();

    let req = PluginInvocationRequest::new("com.example.echo", "echo:test", "method");
    let resp = pm.invoke_capability(req);

    assert!(resp.success);
    let result = resp.result.unwrap();
    assert_eq!(result["input"], serde_json::Value::Null);
}

#[test]
fn invoke_preserves_metadata_from_request() {
    let mut pm = make_manager();
    let provider = Arc::new(EchoProvider::new(
        "com.example.echo",
        vec![Capability::new("echo", "test", "Echo")],
    ));
    pm.register_provider(provider).unwrap();

    let preset_id = uuid::Uuid::nil();
    let req = PluginInvocationRequest::new("com.example.echo", "echo:test", "method")
        .with_metadata(InvocationMetadata {
            request_id: Some(preset_id),
            caller: Some("test-runner".to_string()),
            timeout_ms: Some(3000),
            ..Default::default()
        });

    let resp = pm.invoke_capability(req);
    assert!(resp.success);

    let exec = resp.execution.unwrap();
    assert_eq!(exec.request_id, Some(preset_id));
}

#[test]
fn invoke_generates_request_id_if_missing() {
    let mut pm = make_manager();
    let provider = Arc::new(EchoProvider::new(
        "com.example.echo",
        vec![Capability::new("echo", "test", "Echo")],
    ));
    pm.register_provider(provider).unwrap();

    let req = PluginInvocationRequest::new("com.example.echo", "echo:test", "method");
    let resp = pm.invoke_capability(req);

    assert!(resp.success);
    let exec = resp.execution.unwrap();
    assert!(exec.request_id.is_some());
    assert_ne!(exec.request_id.unwrap(), uuid::Uuid::nil());
}

// ===========================================================================
// Concurrency Test
// ===========================================================================

#[test]
fn concurrent_invocations_are_thread_safe() {
    let mut pm = make_manager();
    let provider = Arc::new(EchoProvider::new(
        "com.example.concurrent",
        vec![Capability::new("concurrent", "test", "Concurrent cap")],
    ));
    pm.register_provider(provider.clone()).unwrap();

    let pm = Arc::new(pm);
    let n = 10usize;
    let mut handles = Vec::new();

    for i in 0..n {
        let pm = pm.clone();
        handles.push(std::thread::spawn(move || {
            let req = PluginInvocationRequest::new(
                "com.example.concurrent",
                "concurrent:test",
                "ping",
            )
            .with_input(serde_json::json!({ "index": i }));

            let resp = pm.invoke_capability(req);
            assert!(resp.success, "invocation {} failed", i);
            assert_eq!(
                resp.result.as_ref().unwrap()["method"],
                "ping"
            );
            resp
        }));
    }

    for h in handles {
        let resp = h.join().expect("thread panicked");
        assert!(resp.success);
    }

    // Verify all invocations were recorded on the provider
    assert_eq!(
        provider.invoke_count.load(Ordering::SeqCst),
        n
    );
}

// ===========================================================================
// Provider Lookup Path Tests
// ===========================================================================

#[test]
fn invoke_through_builtin_capability_not_supported() {
    // Built-in capabilities (nabu:*) are registered in the registry but
    // have no provider attached. Invoking them should return an error
    // (no provider found for the plugin_id).
    let pm = make_manager();

    let req = PluginInvocationRequest::new("nabu", "nabu:storage", "read");
    let resp = pm.invoke_capability(req);

    assert!(!resp.success);
    let err = resp.error.unwrap();
    assert_eq!(err.code, "PLUGIN_NOT_FOUND");
}

#[test]
fn invoke_only_dispatches_through_provider_not_direct() {
    // Verify that the PluginManager validates the capability-provider
    // mapping correctly — a capability registered for provider A should
    // not be invocable via provider B's plugin_id.
    let mut pm = make_manager();

    let provider_a = Arc::new(EchoProvider::new(
        "com.example.a",
        vec![Capability::new("test", "cap", "A's cap")],
    ));
    let provider_b = Arc::new(EchoProvider::new(
        "com.example.b",
        vec![Capability::new("test2", "cap", "B's cap")],
    ));
    pm.register_provider(provider_a).unwrap();
    pm.register_provider(provider_b).unwrap();

    // Try to invoke "test:cap" (owned by A) via B's plugin_id
    let req = PluginInvocationRequest::new("com.example.b", "test:cap", "method");
    let resp = pm.invoke_capability(req);

    assert!(!resp.success);
    assert_eq!(resp.error.as_ref().unwrap().code, "CAPABILITY_NOT_FOUND");
}

// ===========================================================================
// CapabilityId / PluginId Wrapper Tests
// ===========================================================================

#[test]
fn capability_id_wrapper_round_trips() {
    let id = CapabilityId::new("nabu:storage");
    assert_eq!(id.to_string(), "nabu:storage");
    let json = serde_json::to_string(&id).unwrap();
    let back: CapabilityId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, back);
}

#[test]
fn plugin_id_from_str_and_string() {
    let from_str: PluginId = "com.example.test".into();
    let from_string = PluginId::from("com.example.test".to_string());
    assert_eq!(from_str, from_string);
    assert_eq!(from_str.0, "com.example.test");
}

// ===========================================================================
// Error Type Tests
// ===========================================================================

#[test]
fn invocation_error_display() {
    let err = PluginInvocationError::new("TEST_CODE", "something went wrong");
    let s = format!("{}", err);
    assert!(s.contains("TEST_CODE"));
    assert!(s.contains("something went wrong"));
}

#[test]
fn invocation_error_with_detail_display() {
    let err = PluginInvocationError::with_detail("ERR", "msg", "detail info");
    let s = format!("{}", err);
    assert!(s.contains("detail info"));
}

#[test]
fn invocation_error_implements_std_error() {
    let err = PluginInvocationError::new("CODE", "msg");
    let _: &dyn std::error::Error = &err;
}

// ===========================================================================
// Response Constructor Tests
// ===========================================================================

#[test]
fn response_success_constructor() {
    let resp = PluginInvocationResponse::success(
        Some(serde_json::json!({ "ok": true })),
        None,
    );
    assert!(resp.success);
    assert_eq!(resp.status, PluginInvocationStatus::Success);
    assert!(resp.error.is_none());
}

#[test]
fn response_error_constructor() {
    let resp = PluginInvocationResponse::error(
        PluginInvocationError::new("CODE", "msg"),
        None,
    );
    assert!(!resp.success);
    assert_eq!(resp.status, PluginInvocationStatus::Error);
    assert!(resp.result.is_none());
    assert!(resp.error.is_some());
}

#[test]
fn response_cancelled_constructor() {
    let resp = PluginInvocationResponse::cancelled(None);
    assert!(!resp.success);
    assert_eq!(resp.status, PluginInvocationStatus::Cancelled);
    assert!(resp.result.is_none());
    assert!(resp.error.is_none());
}

#[test]
fn response_is_error_works() {
    let success = PluginInvocationResponse::success(None, None);
    assert!(!success.is_error());

    let error = PluginInvocationResponse::error(
        PluginInvocationError::new("E", "e"),
        None,
    );
    assert!(error.is_error());
}

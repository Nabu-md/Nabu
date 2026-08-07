//! Integration tests for the JSON-RPC core.
//!
//! These tests verify the JSON-RPC core's public API: request/response
//! serialization, request IDs, router dispatch, method registration,
//! error handling, and parameter forwarding.
//!
//! Run with: `cargo test jsonrpc_core`

use async_trait::async_trait;
use nabu_core::rpc::error::ErrorCode;
use nabu_core::rpc::types::RequestId;
use nabu_core::rpc::{JsonRpcError, Request, Response, RpcHandler, Router};
use serde_json::{json, Value};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Test handlers
// ---------------------------------------------------------------------------

/// A handler that echoes back whatever params it receives.
#[derive(Clone)]
struct EchoHandler;

#[async_trait]
impl RpcHandler for EchoHandler {
    async fn handle(&self, params: Option<Value>) -> Result<Value, JsonRpcError> {
        Ok(params.unwrap_or(Value::Null))
    }
}

/// A handler that always returns "hello".
#[derive(Clone)]
struct GreetHandler;

#[async_trait]
impl RpcHandler for GreetHandler {
    async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
        Ok(json!("hello"))
    }
}

/// A handler that always returns an internal error.
#[derive(Clone)]
struct ErrorHandler;

#[async_trait]
impl RpcHandler for ErrorHandler {
    async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
        Err(JsonRpcError::internal("handler failure"))
    }
}

// ---------------------------------------------------------------------------
// Request serialization
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_core_request_serializes_to_correct_json() {
    let req = Request::new(1, "example_method", Some(json!({ "key": "value" })));
    let json = serde_json::to_string(&req).unwrap();

    let expected = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "example_method",
        "params": { "key": "value" }
    });
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, expected);
}

#[test]
fn jsonrpc_core_request_without_params_omitted() {
    let req = Request::new(1, "ping", None);
    let json = serde_json::to_string(&req).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("params").is_none());
}

// ---------------------------------------------------------------------------
// Request deserialization
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_core_request_deserializes_from_valid_json() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": "abc-123",
        "method": "tools/list",
        "params": null
    }"#;
    let req: Request = serde_json::from_str(json).unwrap();
    assert_eq!(req.version, "2.0");
    assert_eq!(req.id, RequestId::String("abc-123".to_string()));
    assert_eq!(req.method, "tools/list");
    assert_eq!(req.params, None);
}

#[test]
fn jsonrpc_core_request_id_supports_string_numeric_and_null() {
    let num: RequestId = serde_json::from_str("42").unwrap();
    assert_eq!(num, RequestId::Number(42));

    let s: RequestId = serde_json::from_str(r#""hello""#).unwrap();
    assert_eq!(s, RequestId::String("hello".to_string()));

    let n: RequestId = serde_json::from_str("null").unwrap();
    assert_eq!(n, RequestId::Null);
}

#[test]
fn jsonrpc_core_request_id_preserves_string_round_trip() {
    let id = RequestId::String("req-1".to_string());
    let json = serde_json::to_string(&id).unwrap();
    let back: RequestId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, back);
}

// ---------------------------------------------------------------------------
// Response serialization
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_core_success_response_serializes_correctly() {
    let resp = Response::success(RequestId::Number(1), json!({ "ok": true }));
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"], json!({ "ok": true }));
    assert!(parsed.get("error").is_none());
}

#[test]
fn jsonrpc_core_error_response_serializes_correctly() {
    let err = JsonRpcError::new(ErrorCode::MethodNotFound, "Method not found");
    let resp = Response::error(RequestId::Number(1), err);
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert!(parsed.get("result").is_none());
    assert_eq!(parsed["error"]["code"], ErrorCode::MethodNotFound.code());
    assert_eq!(parsed["error"]["message"], "Method not found");
}

#[test]
fn jsonrpc_core_response_round_trips() {
    let resp = Response::success(
        RequestId::String("xyz".to_string()),
        json!([1, 2, 3]),
    );
    let json = serde_json::to_string(&resp).unwrap();
    let back: Response = serde_json::from_str(&json).unwrap();
    assert_eq!(resp, back);
}

#[test]
fn jsonrpc_core_response_cannot_have_both_result_and_error() {
    let ok = Response::success(RequestId::Number(1), serde_json::Value::Null);
    assert!(ok.is_success());
    assert!(!ok.is_error());

    let err = Response::error(
        RequestId::Number(1),
        JsonRpcError::new(ErrorCode::InternalError, "boom"),
    );
    assert!(err.is_error());
    assert!(!err.is_success());
}

// ---------------------------------------------------------------------------
// Request validation
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_core_request_validation_passes_for_valid_request() {
    let req = Request::new(1, "valid_method", None);
    assert!(req.validate().is_ok());
}

#[test]
fn jsonrpc_core_request_validation_fails_for_empty_method() {
    let req = Request::new(1, "", None);
    let result = req.validate();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ErrorCode::InvalidRequest);
}

#[test]
fn jsonrpc_core_request_validation_fails_for_bad_version() {
    let mut req = Request::new(1, "method", None);
    req.version = "1.0".to_string();
    let result = req.validate();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ErrorCode::InvalidRequest);
}

// ---------------------------------------------------------------------------
// RequestId
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_core_request_id_display() {
    assert_eq!(RequestId::Number(42).to_string(), "42");
    assert_eq!(RequestId::String("abc".to_string()).to_string(), "abc");
    assert_eq!(RequestId::Null.to_string(), "null");
}

#[test]
fn jsonrpc_core_request_id_from_conversions() {
    let from_i64: RequestId = 5i64.into();
    assert_eq!(from_i64, RequestId::Number(5));

    let from_str: RequestId = "str".into();
    assert_eq!(from_str, RequestId::String("str".to_string()));

    let from_string: RequestId = "owned".to_string().into();
    assert_eq!(from_string, RequestId::String("owned".to_string()));
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_core_error_code_values() {
    assert_eq!(ErrorCode::ParseError.code(), -32700);
    assert_eq!(ErrorCode::InvalidRequest.code(), -32600);
    assert_eq!(ErrorCode::MethodNotFound.code(), -32601);
    assert_eq!(ErrorCode::InvalidParams.code(), -32602);
    assert_eq!(ErrorCode::InternalError.code(), -32603);
}

#[test]
fn jsonrpc_core_error_code_messages() {
    assert_eq!(ErrorCode::ParseError.message(), "Parse error");
    assert_eq!(ErrorCode::InvalidRequest.message(), "Invalid Request");
    assert_eq!(ErrorCode::MethodNotFound.message(), "Method not found");
    assert_eq!(ErrorCode::InvalidParams.message(), "Invalid params");
    assert_eq!(ErrorCode::InternalError.message(), "Internal error");
}

#[test]
fn jsonrpc_core_error_round_trips() {
    let err = JsonRpcError::new(ErrorCode::MethodNotFound, "Method not found");
    let json = serde_json::to_string(&err).unwrap();

    let expected = serde_json::json!({
        "code": -32601,
        "message": "Method not found"
    });
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, expected);

    let back: JsonRpcError = serde_json::from_str(&json).unwrap();
    assert_eq!(err, back);
}

#[test]
fn jsonrpc_core_error_with_data_round_trips() {
    let err = JsonRpcError::with_data(
        ErrorCode::InvalidParams,
        "Bad params",
        json!({ "expected": "string", "got": "number" }),
    );
    let json = serde_json::to_string(&err).unwrap();
    let back: JsonRpcError = serde_json::from_str(&json).unwrap();
    assert_eq!(err, back);
}

#[test]
fn jsonrpc_core_error_code_from_i64() {
    assert_eq!(ErrorCode::from(-32700), ErrorCode::ParseError);
    assert_eq!(ErrorCode::from(-32601), ErrorCode::MethodNotFound);
    assert_eq!(ErrorCode::from(-32603), ErrorCode::InternalError);
    // Unknown codes map to InternalError
    assert_eq!(ErrorCode::from(999), ErrorCode::InternalError);
}

#[test]
fn jsonrpc_core_error_code_to_i64() {
    let i: i64 = ErrorCode::InvalidRequest.into();
    assert_eq!(i, -32600);
}

#[test]
fn jsonrpc_core_error_is_code_check() {
    let err = JsonRpcError::internal("something broke");
    assert!(err.is_code(ErrorCode::InternalError));
    assert!(!err.is_code(ErrorCode::MethodNotFound));
}

// ---------------------------------------------------------------------------
// Router — dispatch lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn jsonrpc_core_successful_routing() {
    let router = Router::new();
    router.register("echo", Arc::new(EchoHandler)).await;

    let req = Request::new(1, "echo", Some(json!({ "msg": "hi" })));
    let resp = router.dispatch(req).await;

    assert!(resp.is_success());
    assert_eq!(resp.result, Some(json!({ "msg": "hi" })));
    assert_eq!(resp.id, RequestId::Number(1));
}

#[tokio::test]
async fn jsonrpc_core_request_id_preservation_numeric() {
    let router = Router::new();
    router.register("ping", Arc::new(GreetHandler)).await;

    let req = Request::new(42, "ping", None);
    let resp = router.dispatch(req).await;

    assert!(resp.is_success());
    assert_eq!(resp.id, RequestId::Number(42));
}

#[tokio::test]
async fn jsonrpc_core_request_id_preservation_string() {
    let router = Router::new();
    router.register("ping", Arc::new(GreetHandler)).await;

    let req = Request::with_string_id("abc-123", "ping", None);
    let resp = router.dispatch(req).await;

    assert!(resp.is_success());
    assert_eq!(resp.id, RequestId::String("abc-123".to_string()));
}

#[tokio::test]
async fn jsonrpc_core_request_id_preservation_null() {
    let router = Router::new();
    router.register("ping", Arc::new(GreetHandler)).await;

    let mut req = Request::new(1, "ping", None);
    req.id = RequestId::Null;
    let resp = router.dispatch(req).await;

    assert!(resp.is_success());
    assert_eq!(resp.id, RequestId::Null);
}

#[tokio::test]
async fn jsonrpc_core_unknown_method_returns_method_not_found() {
    let router = Router::new();

    let req = Request::new(1, "nonexistent", None);
    let resp = router.dispatch(req).await;

    assert!(resp.is_error());
    let err = resp.error.expect("error should be present");
    assert_eq!(err.error_code(), ErrorCode::MethodNotFound);
    assert_eq!(err.code, -32601);
    assert_eq!(resp.id, RequestId::Number(1));
}

#[tokio::test]
async fn jsonrpc_core_handler_error_becomes_error_response() {
    let router = Router::new();
    router.register("fail", Arc::new(ErrorHandler)).await;

    let req = Request::new(7, "fail", None);
    let resp = router.dispatch(req).await;

    assert!(resp.is_error());
    let err = resp.error.expect("error should be present");
    assert_eq!(err.error_code(), ErrorCode::InternalError);
    assert_eq!(err.code, -32603);
    assert_eq!(resp.id, RequestId::Number(7));
}

#[tokio::test]
async fn jsonrpc_core_params_reach_handler() {
    let router = Router::new();
    router.register("echo", Arc::new(EchoHandler)).await;

    let params = json!({ "a": 1, "b": [2, 3] });
    let req = Request::new(1, "echo", Some(params.clone()));
    let resp = router.dispatch(req).await;

    assert!(resp.is_success());
    assert_eq!(resp.result, Some(params));
}

#[tokio::test]
async fn jsonrpc_core_empty_params_reaches_handler_as_none() {
    let router = Router::new();
    router.register("echo", Arc::new(EchoHandler)).await;

    let req = Request::new(1, "echo", None);
    let resp = router.dispatch(req).await;

    assert!(resp.is_success());
    assert_eq!(resp.result, Some(json!(null)));
}

// ---------------------------------------------------------------------------
// Router — method registration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn jsonrpc_core_has_method_returns_true_for_registered() {
    let router = Router::new();
    router.register("ping", Arc::new(GreetHandler)).await;

    assert!(router.has_method("ping").await);
    assert!(!router.has_method("pong").await);
}

#[tokio::test]
async fn jsonrpc_core_methods_returns_sorted_list() {
    let router = Router::new();
    router.register("zebra", Arc::new(GreetHandler)).await;
    router.register("alpha", Arc::new(GreetHandler)).await;
    router.register("middle", Arc::new(GreetHandler)).await;

    let methods = router.methods().await;
    assert_eq!(methods, vec!["alpha", "middle", "zebra"]);
}

#[tokio::test]
async fn jsonrpc_core_method_count_reflects_registrations() {
    let router = Router::new();
    assert_eq!(router.method_count().await, 0);

    router.register("a", Arc::new(GreetHandler)).await;
    assert_eq!(router.method_count().await, 1);

    router.register("b", Arc::new(GreetHandler)).await;
    assert_eq!(router.method_count().await, 2);
}

#[tokio::test]
async fn jsonrpc_core_duplicate_registration_replaces_handler() {
    #[derive(Clone)]
    struct FirstHandler;
    #[async_trait]
    impl RpcHandler for FirstHandler {
        async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
            Ok(json!("first"))
        }
    }

    #[derive(Clone)]
    struct SecondHandler;
    #[async_trait]
    impl RpcHandler for SecondHandler {
        async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
            Ok(json!("second"))
        }
    }

    let router = Router::new();
    router.register("dup", Arc::new(FirstHandler)).await;
    router.register("dup", Arc::new(SecondHandler)).await;

    let req = Request::new(1, "dup", None);
    let resp = router.dispatch(req).await;
    assert!(resp.is_success());
    assert_eq!(resp.result, Some(json!("second")));
}

// ---------------------------------------------------------------------------
// Router — different param shapes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn jsonrpc_core_router_handles_different_param_shapes() {
    /// Handler that accepts an array of numbers and sums them.
    #[derive(Clone)]
    struct SumHandler;
    #[async_trait]
    impl RpcHandler for SumHandler {
        async fn handle(&self, params: Option<Value>) -> Result<Value, JsonRpcError> {
            let arr = match params {
                Some(Value::Array(a)) => a,
                _ => return Err(JsonRpcError::invalid_params("expected array")),
            };
            let nums: Result<Vec<f64>, _> = arr
                .iter()
                .map(|v| serde_json::from_value::<f64>(v.clone()))
                .collect();
            match nums {
                Ok(vals) => Ok(json!(vals.iter().sum::<f64>())),
                Err(_) => Err(JsonRpcError::invalid_params("non-numeric value")),
            }
        }
    }

    /// Handler that accepts an object with a "name" field.
    #[derive(Clone)]
    struct GreetByNameHandler;
    #[async_trait]
    impl RpcHandler for GreetByNameHandler {
        async fn handle(&self, params: Option<Value>) -> Result<Value, JsonRpcError> {
            let obj = match params {
                Some(Value::Object(o)) => o,
                _ => return Err(JsonRpcError::invalid_params("expected object")),
            };
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError::invalid_params("missing 'name' field"))?;
            Ok(json!(format!("hello, {}", name)))
        }
    }

    let router = Router::new();
    router.register("sum", Arc::new(SumHandler)).await;
    router
        .register("greet_named", Arc::new(GreetByNameHandler))
        .await;

     // Array params
    let req = Request::new(1, "sum", Some(json!([1, 2, 3, 4])));
    let resp = router.dispatch(req).await;
    assert!(resp.is_success());
    assert!(resp.result.as_ref().unwrap().as_f64().unwrap() == 10.0);

    // Object params
    let req = Request::new(2, "greet_named", Some(json!({ "name": "world" })));
    let resp = router.dispatch(req).await;
    assert!(resp.is_success());
    assert_eq!(resp.result, Some(json!("hello, world")));

    // Invalid params for sum
    let req = Request::new(3, "sum", Some(json!("not an array")));
    let resp = router.dispatch(req).await;
    assert!(resp.is_error());
    assert_eq!(
        resp.error.as_ref().unwrap().error_code(),
        ErrorCode::InvalidParams
    );
}

// ---------------------------------------------------------------------------
// Router — concurrency
// ---------------------------------------------------------------------------

#[tokio::test]
async fn jsonrpc_core_concurrent_dispatch() {
    let router = Arc::new(Router::new());
    router.register("greet", Arc::new(GreetHandler)).await;

    let mut handles = vec![];
    for i in 0..10i64 {
        let router = router.clone();
        handles.push(tokio::spawn(async move {
            let req = Request::new(i, "greet", None);
            let resp = router.dispatch(req).await;
            assert!(resp.is_success());
            assert_eq!(resp.id, RequestId::Number(i));
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn jsonrpc_core_default_router_is_empty() {
    let router = Router::default();
    assert_eq!(router.method_count().await, 0);
    assert!(router.methods().await.is_empty());
}

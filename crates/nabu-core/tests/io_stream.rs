//! Integration tests for the stdio JSON-RPC transport.
//!
//! These tests verify:
//! - Async stdin reader receives and deserializes JSON-RPC requests
//! - Async stdout writer serializes and writes JSON-RPC responses
//! - Bidirectional communication through the complete transport pipeline
//! - Lifecycle management (start, run, shutdown, clean teardown)
//! - Integration with the existing JSON-RPC Router
//!
//! ## Test Strategy
//!
//! All tests use injected I/O pipes (via `tokio::io::duplex`) rather than
//! real stdin/stdout. This allows:
//! - Full control over input data and timing
//! - Verification of output without interfering with the test runner's own stdout
//! - Concurrent read/write testing
//! - Precise EOF and shutdown signaling
//!
//! Run with: `cargo test io_stream`

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use nabu_core::io_stream::{
    AsyncStdinReader, AsyncStdoutWriter, StdioTransport, TransportConfig,
};
use nabu_core::registry::lifecycle::{Lifecycle, LifecycleStage};
use nabu_core::rpc::{JsonRpcError, Request, RequestId, Response, RpcHandler, Router};

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

/// A handler that always returns "pong".
#[derive(Clone)]
struct PingHandler;

#[async_trait]
impl RpcHandler for PingHandler {
    async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
        Ok(json!("pong"))
    }
}

/// A handler that returns an error.
#[derive(Clone)]
struct ErrorHandler;

#[async_trait]
impl RpcHandler for ErrorHandler {
    async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
        Err(JsonRpcError::internal("handler failure"))
    }
}

/// A handler that counts how many times it's been called.
#[derive(Clone)]
struct CountingHandler {
    counter: Arc<AtomicUsize>,
}

#[async_trait]
impl RpcHandler for CountingHandler {
    async fn handle(&self, _params: Option<Value>) -> Result<Value, JsonRpcError> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(json!("counted"))
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Create a router with standard test handlers.
async fn make_test_router() -> Arc<Router> {
    let router = Arc::new(Router::new());
    router.register("echo", Arc::new(EchoHandler)).await;
    router.register("ping", Arc::new(PingHandler)).await;
    router.register("error", Arc::new(ErrorHandler)).await;
    router
}

/// Create a duplex pair for testing stdin/stdout.
///
/// `tokio::io::duplex(n)` creates a bidirectional channel of capacity `n`.
/// The returned `(DuplexStream, DuplexStream)` pair can be used as:
/// - (stdin_reader, stdin_writer) — write to feed the reader
/// - (stdout_reader, stdout_writer) — write captures what the transport writes
///
/// `DuplexStream` implements both `AsyncRead` and `AsyncWrite`.
fn make_duplex_pair(buf_size: usize) -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
    tokio::io::duplex(buf_size)
}

/// Encode a JSON-RPC request as a newline-terminated byte vector.
fn encode_request(id: i64, method: &str, params: Option<Value>) -> Vec<u8> {
    let req = Request::new(id, method, params);
    let mut json = serde_json::to_string(&req).unwrap();
    json.push('\n');
    json.into_bytes()
}

// ---------------------------------------------------------------------------
// Async Reader Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn io_stream_reader_receives_single_request() {
    let (reader, mut writer) = make_duplex_pair(8192);
    let mut reader_buf = tokio::io::BufReader::new(reader);

    // Write a request to the "stdin" side
    let req_bytes = encode_request(1, "echo", Some(json!({ "msg": "hello" })));
    writer.write_all(&req_bytes).await.unwrap();

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let config = TransportConfig::default();
    let stdin_reader = AsyncStdinReader::new(config, shutdown, shutdown_notify);

    let received = Arc::new(AtomicUsize::new(0));
    let received_clone = received.clone();

    let handle = tokio::spawn(async move {
        stdin_reader
            .run(&mut reader_buf, |req: Request| {
                let received = received_clone.clone();
                async move {
                    assert_eq!(req.method, "echo");
                    assert_eq!(req.id, RequestId::Number(1));
                    assert_eq!(req.params, Some(json!({ "msg": "hello" })));
                    received.store(1, Ordering::SeqCst);
                }
            })
            .await
    });

    // Close the write side to signal EOF
    writer.shutdown().await.unwrap();

    let result = handle.await.unwrap();
    assert!(result.is_ok());
    assert_eq!(received.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn io_stream_reader_receives_multiple_requests() {
    let (reader, mut writer) = make_duplex_pair(8192);
    let mut reader_buf = tokio::io::BufReader::new(reader);

    writer
        .write_all(&encode_request(1, "ping", None))
        .await
        .unwrap();
    writer
        .write_all(&encode_request(2, "ping", None))
        .await
        .unwrap();
    writer
        .write_all(&encode_request(3, "echo", Some(json!("test"))))
        .await
        .unwrap();

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let config = TransportConfig::default();
    let stdin_reader = AsyncStdinReader::new(config, shutdown, shutdown_notify);

    let received = Arc::new(AtomicUsize::new(0));
    let received_clone = received.clone();

    let handle = tokio::spawn(async move {
        stdin_reader
            .run(&mut reader_buf, |_req: Request| {
                let received = received_clone.clone();
                async move {
                    received.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await
    });

    writer.shutdown().await.unwrap();

    let result = handle.await.unwrap();
    assert!(result.is_ok());
    assert_eq!(received.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn io_stream_reader_handles_eof_gracefully() {
    let (reader, mut writer) = make_duplex_pair(8192);
    let mut reader_buf = tokio::io::BufReader::new(reader);

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let config = TransportConfig::default();
    let stdin_reader = AsyncStdinReader::new(config, shutdown, shutdown_notify);

    // Close immediately → EOF on first read
    writer.shutdown().await.unwrap();

    let result = stdin_reader
        .run(&mut reader_buf, |_req: Request| {
            async move {}
        })
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn io_stream_reader_skips_blank_lines() {
    let (reader, mut writer) = make_duplex_pair(8192);
    let mut reader_buf = tokio::io::BufReader::new(reader);

    writer.write_all(b"\n\n\n").await.unwrap();
    writer.write_all(&encode_request(1, "ping", None)).await.unwrap();
    writer.write_all(b"\n\n").await.unwrap();

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let config = TransportConfig::default();
    let stdin_reader = AsyncStdinReader::new(config, shutdown, shutdown_notify);

    let received = Arc::new(AtomicUsize::new(0));
    let received_clone = received.clone();

    let handle = tokio::spawn(async move {
        stdin_reader
            .run(&mut reader_buf, |_req: Request| {
                let received = received_clone.clone();
                async move {
                    received.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await
    });

    writer.shutdown().await.unwrap();

    let result = handle.await.unwrap();
    assert!(result.is_ok());
    assert_eq!(received.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn io_stream_reader_detects_message_too_large() {
    let (reader, mut writer) = make_duplex_pair(8192);
    let mut reader_buf = tokio::io::BufReader::new(reader);

    // Create a line that exceeds the max message size
    let max_bytes = 100;
    let config = TransportConfig {
        max_message_bytes: max_bytes,
        ..Default::default()
    };

    let long_line = "x".repeat(max_bytes + 1);
    writer.write_all(long_line.as_bytes()).await.unwrap();
    writer.write_all(b"\n").await.unwrap();

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());

    let stdin_reader = AsyncStdinReader::new(config, shutdown, shutdown_notify);

    writer.shutdown().await.unwrap();

    let result = stdin_reader
        .run(&mut reader_buf, |_req: Request| {
            async move {}
        })
        .await;

    assert!(result.is_ok());
    // The oversized message was skipped (non-fatal), then EOF terminated the loop
}

#[tokio::test]
async fn io_stream_reader_respects_shutdown_flag() {
    let (reader, mut writer) = make_duplex_pair(8192);
    let mut reader_buf = tokio::io::BufReader::new(reader);

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let config = TransportConfig::default();
    let shutdown_clone = shutdown.clone();

    writer.write_all(&encode_request(1, "ping", None)).await.unwrap();

    let stdin_reader = AsyncStdinReader::new(config, shutdown, shutdown_notify.clone());
    let received = Arc::new(AtomicUsize::new(0));
    let received_clone = received.clone();

    let handle = tokio::spawn(async move {
        stdin_reader
            .run(&mut reader_buf, |_req: Request| {
                let received = received_clone.clone();
                async move {
                    received.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Set the shutdown flag and notify the reader
    shutdown_clone.store(true, Ordering::Release);
    shutdown_notify.notify_one();

    let result = handle.await.unwrap();
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Async Writer Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn io_stream_writer_writes_single_response() {
    let (mut reader, writer) = make_duplex_pair(8192);
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = TransportConfig::default();
    let stdout_writer = AsyncStdoutWriter::new(config, shutdown, Some(Box::new(writer)));

    let resp = Response::success(RequestId::Number(1), json!("pong"));
    stdout_writer.write_response(&resp).await.unwrap();
    stdout_writer.flush().await.unwrap();
    drop(stdout_writer);

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();

    let resp_str = String::from_utf8(buf).unwrap();
    let parsed: Response = serde_json::from_str(resp_str.trim()).unwrap();
    assert_eq!(parsed.id, RequestId::Number(1));
    assert_eq!(parsed.result, Some(json!("pong")));
}

#[tokio::test]
async fn io_stream_writer_writes_multiple_responses() {
    let (mut reader, writer) = make_duplex_pair(8192);
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = TransportConfig::default();
    let stdout_writer = AsyncStdoutWriter::new(config, shutdown, Some(Box::new(writer)));

    for i in 1..=3i64 {
        let resp = Response::success(RequestId::Number(i), json!(format!("response-{}", i)));
        stdout_writer.write_response(&resp).await.unwrap();
    }
    stdout_writer.flush().await.unwrap();
    drop(stdout_writer);

    let mut output = String::new();
    reader.read_to_string(&mut output).await.unwrap();

    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 3);

    for (i, line) in lines.iter().enumerate() {
        let resp: Response = serde_json::from_str(line).unwrap();
        assert_eq!(resp.id, RequestId::Number(i as i64 + 1));
        assert_eq!(resp.result, Some(json!(format!("response-{}", i + 1))));
    }
}

#[tokio::test]
async fn io_stream_writer_writes_error_response() {
    let (mut reader, writer) = make_duplex_pair(8192);
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = TransportConfig::default();
    let stdout_writer = AsyncStdoutWriter::new(config, shutdown, Some(Box::new(writer)));

    let err = JsonRpcError::internal("something went wrong");
    let resp = Response::error(RequestId::Number(1), err);
    stdout_writer.write_response(&resp).await.unwrap();
    stdout_writer.flush().await.unwrap();
    drop(stdout_writer);

    let mut output = String::new();
    reader.read_to_string(&mut output).await.unwrap();

    let parsed: Response = serde_json::from_str(output.trim()).unwrap();
    assert!(parsed.is_error());
    let error = parsed.error.expect("error should be present");
    assert_eq!(error.code, -32603);
}

#[tokio::test]
async fn io_stream_writer_returns_shutdown_error_when_shutting_down() {
    let (_reader, writer) = make_duplex_pair(8192);
    let shutdown = Arc::new(AtomicBool::new(true));
    let config = TransportConfig::default();
    let stdout_writer = AsyncStdoutWriter::new(config, shutdown, Some(Box::new(writer)));

    let resp = Response::success(RequestId::Number(1), json!("pong"));
    let result = stdout_writer.write_response(&resp).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().is_shutdown());
}

#[tokio::test]
async fn io_stream_writer_flush_after_write() {
    let (mut reader, writer) = make_duplex_pair(8192);
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = TransportConfig {
        flush_after_write: true,
        ..Default::default()
    };
    let stdout_writer = AsyncStdoutWriter::new(config, shutdown, Some(Box::new(writer)));

    let resp = Response::success(RequestId::Number(1), json!("pong"));
    stdout_writer.write_response(&resp).await.unwrap();

    let mut buf = vec![0u8; 256];
    let n = reader.read(&mut buf).await.unwrap();
    assert!(n > 0);
    let written = String::from_utf8_lossy(&buf[..n]);
    assert!(written.contains("\"result\":\"pong\""));
}

// ---------------------------------------------------------------------------
// Bidirectional Transport Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn io_stream_bidirectional_ping_pong() {
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let router = make_test_router().await;
    let config = TransportConfig::default();

    let transport = StdioTransport::with_io(
        router,
        config,
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    // Send a request through stdin
    let req_bytes = encode_request(1, "ping", None);
    stdin_writer.write_all(&req_bytes).await.unwrap();

    // Signal EOF on stdin and wait for the read loop to finish
    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    // Read the response from stdout
    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    assert!(!output.is_empty());
    let resp: Response = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(resp.id, RequestId::Number(1));
    assert_eq!(resp.result, Some(json!("pong")));
}

#[tokio::test]
async fn io_stream_bidirectional_echo_with_params() {
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let router = make_test_router().await;
    let config = TransportConfig::default();

    let transport = StdioTransport::with_io(
        router,
        config,
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    let params = json!({ "message": "hello transport" });
    let req_bytes = encode_request(1, "echo", Some(params.clone()));
    stdin_writer.write_all(&req_bytes).await.unwrap();

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let resp: Response = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(resp.id, RequestId::Number(1));
    assert_eq!(resp.result, Some(params));
}

#[tokio::test]
async fn io_stream_bidirectional_error_response() {
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let router = make_test_router().await;

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    let req_bytes = encode_request(1, "error", None);
    stdin_writer.write_all(&req_bytes).await.unwrap();

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let resp: Response = serde_json::from_str(output.trim()).unwrap();
    assert!(resp.is_error());
    let err = resp.error.expect("error should be present");
    assert_eq!(err.code, -32603);
    assert_eq!(err.message, "handler failure");
}

#[tokio::test]
async fn io_stream_bidirectional_method_not_found() {
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let router = make_test_router().await;

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    let req_bytes = encode_request(1, "nonexistent", None);
    stdin_writer.write_all(&req_bytes).await.unwrap();

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let resp: Response = serde_json::from_str(output.trim()).unwrap();
    assert!(resp.is_error());
    let err = resp.error.expect("error should be present");
    assert_eq!(err.code, -32601); // MethodNotFound
}

#[tokio::test]
async fn io_stream_bidirectional_multiple_requests() {
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let router = make_test_router().await;
    let config = TransportConfig::default();

    let transport = StdioTransport::with_io(
        router,
        config,
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    stdin_writer.write_all(&encode_request(1, "ping", None)).await.unwrap();
    stdin_writer
        .write_all(&encode_request(2, "echo", Some(json!("hello"))))
        .await
        .unwrap();
    stdin_writer.write_all(&encode_request(3, "ping", None)).await.unwrap();

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 3);

    let resp1: Response = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(resp1.id, RequestId::Number(1));
    assert_eq!(resp1.result, Some(json!("pong")));

    let resp2: Response = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(resp2.id, RequestId::Number(2));
    assert_eq!(resp2.result, Some(json!("hello")));

    let resp3: Response = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(resp3.id, RequestId::Number(3));
    assert_eq!(resp3.result, Some(json!("pong")));
}

#[tokio::test]
async fn io_stream_bidirectional_preserves_string_id() {
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let router = make_test_router().await;

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    // Send a request with a string ID
    let req = Request::with_string_id("abc-123", "ping", None);
    let mut json = serde_json::to_string(&req).unwrap();
    json.push('\n');
    stdin_writer.write_all(json.as_bytes()).await.unwrap();

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let resp: Response = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(resp.id, RequestId::String("abc-123".to_string()));
}

#[tokio::test]
async fn io_stream_bidirectional_null_id_preserved() {
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let router = make_test_router().await;

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    // Build a request with null ID
    let req = Request {
        version: "2.0".to_string(),
        id: RequestId::Null,
        method: "ping".to_string(),
        params: None,
    };
    let mut json = serde_json::to_string(&req).unwrap();
    json.push('\n');
    stdin_writer.write_all(json.as_bytes()).await.unwrap();

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let resp: Response = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(resp.id, RequestId::Null);
}

// ---------------------------------------------------------------------------
// Lifecycle Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn io_stream_lifecycle_created_then_initialized() {
    let router = make_test_router().await;
    let (stdin_reader, writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(writer),
    );

    assert_eq!(transport.lifecycle_stage(), LifecycleStage::Created);

    transport.initialize().unwrap();
    assert_eq!(transport.lifecycle_stage(), LifecycleStage::Initialized);
}

#[tokio::test]
async fn io_stream_lifecycle_start_and_shutdown() {
    let router = make_test_router().await;
    let (stdin_reader, writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();
    assert!(transport.is_running());

    transport.shutdown_transport().unwrap();
    assert!(transport.is_shutdown());

    transport.run().await.unwrap();
}

#[tokio::test]
async fn io_stream_lifecycle_double_shutdown_is_safe() {
    let router = make_test_router().await;
    let (stdin_reader, writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    transport.shutdown_transport().unwrap();
    let result = transport.shutdown_transport();
    assert!(result.is_ok());

    transport.run().await.unwrap();
}

#[tokio::test]
async fn io_stream_lifecycle_start_requires_initialize() {
    let router = make_test_router().await;
    let (stdin_reader, writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(writer),
    );

    let result = transport.start_transport();
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("initialized") || msg.contains("lifecycle"));
}

#[tokio::test]
async fn io_stream_lifecycle_shutdown_without_start() {
    let router = make_test_router().await;
    let (stdin_reader, writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(writer),
    );

    transport.initialize().unwrap();
    transport.shutdown_transport().unwrap();
    assert!(transport.is_shutdown());

    transport.run().await.unwrap();
}

#[tokio::test]
async fn io_stream_lifecycle_backward_transition_rejected() {
    let router = make_test_router().await;
    let (stdin_reader, writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();
    transport.shutdown_transport().unwrap();

    // Cannot start after shutdown
    let result = transport.start_transport();
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// JSON-RPC Integration Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn io_stream_integration_router_dispatch_success() {
    let router = make_test_router().await;

    let req = Request::new(1, "ping", None);
    let resp = router.dispatch(req).await;
    assert!(resp.is_success());
    assert_eq!(resp.result, Some(json!("pong")));
    assert_eq!(resp.id, RequestId::Number(1));
}

#[tokio::test]
async fn io_stream_integration_router_dispatch_error() {
    let router = make_test_router().await;

    let req = Request::new(1, "error", None);
    let resp = router.dispatch(req).await;
    assert!(resp.is_error());
    let err = resp.error.expect("error should be present");
    assert_eq!(err.code, -32603);
}

#[tokio::test]
async fn io_stream_integration_router_method_not_found() {
    let router = make_test_router().await;

    let req = Request::new(1, "unknown_method", None);
    let resp = router.dispatch(req).await;
    assert!(resp.is_error());
    let err = resp.error.expect("error should be present");
    assert_eq!(err.code, -32601); // MethodNotFound
}

#[tokio::test]
async fn io_stream_integration_transport_forwards_to_router() {
    let router = make_test_router().await;
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    let params = json!({ "key": "value" });
    let req_bytes = encode_request(1, "echo", Some(params.clone()));
    stdin_writer.write_all(&req_bytes).await.unwrap();

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let resp: Response = serde_json::from_str(output.trim()).unwrap();
    assert!(resp.is_success());
    assert_eq!(resp.id, RequestId::Number(1));
    assert_eq!(resp.result, Some(params));
}

// ---------------------------------------------------------------------------
// Framing Tests
// ---------------------------------------------------------------------------

#[test]
fn io_stream_framing_encode_request() {
    let req = Request::new(1, "echo", Some(json!({ "key": "value" })));
    let encoded = nabu_core::io_stream::encode_message(&req).unwrap();
    assert!(encoded.ends_with('\n'));

    let parsed: Request = serde_json::from_str(encoded.trim()).unwrap();
    assert_eq!(parsed, req);
}

#[test]
fn io_stream_framing_encode_response() {
    let resp = Response::success(RequestId::Number(1), json!("pong"));
    let encoded = nabu_core::io_stream::encode_message(&resp).unwrap();
    assert!(encoded.ends_with('\n'));

    let parsed: Response = serde_json::from_str(encoded.trim()).unwrap();
    assert_eq!(parsed, resp);
}

#[test]
fn io_stream_framing_decode_request() {
    let json_str = r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":null}"#;
    let req: Request = nabu_core::io_stream::decode_message(json_str).unwrap();
    assert_eq!(req.method, "ping");
    assert_eq!(req.id, RequestId::Number(1));
}

#[test]
fn io_stream_framing_decode_response() {
    let json_str = r#"{"jsonrpc":"2.0","id":1,"result":"pong"}"#;
    let resp: Response = nabu_core::io_stream::decode_message(json_str).unwrap();
    assert!(resp.is_success());
    assert_eq!(resp.result, Some(json!("pong")));
}

#[test]
fn io_stream_framing_decode_invalid_json_returns_error() {
    let result = nabu_core::io_stream::decode_message::<Request>("not json");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Concurrent Handler Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn io_stream_concurrent_handlers_process_requests() {
    let router = Arc::new(Router::new());
    let counter = Arc::new(AtomicUsize::new(0));
    let handler = CountingHandler {
        counter: counter.clone(),
    };
    router.register("count", Arc::new(handler)).await;

    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (mut stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    for i in 1..=10i64 {
        let req_bytes = encode_request(i, "count", None);
        stdin_writer.write_all(&req_bytes).await.unwrap();
    }

    stdin_writer.shutdown().await.unwrap();
    transport.run().await.unwrap();
    transport.shutdown_transport().unwrap();
    drop(transport);

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).await.unwrap();

    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 10);

    for line in &lines {
        let resp: Response = serde_json::from_str(line).unwrap();
        assert!(resp.is_success());
        assert_eq!(resp.result, Some(json!("counted")));
    }

    assert_eq!(counter.load(Ordering::SeqCst), 10);
}

// ---------------------------------------------------------------------------
// EOF / Shutdown Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn io_stream_eof_terminates_read_loop() {
    let router = make_test_router().await;
    let (stdin_reader, mut stdin_writer) = make_duplex_pair(8192);
    let (stdout_reader, stdout_writer) = make_duplex_pair(8192);
    let _ = stdout_reader;

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    // Write one request and then close stdin (EOF)
    let req_bytes = encode_request(1, "ping", None);
    stdin_writer.write_all(&req_bytes).await.unwrap();
    stdin_writer.shutdown().await.unwrap();

    // Wait for the read loop to finish
    let result = transport.run().await;
    assert!(result.is_ok());

    transport.shutdown_transport().unwrap();
}

#[tokio::test]
async fn io_stream_shutdown_signal_stops_read_loop() {
    let router = make_test_router().await;
    let (stdin_reader, _stdin_writer) = make_duplex_pair(8192);
    let (_stdout_reader, stdout_writer) = make_duplex_pair(8192);

    let transport = StdioTransport::with_io(
        router,
        TransportConfig::default(),
        Box::new(tokio::io::BufReader::new(stdin_reader)),
        Box::new(stdout_writer),
    );

    transport.initialize().unwrap();
    transport.start_transport().unwrap();

    // Signal shutdown (no data written, no EOF)
    transport.signal_shutdown();

    // The read loop should exit
    let result = transport.run().await;
    assert!(result.is_ok());

    transport.shutdown_transport().unwrap();
}

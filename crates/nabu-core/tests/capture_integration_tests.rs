use nabu_core::capture::{CaptureEngine, CaptureRequest, FileDropHandler, IngestionStatus};
use nabu_core::models::knowledge_object::{ObjectContent, ObjectType};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_temp_dir(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("nabu_capture_integration_{}", name));
    let _ = fs::create_dir_all(&dir);
    (dir.clone(), dir)
}

fn teardown_temp_dir(dir: &std::path::PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

fn write_temp_file(dir: &std::path::Path, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

// ---------------------------------------------------------------------------
// End-to-end: File → CaptureEngine → Handler → Normaliser → Pipeline → KnowledgeObject
// ---------------------------------------------------------------------------

#[test]
fn e2e_text_file_produces_note() {
    let (dir, _) = setup_temp_dir("e2e_text");
    let file_path = write_temp_file(&dir, "note.txt", b"Hello, Nabu!");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    assert!(result.knowledge_object.is_some());
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Note);
    assert_eq!(obj.content, ObjectContent::PlainText);
    assert_eq!(obj.vault_id, "vault-1");
    assert_eq!(obj.metadata.title, Some("note".to_string()));
    assert_eq!(result.source, "file_drop");
    assert!(result.knowledge_object_id.is_some());
    assert_eq!(result.knowledge_object_id, Some(obj.id));

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_markdown_file_produces_note_with_markdown_content() {
    let (dir, _) = setup_temp_dir("e2e_markdown");
    let file_path = write_temp_file(&dir, "readme.md", b"# README\n\nContent here.");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Note);
    assert_eq!(obj.content, ObjectContent::Markdown);
    assert_eq!(obj.metadata.title, Some("readme".to_string()));

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_pdf_file_produces_pdf_object() {
    let (dir, _) = setup_temp_dir("e2e_pdf");
    let file_path = write_temp_file(&dir, "paper.pdf", b"%PDF-1.4 fake content");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Pdf);
    assert_eq!(obj.content, ObjectContent::Binary);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_image_file_produces_image_object() {
    let (dir, _) = setup_temp_dir("e2e_image");
    let file_path = write_temp_file(&dir, "photo.png", b"\x89PNG\r\n\x1a\nfake png data");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Image);
    assert_eq!(obj.content, ObjectContent::Binary);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_binary_file_produces_attachment() {
    let (dir, _) = setup_temp_dir("e2e_binary");
    let file_path = write_temp_file(&dir, "data.bin", &[0u8, 255u8, 128u8, 16u8]);

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Attachment);
    assert_eq!(obj.content, ObjectContent::Binary);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_invalid_path_returns_failed_result() {
    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": "/nonexistent/path/file.txt" }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert!(matches!(result.status, IngestionStatus::Failed(_)));
    assert!(result.knowledge_object.is_none());
    assert!(result.knowledge_object_id.is_none());
    assert_eq!(result.source, "file_drop");
}

#[test]
fn e2e_empty_file_produces_note() {
    let (dir, _) = setup_temp_dir("e2e_empty");
    let file_path = write_temp_file(&dir, "empty.txt", b"");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Note);
    assert_eq!(obj.content, ObjectContent::PlainText);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_unsupported_mime_type_produces_attachment() {
    let (dir, _) = setup_temp_dir("e2e_unknown");
    let file_path = write_temp_file(&dir, "unknown.xyz", b"some unknown content");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Attachment);
    assert_eq!(obj.content, ObjectContent::Binary);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_no_handler_returns_failed_result() {
    let engine = CaptureEngine::new();
    // No handler registered

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": "/path/to/file.txt" }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert!(matches!(result.status, IngestionStatus::Failed(_)));
    assert!(result.knowledge_object.is_none());
    assert_eq!(result.source, "file_drop");
}

#[test]
fn e2e_handler_can_be_unregistered() {
    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));
    assert!(engine.lookup("file_drop").is_some());

    engine.unregister("file_drop");
    assert!(engine.lookup("file_drop").is_none());

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": "/path/to/file.txt" }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert!(matches!(result.status, IngestionStatus::Failed(_)));
}

#[test]
fn e2e_json_file_produces_document_with_structured_content() {
    let (dir, _) = setup_temp_dir("e2e_json");
    let file_path = write_temp_file(&dir, "data.json", br#"{"key": "value", "count": 42}"#);

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Document);
    assert!(matches!(obj.content, ObjectContent::Structured(_)));

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_html_file_produces_document_with_html_content() {
    let (dir, _) = setup_temp_dir("e2e_html");
    let file_path = write_temp_file(&dir, "page.html", b"<html><body>Hello</body></html>");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Document);
    assert_eq!(obj.content, ObjectContent::Html);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_audio_file_produces_audio_recording() {
    let (dir, _) = setup_temp_dir("e2e_audio");
    let file_path = write_temp_file(&dir, "song.mp3", b"ID3 fake mp3 content");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::AudioRecording);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_video_file_produces_video_object() {
    let (dir, _) = setup_temp_dir("e2e_video");
    let file_path = write_temp_file(&dir, "clip.mp4", b"\x00\x00\x00\x20ftypfake mp4");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Video);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_unicode_filename_handled_correctly() {
    let (dir, _) = setup_temp_dir("e2e_unicode_日本語");
    let file_path = write_temp_file(&dir, "日本語ファイル.txt", b"Unicode content");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    let obj = result.knowledge_object.unwrap();
    assert_eq!(obj.object_type, ObjectType::Note);
    assert_eq!(obj.content, ObjectContent::PlainText);

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_directory_path_returns_failed_result() {
    let (dir, _) = setup_temp_dir("e2e_dir");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": dir.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert!(matches!(result.status, IngestionStatus::Failed(_)));
    assert!(result.knowledge_object.is_none());

    teardown_temp_dir(&dir);
}

#[test]
fn e2e_missing_payload_returns_failed_result() {
    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({}), // missing file_path
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert!(matches!(result.status, IngestionStatus::Failed(_)));
    assert!(result.knowledge_object.is_none());
    assert_eq!(result.source, "file_drop");
}

#[test]
fn e2e_result_contains_timestamp() {
    let (dir, _) = setup_temp_dir("e2e_timestamp");
    let file_path = write_temp_file(&dir, "ts.txt", b"timestamp test");

    let engine = CaptureEngine::new();
    engine.register(Arc::new(FileDropHandler::new()));

    let request = CaptureRequest {
        source_type: "file_drop".to_string(),
        payload: serde_json::json!({ "file_path": file_path.to_str() }),
        vault_id: "vault-1".to_string(),
        context: HashMap::new(),
    };

    let result = engine.ingest(request);
    assert_eq!(result.status, IngestionStatus::Success);
    assert!(!result.timestamp.is_empty());
    assert!(result.timestamp.ends_with("Z"));

    teardown_temp_dir(&dir);
}

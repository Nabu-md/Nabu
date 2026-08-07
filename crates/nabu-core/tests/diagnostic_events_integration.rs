//! Integration tests for the diagnostic event system.
//!
//! These tests verify that diagnostic events can be published and consumed
//! through the existing EventBus<PipelineEvent> without any modification to
//! the EventBus itself — confirming the unified event architecture.

use nabu_core::diagnostic::{
    Diagnostic, DiagnosticBatch, DiagnosticEvent, DiagnosticEventContract,
    BatchClearedEvent, BatchRemovedEvent,
    publish_diagnostic_event,
};
use nabu_core::diagnostic::{DiagnosticSeverity, TextPosition, TextRange};
use nabu_core::event_bus::kinds;
use nabu_core::event_bus::{EventBus, PipelineEvent};

use std::sync::Arc;

/// A diagnostic event can be published to the EventBus and received by a
/// subscriber under the correct kind string.
#[test]
fn diagnostic_event_publishes_through_event_bus() {
    let bus = EventBus::<PipelineEvent>::new();
    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let batch = DiagnosticBatch::new(
        "spell-checker",
        "vault:notes/example.md",
        vec![Diagnostic::new(
            DiagnosticSeverity::Warning,
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
            "typo detected",
        )],
    );
    let event = DiagnosticEvent::BatchPublished(batch);
    let kind = event.kind();

    bus.subscribe(kind, move |pe: &PipelineEvent| {
        if let PipelineEvent::Diagnostic(e) = pe {
            received_clone.lock().unwrap().push(e.clone());
        }
    });

    publish_diagnostic_event(&bus, &event);

    let stored = received.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0], event);
}

/// BatchCleared publishes under its own kind.
#[test]
fn batch_cleared_publishes_through_event_bus() {
    let bus = EventBus::<PipelineEvent>::new();
    let received = Arc::new(std::sync::Mutex::new(false));
    let received_clone = received.clone();

    let event = DiagnosticEvent::BatchCleared(
        BatchClearedEvent::new("ai-assistant", "vault:doc.md"),
    );

    bus.subscribe(event.kind(), move |pe: &PipelineEvent| {
        if let PipelineEvent::Diagnostic(e) = pe {
            let _ = e;
            *received_clone.lock().unwrap() = true;
        }
    });

    publish_diagnostic_event(&bus, &event);

    assert!(*received.lock().unwrap());
}

/// BatchRemoved publishes under its own kind with the batch_id.
#[test]
fn batch_removed_publishes_through_event_bus() {
    let bus = EventBus::<PipelineEvent>::new();
    let received = Arc::new(std::sync::Mutex::new(false));
    let received_clone = received.clone();

    let event = DiagnosticEvent::BatchRemoved(
        BatchRemovedEvent::new("ocr-engine", "vault:scan.png", uuid::Uuid::new_v4()),
    );

    bus.subscribe(event.kind(), move |pe: &PipelineEvent| {
        if let PipelineEvent::Diagnostic(e) = pe {
            assert_eq!(e.origin(), "ocr-engine");
            *received_clone.lock().unwrap() = true;
        }
    });

    publish_diagnostic_event(&bus, &event);

    assert!(*received.lock().unwrap());
}

/// A batch with multiple diagnostics serializes and round-trips correctly.
#[test]
fn batch_with_many_diagnostics_round_trips() {
    let diags = (0..5)
        .map(|i| {
            Diagnostic::new(
                DiagnosticSeverity::Error,
                TextRange::empty(TextPosition::new(i, 0)),
                format!("error #{}", i),
            )
        })
        .collect();

    let batch = DiagnosticBatch::new("validator", "vault:doc.md", diags);
    assert_eq!(batch.diagnostic_count(), 5);

    let json = serde_json::to_string(&batch).expect("serialize batch");
    let back: DiagnosticBatch = serde_json::from_str(&json).expect("deserialize batch");
    assert_eq!(batch, back);
}

/// Incremental batches carry the is_incremental flag through serialization.
#[test]
fn incremental_batch_serializes_flag() {
    let batch = DiagnosticBatch::new(
        "ai",
        "vault:doc.md",
        vec![Diagnostic::new(
            DiagnosticSeverity::Hint,
            TextRange::empty(TextPosition::new(0, 0)),
            "refinement",
        )],
    )
    .incremental(true)
    .replaces(uuid::Uuid::new_v4());

    let json = serde_json::to_string(&batch).unwrap();
    assert!(json.contains("\"is_incremental\""));
    assert!(json.contains("\"replaces\""));

    let back: DiagnosticBatch = serde_json::from_str(&json).unwrap();
    assert!(back.is_incremental);
    assert!(back.replaces.is_some());
}

/// DiagnosticEventContract trait methods work through the trait.
#[test]
fn event_contract_trait_works() {
    let batch = DiagnosticBatch::new("test", "vault:doc.md", vec![
        Diagnostic::new(
            DiagnosticSeverity::Information,
            TextRange::empty(TextPosition::new(0, 0)),
            "info",
        ),
    ]);
    let event = DiagnosticEvent::BatchPublished(batch);

    fn check<E: DiagnosticEventContract>(event: &E) {
        assert!(!event.kind().is_empty());
        assert!(!event.origin().is_empty());
        assert!(!event.resource_id().is_empty());
        assert!(event.timestamp().to_string().len() > 0);
    }

    check(&event);
}

/// The event is delivered as a PipelineEvent::Diagnostic variant.
#[test]
fn event_wraps_in_pipeline_event_correctly() {
    let event = DiagnosticEvent::BatchPublished(
        DiagnosticBatch::new("test", "vault:doc.md", vec![
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                TextRange::empty(TextPosition::new(0, 0)),
                "warn",
            ),
        ]),
    );

    let pipeline = event.to_pipeline_event();
    match &pipeline {
        PipelineEvent::Diagnostic(inner) => {
            assert_eq!(*inner, event);
        }
        _ => panic!("expected PipelineEvent::Diagnostic"),
    }

    // kind() and timestamp() work on the PipelineEvent wrapper
    assert_eq!(pipeline.kind(), event.kind());
    assert_eq!(pipeline.timestamp(), Some(event.timestamp()));
}

/// Subscribers only receive events matching their subscribed kind.
#[test]
fn subscriber_kind_isolation() {
    let bus = EventBus::<PipelineEvent>::new();
    let published = Arc::new(std::sync::Mutex::new(0usize));
    let cleared = Arc::new(std::sync::Mutex::new(0usize));
    let published_clone = published.clone();
    let cleared_clone = cleared.clone();

    bus.subscribe(kinds::DIAGNOSTIC_BATCH_PUBLISHED, move |_pe: &PipelineEvent| {
        *published_clone.lock().unwrap() += 1;
    });
    bus.subscribe(kinds::DIAGNOSTIC_BATCH_CLEARED, move |_pe: &PipelineEvent| {
        *cleared_clone.lock().unwrap() += 1;
    });

    // Publish a BatchPublished — only the published subscriber fires.
    let event = DiagnosticEvent::BatchPublished(
        DiagnosticBatch::new("test", "vault:doc.md", vec![
            Diagnostic::new(
                DiagnosticSeverity::Hint,
                TextRange::empty(TextPosition::new(0, 0)),
                "h",
            ),
        ]),
    );
    publish_diagnostic_event(&bus, &event);

    // Publish a BatchCleared — only the cleared subscriber fires.
    let cleared_event = DiagnosticEvent::BatchCleared(
        BatchClearedEvent::new("test", "vault:doc.md"),
    );
    publish_diagnostic_event(&bus, &cleared_event);

    assert_eq!(*published.lock().unwrap(), 1);
    assert_eq!(*cleared.lock().unwrap(), 1);
}

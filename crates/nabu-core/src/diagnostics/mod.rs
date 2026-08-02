//! # Diagnostics — Local-Only Observability Foundation
//!
//! Nabu's canonical logging and diagnostics system.
//!
//! ## Design Principles
//!
//! - **Zero telemetry** — Nothing ever leaves the local machine.
//! - **Structured logging** — Every event includes subsystem, component,
//!   operation, and duration fields. Never plain formatted strings.
//! - **Tracing spans** — Every major operation has a parent span that
//!   captures the full execution hierarchy.
//! - **Configuration** — Verbosity is controlled via`NABU_LOG` or `RUST_LOG`
//!   environment variables, no recompilation needed.
//! - **Local storage** — Logs are written to `.nabu/logs/` for post-hoc
//!   analysis. No external services.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use nabu_core::diagnostics;
//! use tracing::{info, span};
//! use uuid::Uuid;
//!
//! // Initialize once at application startup:
//! diagnostics::init(None, "nabu");
//!
//! // Structured tracing throughout all subsystems:
//! let object_id = Uuid::new_v4();
//! info!(
//!     subsystem = "capture",
//!     component = "engine",
//!     operation = "ingest",
//!     object_id = %object_id,
//!     "Capture session started"
//! );
//!
//! // Nested spans:
//! let parent = span!(tracing::Level::INFO, "capture_session", object_id = %object_id);
//! let _guard = parent.enter();
//! // ... nested operations inherit this span
//! ```
//!
//! ## Subsystem Identifiers
//!
//! Use these constants for the `subsystem` field in all tracing calls:
//!
//! | Identifier | Subsystem |
//! |-----------|-----------|
//! | `SUBSYSTEM_CAPTURE` | Capture engine |
//! | `SUBSYSTEM_PROCESSING` | Processing pipeline |
//! | `SUBSYSTEM_STORAGE` | Storage manager |
//! | `SUBSYSTEM_INDEXER` | Search index |
//! | `SUBSYSTEM_GRAPH` | Vault graph |
//! | `SUBSYSTEM_QUEUE` | Job queue |
//! | `SUBSYSTEM_WORKER` | Worker pool |
//! | `SUBSYSTEM_OCR` | OCR processing |
//! | `SUBSYSTEM_SPEECH` | Speech/Whisper |
//! | `SUBSYSTEM_AI` | AI services |
//! | `SUBSYSTEM_EMBEDDING` | Embedding generation |
//! | `SUBSYSTEM_SEARCH` | Search |
//! | `SUBSYSTEM_EXPORT` | Export |
//! | `SUBSYSTEM_PLUGIN` | Plugin system |
//! | `SUBSYSTEM_UI` | User interface |
//! | `SUBSYSTEM_VAULT` | Vault management |
//! | `SUBSYSTEM_EVENT_BUS` | Event bus |
//! | `SUBSYSTEM_REGISTRY` | Service registry |
//! | `SUBSYSTEM_PIPELINE` | Pipeline migration bridge |

pub mod init;
pub mod layers;
pub mod metrics;
pub mod performance;
pub mod spans;

pub use init::*;
pub use layers::*;
pub use metrics::*;
pub use performance::*;
pub use spans::*;

// ---------------------------------------------------------------------------
// Subsystem identifiers — use these as the `subsystem` field in tracing calls
// ---------------------------------------------------------------------------

/// Capture engine subsystem
pub const SUBSYSTEM_CAPTURE: &str = "capture";
/// Processing pipeline subsystem
pub const SUBSYSTEM_PROCESSING: &str = "processing";
/// Storage manager subsystem
pub const SUBSYSTEM_STORAGE: &str = "storage";
/// Search index subsystem
pub const SUBSYSTEM_INDEXER: &str = "indexer";
/// Vault graph subsystem
pub const SUBSYSTEM_GRAPH: &str = "graph";
/// Job queue subsystem
pub const SUBSYSTEM_QUEUE: &str = "queue";
/// Worker pool subsystem
pub const SUBSYSTEM_WORKER: &str = "worker";
/// OCR processing subsystem
pub const SUBSYSTEM_OCR: &str = "ocr";
/// Speech/Whisper transcription subsystem
pub const SUBSYSTEM_SPEECH: &str = "speech";
/// AI services subsystem
pub const SUBSYSTEM_AI: &str = "ai";
/// Embedding generation subsystem
pub const SUBSYSTEM_EMBEDDING: &str = "embedding";
/// Search subsystem
pub const SUBSYSTEM_SEARCH: &str = "search";
/// Export subsystem
pub const SUBSYSTEM_EXPORT: &str = "export";
/// Plugin system subsystem
pub const SUBSYSTEM_PLUGIN: &str = "plugin";
/// User interface subsystem
pub const SUBSYSTEM_UI: &str = "ui";
/// Vault management subsystem
pub const SUBSYSTEM_VAULT: &str = "vault";
/// Event bus subsystem
pub const SUBSYSTEM_EVENT_BUS: &str = "event_bus";
/// Service registry subsystem
pub const SUBSYSTEM_REGISTRY: &str = "registry";
/// Pipeline migration bridge subsystem
pub const SUBSYSTEM_PIPELINE: &str = "pipeline_migration";

/// All recognized subsystem identifiers (for validation / documentation).
pub const ALL_SUBSYSTEMS: &[&str] = &[
    SUBSYSTEM_CAPTURE,
    SUBSYSTEM_PROCESSING,
    SUBSYSTEM_STORAGE,
    SUBSYSTEM_INDEXER,
    SUBSYSTEM_GRAPH,
    SUBSYSTEM_QUEUE,
    SUBSYSTEM_WORKER,
    SUBSYSTEM_OCR,
    SUBSYSTEM_SPEECH,
    SUBSYSTEM_AI,
    SUBSYSTEM_EMBEDDING,
    SUBSYSTEM_SEARCH,
    SUBSYSTEM_EXPORT,
    SUBSYSTEM_PLUGIN,
    SUBSYSTEM_UI,
    SUBSYSTEM_VAULT,
    SUBSYSTEM_EVENT_BUS,
    SUBSYSTEM_REGISTRY,
    SUBSYSTEM_PIPELINE,
];

// ---------------------------------------------------------------------------
// Standard component identifiers — use these as the `component` field
// ---------------------------------------------------------------------------

/// Core engine component
pub const COMPONENT_ENGINE: &str = "engine";
/// Handler component
pub const COMPONENT_HANDLER: &str = "handler";
/// Pipeline component
pub const COMPONENT_PIPELINE: &str = "pipeline";
/// Processor component
pub const COMPONENT_PROCESSOR: &str = "processor";
/// Executor component
pub const COMPONENT_EXECUTOR: &str = "executor";
/// Pool component
pub const COMPONENT_POOL: &str = "pool";
/// Store component
pub const COMPONENT_STORE: &str = "store";
/// Manager component
pub const COMPONENT_MANAGER: &str = "manager";
/// Index component
pub const COMPONENT_INDEX: &str = "index";
/// Graph component
pub const COMPONENT_GRAPH: &str = "graph";

// ---------------------------------------------------------------------------
// Standard operation identifiers — use these as the `operation` field
// ---------------------------------------------------------------------------

/// Ingest operation
pub const OP_INGEST: &str = "ingest";
/// Process operation
pub const OP_PROCESS: &str = "process";
/// Save operation
pub const OP_SAVE: &str = "save";
/// Load operation
pub const OP_LOAD: &str = "load";
/// Delete operation
pub const OP_DELETE: &str = "delete";
/// Search operation
pub const OP_SEARCH: &str = "search";
/// Enqueue operation
pub const OP_ENQUEUE: &str = "enqueue";
/// Dequeue operation
pub const OP_DEQUEUE: &str = "dequeue";
/// Execute operation
pub const OP_EXECUTE: &str = "execute";
/// Build operation
pub const OP_BUILD: &str = "build";
/// Rebuild operation
pub const OP_REBUILD: &str = "rebuild";
/// Index operation
pub const OP_INDEX: &str = "index";
/// Update operation
pub const OP_UPDATE: &str = "update";
/// Cancel operation
pub const OP_CANCEL: &str = "cancel";
/// Retry operation
pub const OP_RETRY: &str = "retry";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_subsystems_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for subsystem in ALL_SUBSYSTEMS {
            assert!(
                seen.insert(subsystem),
                "Duplicate subsystem identifier: {}",
                subsystem
            );
        }
        assert_eq!(seen.len(), ALL_SUBSYSTEMS.len());
    }

    #[test]
    fn test_subsystem_constants_match() {
        assert_eq!(SUBSYSTEM_CAPTURE, "capture");
        assert_eq!(SUBSYSTEM_PROCESSING, "processing");
        assert_eq!(SUBSYSTEM_STORAGE, "storage");
        assert_eq!(SUBSYSTEM_INDEXER, "indexer");
        assert_eq!(SUBSYSTEM_GRAPH, "graph");
        assert_eq!(SUBSYSTEM_QUEUE, "queue");
        assert_eq!(SUBSYSTEM_WORKER, "worker");
        assert_eq!(SUBSYSTEM_EVENT_BUS, "event_bus");
        assert_eq!(SUBSYSTEM_REGISTRY, "registry");
    }
}

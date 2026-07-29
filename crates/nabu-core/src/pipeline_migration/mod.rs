//! # Pipeline Migration — Async Execution Bridge
//!
//! This module connects the queue infrastructure (Prompts 35–36) to the
//! documented platform subsystems (CaptureEngine, ProcessingPipeline,
//! StorageManager, EventBus). It implements the architectural migration
//! described in Prompt 37.
//!
//! ## Architecture (After Migration)
//!
//! ```text
//! Capture Source
//!     │
//!     ▼
//! ┌─────────────────┐
//! │  CaptureEngine  │  Creates job, enqueues, returns immediately
//! └────────┬────────┘
//!          │
//!     capture:browser / capture:clipboard / capture:file_drop / ...
//!          │
//!          ▼
//! ┌─────────────────┐
//! │  DurableJobQueue│  Persists job, schedules for worker
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │   WorkerPool    │  Picks up job, looks up executor
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌──────────────────────────────┐
//! │  PipelineExecutor           │  Implements JobExecutor
//! │  ├── ProcessingPipeline     │  Runs all processors in order
//! │  ├── StorageManager.save()  │  Persists processed result
//! │  └── EventBus events        │  Publishes lifecycle events
//! └──────────────────────────────┘
//!          │
//!          ├── EVENT_ITEM_STORED ──► Indexer.index_document()
//!          │                       └─► VaultGraph.update_node()
//!          ▼
//!     Processing Complete
//! ```

mod executor;
mod events;

pub use executor::*;
pub use events::*;

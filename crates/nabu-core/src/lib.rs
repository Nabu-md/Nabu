//! # Nabu Core Library
//!
//! The core library for the Nabu knowledge management platform.
//!
//! ## Architecture
//!
//! Every subsystem flows through the canonical pipeline:
//!
//! ```text
//! CaptureSource
//!     │  CaptureEngine.ingest()
//!     ▼
//! CaptureEvent (published to EventBus)
//!     │
//!     ▼
//! Job Queue (DurableJobQueue)
//!     │  WorkerPool dequeue → PipelineExecutor.execute()
//!     ▼
//! ProcessingPipeline
//!     │  All 14 processors execute in order
//!     ▼
//! StorageManager.save()
//!     │
//!     ├── Indexer.index_document()
//!     └── VaultGraph.update_node()
//! ```

pub mod event_bus;
pub mod models;
pub mod jobs;
pub mod processing;
pub mod capture;
pub mod pipeline_migration;
pub mod storage;
pub mod indexer;
pub mod graph;
pub mod registry;

// Re-export key types for convenient access
pub use event_bus::*;
pub use models::*;
pub use jobs::*;
pub use processing::*;
pub use capture::*;
pub use pipeline_migration::*;
pub use storage::*;
pub use indexer::*;
pub use graph::*;
pub use registry::*;

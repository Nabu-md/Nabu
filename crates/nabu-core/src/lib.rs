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

pub mod capture;
pub mod conversations;
pub mod diagnostic;
pub mod diagnostics;
pub mod event_bus;
pub mod graph;
pub mod history;
#[cfg(not(target_arch = "wasm32"))]
pub mod io_stream;
pub mod ipc_socket;
pub mod indexer;
pub mod jobs;
pub mod models;
pub mod native;
pub mod pipeline_migration;
pub mod plugin;
pub mod processing;
#[cfg(not(target_arch = "wasm32"))]
pub mod process_supervisor;
pub mod registry;
pub mod rpc;
pub mod storage;
pub mod sync;

// Re-export key types for convenient access
// Ambiguous glob re-exports are intentional — all public API types should
// be available at the crate root for ergonomic use by consumers.
#[allow(ambiguous_glob_reexports)]
pub use capture::*;
#[allow(ambiguous_glob_reexports)]
pub use conversations::*;
#[allow(ambiguous_glob_reexports)]
pub use diagnostic::*;
#[allow(ambiguous_glob_reexports)]
pub use diagnostics::*;
#[allow(ambiguous_glob_reexports)]
pub use event_bus::*;
#[allow(ambiguous_glob_reexports)]
pub use graph::*;
#[allow(ambiguous_glob_reexports)]
pub use history::*;
#[cfg(not(target_arch = "wasm32"))]
#[allow(ambiguous_glob_reexports)]
pub use io_stream::*;
#[allow(ambiguous_glob_reexports)]
pub use indexer::*;
#[allow(ambiguous_glob_reexports)]
pub use jobs::*;
#[allow(ambiguous_glob_reexports)]
pub use models::*;
#[allow(ambiguous_glob_reexports)]
pub use pipeline_migration::*;
#[allow(ambiguous_glob_reexports)]
pub use plugin::*;
#[allow(ambiguous_glob_reexports)]
pub use processing::*;
#[cfg(not(target_arch = "wasm32"))]
#[allow(ambiguous_glob_reexports)]
pub use process_supervisor::*;
#[allow(ambiguous_glob_reexports)]
pub use registry::*;
#[allow(ambiguous_glob_reexports)]
pub use rpc::*;
#[allow(ambiguous_glob_reexports)]
pub use storage::*;
#[allow(ambiguous_glob_reexports)]
pub use sync::*;

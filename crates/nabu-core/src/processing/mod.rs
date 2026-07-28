//! Processing module for the knowledge capture pipeline.
//!
//! This module provides the architecture for introducing a processing stage
//! between the [`IngestionPipeline`](crate::capture::IngestionPipeline) and the
//! [`StorageManager`](crate::storage::StorageManager).
//!
//! # Architecture
//!
//! ```text
//! CaptureEngine
//!     ↓  ItemCaptured
//! IngestionPipeline
//!     ↓  ItemProcessed
//! ProcessingPipeline
//!     ↓  ItemProcessingStarted
//!     ↓  ItemProcessingCompleted (or ItemProcessingFailed)
//!     ↓  ItemProcessed (re-published)
//! StorageManager
//!     ↓  ItemStored
//! ```
//!
//! # Components
//!
//! - [`Processor`] trait — a single processing step, composable and stateless.
//! - [`ProcessingPipeline`] — owns an ordered chain of processors, orchestrates
//!   execution, collects history, emits lifecycle events.
//! - [`ProcessingResult`] — structured result from a single processor execution.
//! - [`ProcessingDecision`] — whether to continue or reject.
//! - [`ProcessingHistoryEntry`] — a record of one processor's execution, stored
//!   in the object's metadata.
//!
//! # Constraints
//!
//! - Processors must NOT write to SQLite.
//! - Processors must NOT emit UI events.
//! - Processors must be composable.
//! - The pipeline never blocks asynchronously.
//! - Processing history is stored in the object's metadata, not a database.

mod history;
mod pipeline;
mod processor;

pub use history::{ProcessingHistoryEntry, PROCESSING_HISTORY_KEY};
pub use pipeline::ProcessingPipeline;
pub use processor::{ProcessingDecision, ProcessingResult, Processor};
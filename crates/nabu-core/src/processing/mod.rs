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
mod duplicate_detector;
mod timeline_extractor;
mod ocr_processor;
mod metadata_extractor;
#[cfg(all(feature = "native", any(target_os = "macos", target_os = "ios")))]
mod pdf_text_processor;
#[cfg(all(feature = "native", any(target_os = "macos", target_os = "ios")))]
mod pdf_metadata_processor;
#[cfg(all(feature = "native", any(target_os = "macos", target_os = "ios")))]
mod pdf_annotation_processor;
mod content_classifier;
mod auto_filer;
mod metadata_enricher;

pub use history::{ProcessingHistoryEntry, PROCESSING_HISTORY_KEY};
pub use pipeline::ProcessingPipeline;
pub use processor::{ProcessingDecision, ProcessingResult, Processor};
pub use duplicate_detector::{DuplicateConfidence, DuplicateDetector, DuplicateInfo};
pub use timeline_extractor::{TimelineExtractor, TimelineInfo};
pub use ocr_processor::{OcrInfo, OcrProcessor};
pub use metadata_extractor::MetadataExtractor;
#[cfg(all(feature = "native", any(target_os = "macos", target_os = "ios")))]
pub use pdf_text_processor::PdfTextProcessor;
#[cfg(all(feature = "native", any(target_os = "macos", target_os = "ios")))]
pub use pdf_metadata_processor::PdfMetadataProcessor;
#[cfg(all(feature = "native", any(target_os = "macos", target_os = "ios")))]
pub use pdf_annotation_processor::PdfAnnotationProcessor;
pub use content_classifier::{ClassificationResult, ContentClassifier};
pub use auto_filer::{AutoFileSuggestions, AutoFiler};
pub use metadata_enricher::MetadataEnricher;
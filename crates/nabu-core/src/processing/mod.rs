//! # Nabu Processing Pipeline
//!
//! The processing pipeline is the core of Nabu's content transformation system.
//! It takes raw captured content and runs it through a chain of processors that
//! classify, enrich, deduplicate, and prepare content for storage.
//!
//! ## Architecture
//!
//! ```text
//! Pipeline Context (KnowledgeObject + metadata)
//!     │
//!     ▼
//! ┌──────────────────────────────────────────────────┐
//! │              ProcessingPipeline                  │
//! │  ┌──────────┐ ┌──────────┐         ┌──────────┐ │
//! │  │Processor1│→│Processor2│→ ... → │ProcessorN│ │
//! │  └──────────┘ └──────────┘         └──────────┘ │
//! └──────────────────────────────────────────────────┘
//!     │
//!     ▼
//! Processed result (KnowledgeObject + processor output)
//! ```
//!
//! ## Migration to Async
//!
//! With Prompt 37, the ProcessingPipeline is no longer called directly from
//! capture handlers. Instead, it runs inside a `PipelineExecutor` that is
//! registered with the `ExecutorRegistry` and invoked by workers in the
//! `WorkerPool`. This makes processing asynchronous without changing any
//! processor logic.

mod pipeline;
mod processor;

pub use pipeline::*;
pub use processor::*;

//! # Nabu Worker Pool Runtime
//!
//! This module provides the asynchronous worker execution runtime that processes
//! jobs from the durable job queue (Prompt 35). It includes a configurable worker
//! pool, generic job executors, progress reporting, backpressure regulation,
//! and graceful shutdown.
//!
//! ## Architecture
//!
//! ```text
//!                     ┌─────────────────────────────────────┐
//!                     │           DurableJobQueue           │
//!                     │  (persists + prioritises jobs)      │
//!                     └────────────┬────────────────────────┘
//!                                  │
//!                                  ▼
//!                     ┌─────────────────────────────────────┐
//!                     │           WorkerPool                │
//!                     │  (manages lifecycle, concurrency)   │
//!                     ├─────────────────────────────────────┤
//!                     │  Worker 1  │  Worker 2  │  Worker N │
//!                     │  (tokio    │  (tokio    │  (tokio   │
//!                     │   task)    │   task)    │   task)   │
//!                     └────────────┴────────────┴───────────┘
//!                                  │
//!                     ┌────────────▼──────────────┐
//!                     │      ExecutorRegistry     │
//!                     │  ocr → OcrExecutor        │
//!                     │  whisper → WhisperExecutor│
//!                     │  embedding → ...          │
//!                     └───────────────────────────┘
//! ```
//!
//! ## Key Concepts
//!
//! - **Workers are generic**: They know nothing about job semantics. They receive
//!   a job, look up the executor for the job's type, and delegate execution.
//! - **Executors are pluggable**: New job types (OCR, Whisper, AI, embeddings,
//!   indexing, etc.) are added by registering executors — no worker changes needed.
//! - **Backpressure is natural**: The bounded channel between queue and workers
//!   provides natural backpressure. When all workers are busy, dispatch blocks,
//!   and the queue holds jobs safely in persistent storage.
//! - **Progress is transparent**: Workers report progress via a generic
//!   `ProgressReporter` interface — the reporter handles throttling and delivery.
//! - **Shutdown is graceful**: The pool drains active jobs before stopping,
//!   and queue state is always consistent (persisted before and after execution).

pub mod backpressure;
pub mod errors;
pub mod executor;
pub mod pool;
pub mod progress;
pub mod shutdown;
pub mod worker;

// Re-exports for convenience
pub use backpressure::{Backpressure, BackpressureStatus};
pub use errors::{WorkerError, WorkerResult};
pub use executor::{ExecuteContext, ExecuteResult, ExecutorRegistry, JobExecutor};
pub use pool::{PoolConfig, PoolHealth, WorkerPool};
pub use progress::{
    ChannelProgressReporter, InMemoryProgressTracker, ProgressConfig, ProgressReporter,
    ProgressSnapshot,
};
pub use shutdown::ShutdownCoordinator;
pub use worker::Worker;

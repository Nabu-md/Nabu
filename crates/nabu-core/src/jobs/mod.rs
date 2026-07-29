//! # Nabu Durable Job Queue
//!
//! This module provides the foundational infrastructure for asynchronous background
//! job processing in Nabu. It implements a durable, prioritized, retryable job queue
//! that survives application restarts, crashes, and power loss.
//!
//! ## Architecture
//!
//! ```text
//!                      ┌─────────────────────────────────────────┐
//!                      │             DurableJobQueue             │
//!                      │  (Queue trait + persistence)           │
//!                      └─────┬───────────────────────┬──────────┘
//!                            │                       │
//!                     ┌──────▼────────┐       ┌──────▼────────┐
//!                     │   JobStore    │       │ WorkerChannel │
//!                     │  (File I/O)   │       │  (mpsc chan)  │
//!                     └──────┬────────┘       └──────┬────────┘
//!                            │                       │
//!                     ┌──────▼────────┐       ┌──────▼────────┐
//!                     │ .nabu/jobs/   │       │  WorkerPool   │
//!                     │  (JSON files) │       │ (Prompt 36)   │
//!                     └───────────────┘       └───────────────┘
//! ```
//!
//! ## Key Components
//!
//! | Component | Module | Responsibility |
//! |-----------|--------|---------------|
//! | `Job` | `job` | Canonical job model with lifecycle methods |
//! | `Queue` | `queue` | Core queue interface (trait) |
//! | `DurableJobQueue` | `queue` | Default file-backed queue implementation |
//! | `JobStore` | `persistence` | File I/O for durable JSON persistence |
//! | `Scheduler` | `scheduler` | Delayed execution scheduling |
//! | `RetryPolicy` | `retry` | Exponential backoff retry configuration |
//! | `CancellationToken` | `cancellation` | Cooperative cancellation mechanism |
//! | `Priority` | `priority` | Priority levels (Critical → Background) |
//! | `WorkerChannel` | `worker_channel` | Queue-to-worker communication |
//! | `JobError` | `errors` | Typed error types |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use nabu_core::jobs::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), JobError> {
//!     let queue = DurableJobQueue::new(".nabu/jobs").await?;
//!
//!     // Enqueue a high-priority OCR job
//!     let mut payload = JobPayload::new();
//!     payload.insert("image_path".into(), "photo.png".into());
//!
//!     let job_id = queue
//!         .enqueue(
//!             Job::new("ocr", payload)
//!                 .with_priority(Priority::High)
//!                 .with_retries(3, RetryPolicy::default())
//!         )
//!         .await?;
//!
//!     // Dequeue and process
//!     if let Some(job) = queue.dequeue().await? {
//!         // ... process job ...
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod cancellation;
pub mod errors;
pub mod job;
pub mod persistence;
pub mod priority;
pub mod queue;
pub mod retry;
pub mod scheduler;
pub mod worker_channel;
pub mod workers;

// Re-exports for convenience
pub use cancellation::CancellationToken;
pub use errors::{JobError, JobResult};
pub use job::{Job, JobId, JobPayload, JobStatus, JobType};
pub use persistence::JobStore;
pub use priority::Priority;
pub use queue::{DurableJobQueue, Queue};
pub use retry::RetryPolicy;
pub use scheduler::{ScheduleSpec, Scheduler};
pub use worker_channel::{QueueMessage, WorkerChannel, WorkerHandle, WorkerMessage};

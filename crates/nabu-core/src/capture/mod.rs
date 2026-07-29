//! # Nabu Capture Engine
//!
//! The Capture Engine is the entry point for all content entering Nabu.
//! It provides a registry of `CaptureHandler` implementations and routes
//! incoming content to the appropriate handler.
//!
//! ## Migration to Async
//!
//! With Prompt 37, the Capture Engine no longer processes synchronously.
//! Instead:
//!
//! 1. The capture handler creates a `JobPayload` with the captured content.
//! 2. The `CaptureEngine` enqueues the job via the `DurableJobQueue`.
//! 3. The worker pool picks up the job, looks up the `PipelineExecutor`,
//!    and runs the `ProcessingPipeline` asynchronously.
//! 4. The capture call returns immediately — no blocking.
//!
//! ## Supported Sources (from architecture docs)
//!
//! | Source | Handler | Job Type |
//! |--------|---------|----------|
//! | Browser capture | `BrowserCaptureHandler` | `capture:browser` |
//! | Article (Readability) | `ArticleCaptureHandler` | `capture:article` |
//! | YouTube metadata | `YouTubeCaptureHandler` | `capture:youtube` |
//! | GitHub repository | `GitHubRepositoryHandler` | `capture:github` |
//! | Clipboard | `ClipboardHandler` | `capture:clipboard` |
//! | Screenshot | `ScreenshotHandler` | `capture:screenshot` |
//! | File drop | `FileDropHandler` | `capture:file_drop` |
//! | Watch folder | `WatchFolderHandler` | `capture:watch_folder` |
//! | Safari Reader | (via Safari extension) | `capture:safari_reader` |

mod engine;
mod handler;

pub use engine::*;
pub use handler::*;

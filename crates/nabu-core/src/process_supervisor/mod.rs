//! # Process Supervisor — foundation for managed subprocesses.
//!
//! This module establishes the core supervision infrastructure for Nabu's
//! long-running background service architecture. It provides the
//! [`ProcessSupervisor`], [`ProcessState`], and [`RestartPolicy`] types
//! that future managed services will rely on.
//!
//! ## Architecture
//!
//! ```text
//! ProcessSupervisor   (single authority for all managed subprocesses)
//! ├── processes: RwLock<HashMap<ProcessId, Arc<Mutex<ManagedProcess>>>>
//! ├── ctx: Arc<SupervisorContext>  (shutdown flag, active monitor count)
//! ├── event_bus: Option<Arc<EventBus<PipelineEvent>>>
//! └── lifecycle: LifecycleManager
//!
//! For each spawned process:
//!   1. A ManagedProcess record is created (state = Created)
//!   2. A broadcast channel for stop signaling is created
//!   3. The sender is stored in the record
//!   4. A tokio monitoring task is spawned (owns the Child)
//!   5. The task updates state, handles restarts, publishes events
//! ```
//!
//! ## Lifecycle
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//! ```
//!
//! ## Process lifecycle
//!
//! ```text
//! Created → Starting → Running → Exited / Failed → Restarting → Starting
//!                          │                            ↓
//!                          └─(no restart)→ Stopped ←──┘
//! Running → Stopping → Stopped
//! ```
//!
//! ## Future Compatibility
//!
//! This module is the single foundation for all future managed subprocesses:
//!
//! - MCP server supervision
//! - ACP server supervision
//! - Plugin host processes
//! - AI model workers
//! - OCR workers
//! - Sync services
//! - Search daemons
//! - External CLI integrations
//!
//! Future agents will assume `ProcessSupervisor`, `ProcessState`, and
//! `RestartPolicy` already exist. No major redesign is needed — new services
//! simply call `supervisor.spawn(config)` and manage the returned
//! `ProcessId`.

pub mod config;
pub mod errors;
pub mod managed;
pub mod monitor;
pub mod policy;
pub mod state;
pub mod supervisor;

// Re-export public types at the module level for ergonomic access.
pub use config::ProcessConfig;
pub use errors::{ProcessResult, ProcessSupervisorError};
pub use managed::ProcessSnapshot;
pub use policy::RestartPolicy;
pub use state::ProcessState;
pub use supervisor::{ProcessSupervisor, SupervisorSummary};

// Re-export ProcessId from the event_bus module (always available).
pub use crate::event_bus::ProcessId;

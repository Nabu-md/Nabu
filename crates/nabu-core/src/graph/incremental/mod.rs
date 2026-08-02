//! # Incremental Graph Update Engine
//!
//! Instead of rebuilding the entire graph on every content change, this module
//! tracks what changed and only recalculates the affected graph regions.
//!
//! ## Architecture
//!
//! ```text
//! Content Change (note edited, file added, etc.)
//!     │
//!     ▼
//! UpdateTracker: records which nodes/edges changed
//!     │
//!     ▼
//! DependencyTracker: finds transitively affected nodes
//!     │
//!     ▼
//! RegionEngine: identifies which graph regions are affected
//!     │
//!     ▼
//! IncrementalUpdateEngine: applies only the affected changes
//!     │
//!     ├── ChangeLog: appends change to persistent log
//!     └── Snapshots: updates in-memory graph state
//! ```

pub mod change_log;
pub mod dependency_tracker;
pub mod engine;
pub mod event_wiring;
pub mod region;
pub mod update_tracker;

pub use change_log::*;
pub use dependency_tracker::*;
pub use engine::*;
pub use event_wiring::*;
pub use region::*;
pub use update_tracker::*;

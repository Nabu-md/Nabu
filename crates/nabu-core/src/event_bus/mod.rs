//! Event bus module for decoupled service communication.
//!
//! This module provides a typed publish/subscribe event system for the Rust services.
//! Services communicate through events rather than direct imports, enabling future
//! subscribers (AI, Search, Graph, Automation, Plugins, Sync, Analytics) to react
//! to lifecycle events without modifying existing services.
//!
//! # Architecture
//!
//! The event bus follows the same patterns as the TypeScript event bus in the main
//! Tauri application. It is platform-independent and has no dependencies on
//! Electron, Tauri, or any UI framework.
//!
//! # Event Flow
//!
//! ```text
//! CaptureEngine
//!     ↓
//! ItemCaptured
//!     ↓
//! ProcessingPipeline
//!     ↓
//! ItemProcessed
//!     ↓
//! StorageManager
//!     ↓
//! ItemStored
//! ```

mod bus;
mod events;

pub use bus::EventBus;
pub use events::{ItemCaptured, ItemProcessed, ItemStored, KnowledgeEvents};

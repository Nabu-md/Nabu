//! # Nabu Event Bus
//!
//! A typed publish/subscribe event bus for decoupled service communication.
//! This is the platform's internal communication layer — services publish events
//! and subscribers react without direct coupling.
//!
//! The EventBus is used by the pipeline migration to broadcast lifecycle events:
//! - `ItemCaptured` — when a capture source ingests new content
//! - `ItemProcessingStarted` — when a worker begins processing
//! - `ItemProcessingCompleted` — when processing finishes successfully
//! - `ItemProcessingFailed` — when processing fails
//! - `ItemStored` — when the StorageManager has persisted the result
//!
//! See pipeine_migration::events for the full event registry.

mod bus;
pub use bus::*;

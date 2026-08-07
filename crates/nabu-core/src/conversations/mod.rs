//! # Conversation Persistence Layer
//!
//! This module provides durable storage for conversation threads (`Thread`,
//! `Message`, `Turn`). It isolates all storage concerns (file layout, atomic
//! writes, recovery, validation) from the conversation domain models.
//!
//! ## Architecture
//!
//! ```text
//! Thread / Message / Turn  (in-memory data models, derive Serialize/Deserialize)
//!     │
//!     ▼
//! ConversationStore (persistence layer)
//!     │  atomic write / read / discover / validate
//!     ▼
//! Disk  (.nabu/conversations/<uuid>.json + manifest)
//! ```
//!
//! ## Persistence Layout
//!
//! Threads are stored as individual JSON files under `.nabu/conversations/`
//! in the vault root:
//!
//! - `<uuid>.json` — serialized [`Thread`](crate::models::conversation::Thread)
//! - `manifest.json` — an ordered manifest of thread IDs, used for discovery
//!   and ordering during startup recovery.
//!
//! Writes use an atomic temp-file + rename pattern to prevent partial files
//! and data corruption on crash.
//!
//! ## Lifecycle Integration
//!
//! `ConversationStore` implements the [`Lifecycle`](crate::registry::lifecycle::Lifecycle)
//! trait. The [`ApplicationContext`](crate::registry::context::ApplicationContext)
//! manages it through the standard lifecycle:
//!
//! - **`initialize()`** — discovers persisted threads, deserializes and validates
//!   each one, and loads them into the in-memory cache. Corrupted or invalid
//!   threads are skipped with a warning (never panic).
//! - **`start()`** — marks the store as running and accepting save/load requests.
//! - **`shutdown()`** — flushes the manifest and marks the store as shut down.
//!
//! ## Thread Safety
//!
//! The store uses an internal `RwLock<HashMap<Uuid, Thread>>` for the
//! in-memory cache. All public methods are safe to call from multiple
//! threads concurrently (`Send + Sync`).
//!
//! ## Future Compatibility
//!
//! The manifest uses a versioned schema so future phases (indexing, search,
//! synchronization, cloud providers, encryption) can evolve the on-disk
//! format. The store can be registered as a singleton in the
//! [`ServiceRegistry`](crate::registry::ServiceRegistry) under the key
//! `"conversation_store"`.

pub mod error;
pub mod store;

pub use error::*;
pub use store::*;

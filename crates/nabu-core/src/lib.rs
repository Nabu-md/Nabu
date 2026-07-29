// ---------------------------------------------------------------------------
// Architectural note: lib.rs defines the top-level module structure for
// the Nabu core library.
//
// All subsystems follow the Universal Knowledge Object architecture:
//
//   1. KnowledgeObject is the universal runtime model (Principle 2).
//      No subsystem invents its own object model.
//
//   2. Data flows through exactly one pipeline:
//      Capture → KnowledgeObject → Processing → Storage → EventBus → UI
//      No feature bypasses this lifecycle (Principle 3).
//
//   3. Services never own canonical data (Principle 4).
//      Markdown files on disk are the source of truth (Principle 1).
//      Everything under .nabu/ is derived and rebuildable (Principle 9).
//
//   4. Tantivy is the single search engine (Principle 6).
//      VaultGraph is the single relationship graph (Principle 7).
//      No secondary indexes or feature-specific graphs.
//
//   5. Views (inbox, table, graph, etc.) are projections of
//      KnowledgeObjects — never duplicates (Principle 5).
// ---------------------------------------------------------------------------
pub mod capture;
pub mod event_bus;
pub mod export_engine;
pub mod graph;
pub mod models;
pub mod markdown;
pub mod processing;
pub mod storage;
pub mod template_manager;
pub mod reading_queue;
pub mod theme_manager;
pub mod vault_config;

#[cfg(feature = "native")]
pub mod registry;

#[cfg(feature = "native")]
pub mod view_state;

#[cfg(feature = "native")]
pub mod indexer;

#[cfg(feature = "native")]
pub mod vault;

#[cfg(feature = "native")]
pub mod watcher;

#[cfg(all(feature = "native", any(target_os = "macos", target_os = "ios")))]
pub mod native;
pub mod search_query;

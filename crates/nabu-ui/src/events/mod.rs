//! # Frontend Event Infrastructure
//!
//! The single, reusable abstraction the Dioxus frontend uses to consume
//! platform events broadcast by the Tauri backend.
//!
//! ## Architecture
//!
//! ```text
//! Platform Service → EventBus → EventBusBridge (emit_str "nabu-event")
//!                    → [this module] → Application Components
//! ```
//!
//! The backend `EventBusBridge` (`src-tauri/src/event_bridge.rs`) forwards every
//! published platform event onto one Tauri channel — `nabu-event` — as a
//! `FrontendEvent` envelope (`{ event_type, timestamp, payload }`). This module
//! installs **one** listener on that channel, deserializes each envelope into a
//! strongly-typed [`FrontendEvent`], and fans it out to typed subscribers.
//!
//! ## No direct Tauri access
//!
//! Components must never call `window.__TAURI__.event.listen(...)` directly.
//! They go through [`EventService`] (`use_event_service` / `use_event_listener`),
//! which is the canonical, centralized frontend event interface.
//!
//! ## Layers
//!
//! - [`types`] — the typed data model (`FrontendEvent`, `FrontendEventKind`).
//! - [`bindings`] — the raw `window.__TAURI__.event.listen` wrapper and the
//!   envelope-deserialization shim (the only place the Tauri listen API is
//!   touched).
//! - [`service`] — the subscription manager (`EventService`, `EventSubscription`).
//! - [`provider`] — the Dioxus context provider + ergonomic hooks.

pub mod bindings;
pub mod provider;
pub mod service;
pub mod types;

pub use bindings::{tauri_available, FRONTEND_EVENT_CHANNEL};
pub use provider::{use_event_listener, use_event_service, EventServiceProvider};
pub use service::{EventService, EventServiceError, EventSubscription};
pub use types::{extract_payload, FrontendEvent, FrontendEventKind};

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
//! published platform event onto one Tauri channel — [`FRONTEND_EVENT_CHANNEL`]
//! (`"nabu-event"`) — as a `FrontendEvent` envelope (`{ event_type, timestamp,
//! payload }`). This module installs **one** listener on that channel,
//! deserializes each envelope into a typed [`FrontendEvent`], and fans it out to
//! typed subscribers.
//!
//! ## No direct Tauri access
//!
//! Components must never call `window.__TAURI__.event.listen(...)` directly.
//! They subscribe via [`EventService::subscribe`] or the ergonomic
//! [`use_event_listener`] hook, which are the canonical frontend event entry
//! points.
//!
//! ## Layers
//!
//! - [`types`] — the typed data model (`FrontendEvent`, `FrontendEventKind`),
//!   plus the envelope deserializer (pure Rust).
//! - [`bindings`] — the raw `window.__TAURI__.event.listen` wrapper and the
//!   JS-value → envelope shim (the only place the Tauri listen API is touched).
//! - [`service`] — the subscription manager ([`EventService`],
//!   [`EventSubscription`]) and in-process dispatch.
//! - [`provider`] — the Dioxus context provider and ergonomic hooks.

pub mod bindings;
pub mod provider;
pub mod service;
pub mod types;

pub use bindings::tauri_available;
pub use provider::{use_event_listener, use_event_service, EventServiceProvider};
pub use service::{EventService, EventSubscription};
pub use types::{EventError, FrontendEvent, FrontendEventKind, FRONTEND_EVENT_CHANNEL};

/// Re-export of the backend `EventBus` kind-string constants, so callers that
/// need a raw kind can reference the canonical names rather than strings.
pub use types::raw_kinds as kinds;

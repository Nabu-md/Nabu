//! # Conversation Domain Models
//!
//! Canonical, provider-agnostic conversation models for the Nabu Capability
//! Platform. These types establish a shared hierarchy used by all future
//! conversational capabilities — ACP sessions, MCP conversations, AI assistants,
//! plugin conversations, automation workflows, and collaborative interactions.
//!
//! ## Architecture
//!
//! The hierarchy is strictly parent-child:
//!
//! ```text
//! Thread
//! ├── Message
//! │      ├── Turn
//! │      └── Turn
//! ├── Message
//! │      └── Turn
//! └── ...
//! ```
//!
//! | Type       | File         | Kind   | Purpose                                      |
//! |------------|--------------|--------|----------------------------------------------|
//! | [`Thread`]   | [`thread`]   | struct | A complete conversation                        |
//! | [`Message`]  | [`message`]  | struct | A logical message exchanged within a thread  |
//! | [`Turn`]     | [`turn`]     | struct | An individual interaction step                |
//! | [`Role`]     | [`role`]     | enum   | Participant role (extensible)                 |
//! | [`TurnContent`] | [`turn`]  | enum  | Content payload for a turn (extensible)     |
//! | [`Participant`] | [`participant`] | struct | Participant metadata       |
//! | [`ConversationError`] | [`error`] | enum | Structured validation errors          |
//!
//! ## Provider independence
//!
//! These models make no assumptions about any specific AI provider, protocol,
//! or transport. The [`Role`] enum is `#[non_exhaustive]` and designed to
//! accommodate plugins, services, automation, and external systems — not just
//! human and AI participants.
//!
//! ## Thread safety
//!
//! All types are plain data (`Clone`, `Send`, `Sync`). They contain no
//! interior mutability, no `Arc`/`Rc`, and no shared state. They are safe
//! to pass across thread boundaries.
//!
//! ## Future compatibility
//!
//! - Struct fields use `#[serde(default)]` and `skip_serializing_if` so new
//!   fields can be added without breaking deserialization.
//! - Enums are `#[non_exhaustive]` so new variants can be added without
//!   breaking external matches.
//! - Content is represented as an enum (`TurnContent`) rather than a fixed
//!   string, enabling future structured content (attachments, tool calls,
//!   citations, streaming responses) without model restructuring.
//!
//! ## Module structure
//!
//! | Module           | Contents                                              |
//! |------------------|-------------------------------------------------------|
//! | [`participant`]  | `Participant`, participant metadata                    |
//! | [`role`]         | `Role` enum                                           |
//! | [`turn`]         | `Turn`, `TurnContent`                                 |
//! | [`message`]      | `Message`                                             |
//! | [`thread`]       | `Thread`                                              |
//! | [`error`]        | `ConversationError`, `ConversationResult`             |

pub mod error;
pub mod message;
pub mod participant;
pub mod role;
pub mod thread;
pub mod turn;

pub use error::{ConversationError, ConversationResult};
pub use message::Message;
pub use participant::Participant;
pub use role::Role;
pub use thread::Thread;
pub use turn::Turn;

//! # Nabu Core Library
//!
//! Core engine for the Nabu knowledge base application.
//! This crate provides the foundational infrastructure including:
//!
//! - `event_bus` — Typed publish/subscribe event bus for decoupled communication
//! - `jobs` — Durable job queue with persistence, priority, retry, scheduling
//! - `workers` — Async worker pool with configurable concurrency, backpressure, graceful shutdown
//! - `capture` — Capture engine with source handlers and queue integration
//! - `processing` — Processing pipeline with ordered processor chain
//! - `pipeline_migration` — Async bridge connecting capture → queue → workers → pipeline → store

pub mod event_bus;
pub mod jobs;
pub mod capture;
pub mod processing;
pub mod pipeline_migration;

//! Native macOS engines (Vision OCR, PDFKit, local Whisper).
//!
//! This module is the single boundary between Rust-native processing and
//! Objective-C frameworks. All Objective-C types stay inside `native/`;
//! everything re-exported here is pure Rust.
//!
//! Every submodule provides a working implementation on macOS and a graceful
//! [`NativeError::UnsupportedPlatform`] fallback on other platforms, so the
//! crate always compiles.

pub mod error;
pub mod pdfkit;
pub mod vision;
pub mod whisper;

pub use error::NativeError;

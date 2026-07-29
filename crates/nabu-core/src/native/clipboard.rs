//! Native clipboard layer using Apple's AppKit framework via `objc2`.
//!
//! This module provides thin, memory-safe wrappers around `NSPasteboard`
//! operations. The public API uses only Rust-native types; Objective-C types
//! (`NSPasteboard`, `NSArray`, `NSData`, etc.) are strictly internal.

use std::path::Path;

use anyhow::{Context, Result};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::ClassType;
use objc2_foundation::{NSArray, NSData, NSDictionary, NSObject, NSString, NSURL};

objc2::extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    struct NSPasteboard;
);

// ---------------------------------------------------------------------------
// Rust-native public types
// ---------------------------------------------------------------------------

/// The content currently on the clipboard.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ClipboardContent {
    /// Plain text.
    Text(String),
    /// HTML content.
    Html(String),
    /// A file URL.
    Url(String),
    /// Raw image data (PNG/JPEG/TIFF).
    Image(Vec<u8>),
    /// No clipboard content.
    None,
}

// ---------------------------------------------------------------------------
// NSPasteboard helpers (internal)
// ---------------------------------------------------------------------------

impl NSPasteboard {
    /// Return the general pasteboard.
    fn general() -> Retained<Self> {
        // SAFETY: `generalPasteboard` returns the shared singleton instance.
        unsafe { msg_send![NSPasteboard::class(), generalPasteboard] }
    }

    /// Return the current change count.
    fn change_count(&self) -> u64 {
        // SAFETY: `changeCount` returns a primitive `NSInteger`.
        unsafe { msg_send![&*self, changeCount] }
    }

    /// Return the types currently on the pasteboard.
    fn types(&self) -> Option<Retained<NSArray<NSString>>> {
        // SAFETY: `types` returns an autoreleased `NSArray` (or `nil`).
        unsafe { msg_send![&*self, types] }
    }

    /// Return data for a given type.
    fn data_for_type(&self, type_str: &NSString) -> Option<Retained<NSData>> {
        // SAFETY: `dataForType:` returns autoreleased `NSData` (or `nil`).
        unsafe { msg_send![&*self, dataForType: type_str] }
    }

    /// Return the first item's string representation, if available.
    pub fn string_for_type(&self, type_str: &NSString) -> Option<Retained<NSString>> {
        // SAFETY: `stringForType:` returns an autoreleased `NSString` (or `nil`).
        unsafe { msg_send![&*self, stringForType: type_str] }
    }

    /// Return the first file URL, if available.
    fn url_for_type(&self, type_str: &NSString) -> Option<Retained<NSURL>> {
        // SAFETY: `URLForType:` returns an autoreleased `NSURL` (or `nil`).
        unsafe { msg_send![&*self, URLForType: type_str] }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the current clipboard content.
pub fn read_clipboard() -> ClipboardContent {
    let pasteboard = NSPasteboard::general();
    let types = match pasteboard.types() {
        Some(t) => t,
        None => return ClipboardContent::None,
    };

    // Check for plain text.
    let text_type = NSString::from_str("public.utf8-plain-text");
    if let Some(s) = pasteboard.string_for_type(&text_type) {
        return ClipboardContent::Text(s.to_string());
    }

    // Check for HTML.
    let html_type = NSString::from_str("public.html");
    if let Some(s) = pasteboard.string_for_type(&html_type) {
        return ClipboardContent::Html(s.to_string());
    }

    // Check for file URL.
    let url_type = NSString::from_str("public.file-url");
    if let Some(url) = pasteboard.url_for_type(&url_type) {
        if let Some(path_str) = url.path() {
            return ClipboardContent::Url(path_str.to_string());
        }
    }

    // Check for image data.
    let tiff_type = NSString::from_str("public.tiff");
    if let Some(data) = pasteboard.data_for_type(&tiff_type) {
        return ClipboardContent::Image(data.to_vec());
    }

    ClipboardContent::None
}

/// Return whether the pasteboard contains the given type.
pub fn pasteboard_contains_type(type_str: &str) -> bool {
    let pasteboard = NSPasteboard::general();
    let ns_type = NSString::from_str(type_str);
    let types = match pasteboard.types() {
        Some(t) => t,
        None => return false,
    };

    // SAFETY: `containsObject:` takes an `id` argument and returns a `BOOL`.
    unsafe { msg_send![&*types, containsObject: &*ns_type] }
}

/// Return the current change count of the general pasteboard.
pub fn pasteboard_change_count() -> u64 {
    NSPasteboard::general().change_count()
}

/// Read a file from a path.
pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    let url = NSURL::from_file_path(path).context("Invalid file path")?;
    // SAFETY: `url` is a valid file URL; `dataWithContentsOfURL:` returns
    // autoreleased `NSData` (or `nil` if the file cannot be read).
    let data: Option<Retained<NSData>> =
        unsafe { msg_send![NSData::class(), dataWithContentsOfURL: &*url] };
    data.map(|d| d.to_vec()).context("Failed to read file data")
}

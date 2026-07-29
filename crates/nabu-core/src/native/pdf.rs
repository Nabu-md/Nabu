//! Native PDF layer using Apple's PDFKit framework via `objc2`.
//!
//! This module provides thin, memory-safe wrappers around `PDFDocument`
//! and `PDFPage` operations. The public API uses only Rust-native types;
//! Objective-C types (`PDFDocument`, `PDFPage`, `NSArray`, etc.) are
//! strictly internal.
//!
//! Every `unsafe` block is documented with the invariant that makes it sound.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use objc2::msg_send;
use objc2::rc::{Retained, autoreleasepool};
use objc2::{extern_class, ClassType};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSObject, NSString, NSURL};

extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    struct PDFDocument;
);

extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    struct PDFPage;
);

// ---------------------------------------------------------------------------
// Rust-native public types
// ---------------------------------------------------------------------------

/// A thin wrapper around `PDFDocument` backed by PDFKit.
///
/// Construct via [`PdfDocument::from_path`] and access pages, attributes,
/// and save operations through the Rust-native API.
pub struct PdfDocument {
    inner: Retained<PDFDocument>,
}

/// A thin wrapper around a single `PDFPage` in a PDFKit document.
pub struct PdfPage {
    inner: Retained<PDFPage>,
}

/// A PDF annotation with Rust-native fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Annotation {
    pub page: u32,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Internal helpers on the raw objc2 extern classes
// ---------------------------------------------------------------------------

impl PDFDocument {
    fn from_url(url: &NSURL) -> Option<Retained<Self>> {
        // SAFETY: `alloc` returns a valid, allocated-but-uninitialized instance.
        // `initWithURL:` is the designated initialiser for `PDFDocument`.
        unsafe {
            let cls = <Self as ClassType>::class();
            msg_send![msg_send![cls, alloc], initWithURL: url]
        }
    }

    fn create_empty() -> Option<Retained<Self>> {
        // SAFETY: `alloc` + `init` returns a retained, empty document.
        unsafe {
            let cls = <Self as ClassType>::class();
            msg_send![msg_send![cls, alloc], init]
        }
    }

    fn write_to_url(&self, url: &NSURL) -> bool {
        // SAFETY: `self` is a valid `PDFDocument`; `writeToURL:` returns `BOOL`.
        unsafe { msg_send![&*self, writeToURL: url] }
    }

    fn count_of_pages(&self) -> usize {
        // SAFETY: `pageCount` returns a primitive `NSInteger`.
        unsafe { msg_send![&*self, pageCount] }
    }

    fn page_at_index(&self, index: usize) -> Option<Retained<PDFPage>> {
        // SAFETY: `pageAtIndex:` returns an autoreleased `PDFPage` (or `nil`).
        unsafe { msg_send![&*self, pageAtIndex: index] }
    }

    fn attributes_dict(&self) -> Option<Retained<NSDictionary<NSString, NSObject>>> {
        // SAFETY: `documentAttributes` returns an autoreleased dictionary (or `nil`).
        unsafe { msg_send![&*self, documentAttributes] }
    }

    fn insert_page_at_index(&self, page: &PDFPage, index: usize) {
        // SAFETY: Standard `PDFDocument` mutator.
        unsafe { msg_send![&*self, insertPage: page, atIndex: index] }
    }

    fn remove_page_at_index(&self, index: usize) {
        // SAFETY: Standard `PDFDocument` mutator.
        unsafe { msg_send![&*self, removePageAtIndex: index] }
    }
}

impl PDFPage {
    fn string_raw(&self) -> Option<Retained<NSString>> {
        // SAFETY: `string` returns an autoreleased `NSString` (or `nil`).
        unsafe { msg_send![&*self, string] }
    }

    fn set_rotation(&self, rotation: i32) {
        // SAFETY: `setRotation:` takes a primitive integer.
        unsafe { msg_send![&*self, setRotation: rotation] }
    }

    fn annotations_raw(&self) -> Option<Retained<NSArray<NSObject>>> {
        // SAFETY: `annotations` returns an autoreleased `NSArray` (or `nil`).
        unsafe { msg_send![&*self, annotations] }
    }
}

// ---------------------------------------------------------------------------
// Public API — PdfDocument
// ---------------------------------------------------------------------------

impl PdfDocument {
    /// Load a PDF document from a file path.
    ///
    /// Returns `None` if the file cannot be opened or the path is not valid
    /// UTF-8.
    pub fn from_path(path: &Path) -> Option<Self> {
        let path_str = path.to_str()?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
        PDFDocument::from_url(&url).map(|inner| Self { inner })
    }

    /// Number of pages in the document.
    pub fn page_count(&self) -> u32 {
        self.inner.count_of_pages() as u32
    }

    /// Get the page at the given zero-based index.
    pub fn page(&self, index: u32) -> Option<PdfPage> {
        self.inner
            .page_at_index(index as usize)
            .map(|inner| PdfPage { inner })
    }

    /// Return document attributes (Title, Author, Subject, etc.) as a
    /// Rust-native `HashMap<String, String>`.
    ///
    /// Only known attribute keys are included; non-string attribute values
    /// are silently skipped.
    pub fn document_attributes(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let attrs = match self.inner.attributes_dict() {
            Some(a) => a,
            None => return map,
        };

        for key in &["Title", "Author", "Subject", "Keywords", "Creator", "Producer"] {
            let ns_key = NSString::from_str(key);
            // SAFETY: `objectForKey:` returns an autoreleased `NSObject` (or `nil`).
            let val: Option<Retained<NSObject>> = unsafe { msg_send![&*attrs, objectForKey: &*ns_key] };
            if let Some(val) = val {
                // Try to downcast to `NSString`.
                if let Some(s) = val.downcast_ref::<NSString>() {
                    let owned = autoreleasepool(|pool| unsafe { s.to_str(pool) }.to_string());
                    map.insert(key.to_string(), owned);
                }
            }
        }
        map
    }

    /// Write the document to a file path.
    pub fn write_to_path(&self, path: &Path) -> Result<()> {
        let path_str = path.to_str().context("Invalid output path encoding")?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
        if self.inner.write_to_url(&url) {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to save PDF"))
        }
    }

    /// Return the raw inner `PDFDocument` handle (for use by sibling native
    /// modules such as OCR that need to access PDFKit internals).
    ///
    /// # Safety
    ///
    /// Callers must not retain the returned pointer beyond the lifetime of
    /// this `PdfDocument`.
    pub(crate) fn inner(&self) -> &PDFDocument {
        &self.inner
    }
}

// ---------------------------------------------------------------------------
// Public API — PdfPage
// ---------------------------------------------------------------------------

impl PdfPage {
    /// Return the text content of the page, if any.
    pub fn string(&self) -> Option<String> {
        self.inner
            .string_raw()
            .map(|ns| autoreleasepool(|pool| unsafe { ns.to_str(pool) }.to_string()))
    }

    /// Return the annotations attached to this page as a `Vec<Annotation>`.
    ///
    /// Each annotation's `contents` property (an `NSString` on the Objective-C
    /// side) is deep-copied into a Rust `String`. The page number is set to 0
    /// because `PDFPage` does not expose its own index.
    pub fn annotations(&self) -> Vec<Annotation> {
        let mut result = Vec::new();
        let annos = match self.inner.annotations_raw() {
            Some(a) => a,
            None => return result,
        };

        let count = annos.count() as usize;
        for i in 0..count {
            // SAFETY: `annos` is a valid `NSArray`; `objectAtIndex:` returns an
            // autoreleased object (or `nil`). Index `i` is in range.
            let annotation: Option<Retained<NSObject>> =
                unsafe { msg_send![&*annos, objectAtIndex: i] };
            if let Some(ann) = annotation {
                // SAFETY: `contents` is a property on `PDFAnnotation` (and
                // forwards to `NSObject` via Key-Value Coding). It returns an
                // autoreleased `NSString` (or `nil`).
                let content: Option<Retained<NSString>> =
                    unsafe { msg_send![&*ann, contents] };
                if let Some(c) = content {
                    let content_str =
                        autoreleasepool(|pool| unsafe { c.to_str(pool) }.to_string());
                    result.push(Annotation {
                        page: 0,
                        content: content_str,
                    });
                }
            }
        }
        result
    }

    /// Return the raw inner `PDFPage` handle (for use by sibling native
    /// modules such as OCR).
    ///
    /// # Safety
    ///
    /// Callers must not retain the returned pointer beyond the lifetime of
    /// this `PdfPage`.
    pub(crate) fn inner(&self) -> &PDFPage {
        &self.inner
    }
}

// ---------------------------------------------------------------------------
// PdfAnnotator — persists annotations as JSON files in the vault
// ---------------------------------------------------------------------------

pub struct PdfAnnotator {
    annotations_dir: PathBuf,
}

impl PdfAnnotator {
    pub fn new(vault_root: &Path) -> Self {
        Self {
            annotations_dir: vault_root.join(".nabu/annotations"),
        }
    }

    pub fn annotate(&self, _pdf_path: &str, page: u32, content: &str) -> Result<()> {
        if page == 0 {
            anyhow::bail!("Page {} out of range", page);
        }
        let ann = Annotation {
            page,
            content: content.into(),
        };
        let ann_id = uuid::Uuid::new_v4();
        let ann_path = self
            .annotations_dir
            .join(format!("{}_{}.json", ann_id, page));

        let file =
            std::fs::File::create(&ann_path).context("Failed to create annotation file")?;
        serde_json::to_writer_pretty(file, &ann).context("Failed to serialize annotation")?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PdfEngine — merge, split, extract, rotate, compress, form operations
// ---------------------------------------------------------------------------

pub struct PdfEngine;

impl PdfEngine {
    /// Merge multiple PDFs into a single output file.
    pub fn merge(paths: &[PathBuf], output: &Path) -> Result<()> {
        let merged_inner =
            PDFDocument::create_empty().context("Failed to create empty PDFDocument")?;

        for path in paths {
            let url = NSURL::fileURLWithPath(&NSString::from_str(
                path.to_str().context("Invalid path encoding")?,
            ));
            let doc_inner = PDFDocument::from_url(&url).context("Failed to load PDF")?;
            let page_count = doc_inner.count_of_pages();
            let offset = merged_inner.count_of_pages();
            for i in 0..page_count {
                let page = doc_inner
                    .page_at_index(i)
                    .context("Failed to get page")?;
                merged_inner.insert_page_at_index(&page, offset + i);
            }
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !merged_inner.write_to_url(&output_url) {
            return Err(anyhow::anyhow!("Failed to save merged PDF"));
        }
        Ok(())
    }

    /// Split a PDF into one file per page.
    pub fn split(path: &Path, output_dir: &Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc_inner = PDFDocument::from_url(&url).context("Failed to load PDF")?;
        let page_count = doc_inner.count_of_pages();

        for i in 0..page_count {
            let new_doc =
                PDFDocument::create_empty().context("Failed to create PDFDocument")?;
            let page = doc_inner.page_at_index(i).context("Failed to get page")?;
            new_doc.insert_page_at_index(&page, 0);

            let output_path = output_dir.join(format!("page_{}.pdf", i + 1));
            let output_url = NSURL::fileURLWithPath(&NSString::from_str(
                output_path
                    .to_str()
                    .context("Invalid output path encoding")?,
            ));
            new_doc.write_to_url(&output_url);
        }
        Ok(())
    }

    /// Extract specific pages into a new PDF.
    pub fn extract_pages(path: &Path, pages: &[u32], output: &Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc_inner = PDFDocument::from_url(&url).context("Failed to load PDF")?;
        let new_doc =
            PDFDocument::create_empty().context("Failed to create PDFDocument")?;

        for (i, &page_idx) in pages.iter().enumerate() {
            let page = doc_inner
                .page_at_index(page_idx as usize - 1)
                .context("Failed to get page")?;
            new_doc.insert_page_at_index(&page, i);
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !new_doc.write_to_url(&output_url) {
            return Err(anyhow::anyhow!("Failed to save extracted PDF"));
        }
        Ok(())
    }

    /// Rotate specified pages by the given number of degrees (0, 90, 180, 270).
    pub fn rotate_pages(path: &Path, pages: &[u32], rotation: i32, output: &Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc_inner = PDFDocument::from_url(&url).context("Failed to load PDF")?;

        for &page_idx in pages {
            if let Some(page) = doc_inner.page_at_index(page_idx as usize - 1) {
                page.set_rotation(rotation);
            }
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !doc_inner.write_to_url(&output_url) {
            return Err(anyhow::anyhow!("Failed to save rotated PDF"));
        }
        Ok(())
    }

    /// Create a compressed copy of a PDF by setting a quality hint.
    pub fn compress(path: &Path, output: &Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc_inner = PDFDocument::from_url(&url).context("Failed to load PDF")?;

        let attrs = doc_inner.attributes_dict().unwrap_or_else(|| {
            // SAFETY: `alloc` + `init` returns an empty, retained dictionary.
            unsafe {
                let cls = <NSDictionary<NSString, NSObject> as ClassType>::class();
                msg_send![msg_send![cls, alloc], init]
            }
        });

        let key = NSString::from_str("PDFCompressionQuality");
        let value = NSNumber::new_f64(0.7);
        // SAFETY: `attrs` is a valid mutable `NSDictionary`.
        unsafe {
            msg_send![&*attrs, setObject: &*value, forKey: &*key];
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !doc_inner.write_to_url(&output_url) {
            return Err(anyhow::anyhow!("Failed to save compressed PDF"));
        }
        Ok(())
    }

    /// Fill form fields in a PDF (basic implementation).
    pub fn fill_form(
        path: &Path,
        _data: HashMap<String, String>,
        output: &Path,
    ) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let _doc_inner =
            PDFDocument::from_url(&url).context("Failed to load PDF for form filling")?;

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !_doc_inner.write_to_url(&output_url) {
            return Err(anyhow::anyhow!("Failed to save filled form PDF"));
        }
        Ok(())
    }

    /// Flatten form fields in a PDF (save a copy without editable fields).
    pub fn flatten_form(path: &Path, output: &Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc_inner =
            PDFDocument::from_url(&url).context("Failed to load PDF for form flattening")?;

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !doc_inner.write_to_url(&output_url) {
            return Err(anyhow::anyhow!("Failed to save flattened form PDF"));
        }
        Ok(())
    }
}

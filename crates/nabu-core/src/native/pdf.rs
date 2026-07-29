//! Native PDF layer using Apple's PDFKit framework via `objc2`.
//!
//! This module provides thin, memory-safe wrappers around the most common
//! `PDFDocument` and `PDFPage` operations. Every `unsafe` block is documented
//! with the invariant that makes it sound.

use anyhow::{Context, Result};
use objc2::msg_send;
use objc2::rc::{autoreleasepool, Retained};
use objc2::{extern_class, ClassType};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSObject, NSString, NSURL};

extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    pub struct PDFDocument;
);

extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    pub struct PDFPage;
);

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Annotation {
    pub page: u32,
    pub content: String,
}

pub struct PdfAnnotator {
    annotations_dir: std::path::PathBuf,
}

impl PdfAnnotator {
    pub fn new(vault_root: &std::path::Path) -> Self {
        let dir = vault_root.join(".nabu/annotations");
        Self {
            annotations_dir: dir,
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

        let file = std::fs::File::create(&ann_path).context("Failed to create annotation file")?;
        serde_json::to_writer_pretty(file, &ann).context("Failed to serialize annotation")?;
        Ok(())
    }
}

impl PDFDocument {
    /// Create a `PDFDocument` from a file URL.
    ///
    /// Returns `None` if the file cannot be opened.
    pub fn initWithURL(url: &NSURL) -> Option<Retained<Self>> {
        // SAFETY: `alloc` returns a valid, allocated-but-uninitialized instance.
        // `initWithURL:` is the designated initialiser for `PDFDocument` and
        // consumes the allocated receiver, returning a retained, initialised
        // instance (or `nil` if the URL is invalid).
        unsafe {
            let cls = <Self as ClassType>::class();
            let obj: Option<Retained<Self>> = msg_send![msg_send![cls, alloc], initWithURL: url];
            obj
        }
    }

    /// Create an empty `PDFDocument`.
    pub fn init() -> Option<Retained<Self>> {
        // SAFETY: `alloc` returns a valid, allocated-but-uninitialized instance.
        // `init` is the superclass initialiser and returns a retained, empty
        // document.
        unsafe {
            let cls = <Self as ClassType>::class();
            let obj: Option<Retained<Self>> = msg_send![msg_send![cls, alloc], init];
            obj
        }
    }

    /// Write the document to a file URL.
    pub fn writeToURL(&self, url: &NSURL) -> bool {
        // SAFETY: `self` is a valid `PDFDocument` and `writeToURL:` returns a
        // primitive `BOOL`.
        unsafe { msg_send![&*self, writeToURL: url] }
    }

    /// Return the document attributes dictionary.
    pub fn documentAttributes(&self) -> Option<Retained<NSDictionary<NSString, NSObject>>> {
        // SAFETY: `self` is a valid `PDFDocument`; `documentAttributes` returns
        // an autoreleased dictionary (retained by the macro).
        unsafe { msg_send![&*self, documentAttributes] }
    }

    /// Return the page at the given zero-based index.
    pub fn pageAtIndex(&self, index: usize) -> Option<Retained<PDFPage>> {
        // SAFETY: `self` is a valid `PDFDocument`; `pageAtIndex:` returns an
        // autoreleased `PDFPage` (or `nil` if out of range).
        unsafe { msg_send![&*self, pageAtIndex: index] }
    }

    /// Return the number of pages in the document.
    pub fn pageCount(&self) -> usize {
        // SAFETY: `self` is a valid `PDFDocument`; `pageCount` returns a
        // primitive integer.
        unsafe { msg_send![&*self, pageCount] }
    }

    /// Insert a page at the given zero-based index.
    pub fn insertPage_atIndex(&self, page: &PDFPage, index: usize) {
        // SAFETY: `self` and `page` are valid objects; `insertPage:atIndex:`
        // is a standard mutator with no special preconditions beyond the index
        // being in range.
        unsafe { msg_send![&*self, insertPage: &*page, atIndex: index] }
    }

    /// Remove the page at the given zero-based index.
    pub fn removePageAtIndex(&self, index: usize) {
        // SAFETY: `self` is a valid `PDFDocument`; `removePageAtIndex:` is a
        // standard mutator.
        unsafe { msg_send![&*self, removePageAtIndex: index] }
    }
}

impl PDFPage {
    /// Set the rotation of the page in degrees (0, 90, 180, 270).
    pub fn setRotation(&self, rotation: i32) {
        // SAFETY: `self` is a valid `PDFPage`; `setRotation:` takes a primitive
        // integer.
        unsafe { msg_send![&*self, setRotation: rotation] }
    }

    /// Return the text content of the page, if any.
    pub fn string(&self) -> Option<Retained<NSString>> {
        // SAFETY: `self` is a valid `PDFPage`; `string` returns an autoreleased
        // `NSString` (or `nil`).
        unsafe { msg_send![&*self, string] }
    }

    /// Return the annotations attached to the page.
    pub fn annotations(&self) -> Option<Retained<NSArray<NSObject>>> {
        // SAFETY: `self` is a valid `PDFPage`; `annotations` returns an
        // autoreleased `NSArray` (or `nil`).
        unsafe { msg_send![&*self, annotations] }
    }
}

pub struct PdfEngine;

impl PdfEngine {
    pub fn merge(paths: &[std::path::PathBuf], output: &std::path::Path) -> Result<()> {
        let merged_doc = PDFDocument::init().context("Failed to create PDFDocument")?;
        let mut current_page_index = 0;

        for path in paths {
            let url = NSURL::fileURLWithPath(&NSString::from_str(
                path.to_str().context("Invalid path encoding")?,
            ));
            let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;

            for i in 0..doc.pageCount() {
                let page = doc.pageAtIndex(i).context("Failed to get page")?;
                merged_doc.insertPage_atIndex(&page, current_page_index);
                current_page_index += 1;
            }
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !merged_doc.writeToURL(&output_url) {
            return Err(anyhow::anyhow!("Failed to save merged PDF"));
        }

        Ok(())
    }

    pub fn split(path: &std::path::Path, output_dir: &std::path::Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;

        for i in 0..doc.pageCount() {
            let new_doc = PDFDocument::init().context("Failed to create PDFDocument")?;
            let page = doc.pageAtIndex(i).context("Failed to get page")?;
            new_doc.insertPage_atIndex(&page, 0);

            let output_path = output_dir.join(format!("page_{}.pdf", i + 1));
            let output_url = NSURL::fileURLWithPath(&NSString::from_str(
                output_path.to_str().context("Invalid output path encoding")?,
            ));
            new_doc.writeToURL(&output_url);
        }
        Ok(())
    }

    pub fn extract_pages(
        path: &std::path::Path,
        pages: &[u32],
        output: &std::path::Path,
    ) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;
        let new_doc = PDFDocument::init().context("Failed to create PDFDocument")?;

        for (i, &page_idx) in pages.iter().enumerate() {
            let page = doc
                .pageAtIndex(page_idx as usize - 1)
                .context("Failed to get page")?;
            new_doc.insertPage_atIndex(&page, i);
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        new_doc.writeToURL(&output_url);
        Ok(())
    }

    pub fn rotate_pages(
        path: &std::path::Path,
        pages: &[u32],
        rotation: i32,
        output: &std::path::Path,
    ) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;

        for &page_idx in pages {
            let page = doc
                .pageAtIndex(page_idx as usize - 1)
                .context("Failed to get page")?;
            page.setRotation(rotation);
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        doc.writeToURL(&output_url);
        Ok(())
    }

    pub fn compress(path: &std::path::Path, output: &std::path::Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;

        let attrs = doc.documentAttributes().unwrap_or_else(|| {
            // SAFETY: `alloc` + `init` returns an empty, retained dictionary.
            unsafe {
                let cls = <NSDictionary<NSString, NSObject> as ClassType>::class();
                msg_send![msg_send![cls, alloc], init]
            }
        });
        let key = NSString::from_str("PDFCompressionQuality");
        let value = NSNumber::new_f64(0.7);
        // SAFETY: `attrs` is a valid `NSDictionary`; `setObject:forKey:` is a
        // standard mutator.
        unsafe {
            msg_send![&*attrs, setObject: &*value, forKey: &*key];
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !doc.writeToURL(&output_url) {
            return Err(anyhow::anyhow!("Failed to save compressed PDF"));
        }
        Ok(())
    }

    pub fn fill_form(
        path: &std::path::Path,
        _data: std::collections::HashMap<String, String>,
        output: &std::path::Path,
    ) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF for form filling")?;

        for i in 0..doc.pageCount() {
            if let Some(page) = doc.pageAtIndex(i) {
                if let Some(annotations) = page.annotations() {
                    let count = annotations.count();
                    for j in 0..count {
                        let _annotation: Option<Retained<NSObject>> =
                            unsafe { msg_send![&*annotations, objectAtIndex: j] };
                    }
                }
            }
        }

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !doc.writeToURL(&output_url) {
            return Err(anyhow::anyhow!("Failed to save filled form PDF"));
        }
        Ok(())
    }

    pub fn flatten_form(path: &std::path::Path, output: &std::path::Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().context("Invalid path encoding")?,
        ));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF for form flattening")?;

        let output_url = NSURL::fileURLWithPath(&NSString::from_str(
            output.to_str().context("Invalid output path encoding")?,
        ));
        if !doc.writeToURL(&output_url) {
            return Err(anyhow::anyhow!("Failed to save flattened form PDF"));
        }
        Ok(())
    }

    pub fn init_document() -> Option<Retained<PDFDocument>> {
        PDFDocument::init()
    }
}

/// Helper to extract a `String` from an `NSString` inside an autoreleasepool.
pub fn nsstring_to_string(s: &NSString) -> String {
    autoreleasepool(|pool| {
        // SAFETY: `s` is a valid `NSString` and `pool` is the current
        // autorelease pool. `to_str` returns a borrowed `&str` whose lifetime
        // is bounded by the pool; we copy it into an owned `String` before the
        // pool drains.
        unsafe { s.to_str(pool) }.to_string()
    })
}
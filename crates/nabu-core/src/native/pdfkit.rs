//! macOS PDFKit engine: text extraction, metadata, annotations, rendering.
//!
//! `objc2` has no `objc2-pdfkit` crate in the registry cache, so the PDFKit
//! classes (`PDFDocument`, `PDFPage`, `PDFAnnotation`) are declared here with
//! typed `extern_class!` bindings — the same mechanism the generated objc2
//! crates use. All Objective-C types remain inside this module; the public
//! API below exposes only Rust-native values.

use super::error::NativeError;

/// Extracted text and page count of a PDF document.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfText {
    pub text: String,
    pub page_count: usize,
}

/// Document metadata extracted from PDFKit's `documentAttributes`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub page_count: usize,
}

/// A single PDF annotation with its type and contents.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfAnnotation {
    /// Annotation type string (e.g. "Text", "Highlight", "Link", "Widget").
    pub kind: String,
    /// Optional annotation payload text.
    pub contents: Option<String>,
    /// Zero-based page index the annotation lives on.
    pub page: usize,
}

/// Extract the concatenated text of a PDF document.
pub fn extract_text(data: &[u8]) -> Result<PdfText, NativeError> {
    imp::extract_text(data)
}

/// Extract document metadata (title, author, subject, creator, page count).
pub fn extract_metadata(data: &[u8]) -> Result<PdfMetadata, NativeError> {
    imp::extract_metadata(data)
}

/// Extract all annotations across all pages of a PDF document.
pub fn extract_annotations(data: &[u8]) -> Result<Vec<PdfAnnotation>, NativeError> {
    imp::extract_annotations(data)
}

/// Render a PDF page to a PNG image.
///
/// `page` is zero-based. `max_dimension` clamps the longer side of the
/// rendered image in pixels (aspect ratio preserved).
pub fn render_page_png(
    data: &[u8],
    page: usize,
    max_dimension: f64,
) -> Result<Vec<u8>, NativeError> {
    imp::render_page_png(data, page, max_dimension)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::encode::{Encode, Encoding, RefEncode};
    use objc2::ffi::{NSInteger, NSUInteger};
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{extern_class, extern_methods, AnyThread};
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSRect, NSSize, NSString};

    // ── PDFDisplayBox (NS_ENUM) ───────────────────────────────────────────

    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PDFDisplayBox(pub NSInteger);

    #[allow(non_upper_case_globals, dead_code)]
    impl PDFDisplayBox {
        pub const MediaBox: Self = Self(0);
        pub const CropBox: Self = Self(1);
        pub const BleedBox: Self = Self(2);
        pub const TrimBox: Self = Self(3);
        pub const ArtBox: Self = Self(4);
    }

    unsafe impl Encode for PDFDisplayBox {
        const ENCODING: Encoding = NSInteger::ENCODING;
    }
    unsafe impl RefEncode for PDFDisplayBox {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }

    // ── Framework linking ─────────────────────────────────────────────────
    //
    // objc2 framework crates emit this directive from their generated modules;
    // for the manual PDFKit bindings below we must link the framework
    // ourselves or class lookup panics at runtime.

    #[link(name = "PDFKit", kind = "framework")]
    extern "C" {}

    // ── Typed PDFKit class declarations ───────────────────────────────────

    extern_class!(
        #[unsafe(super(NSObject))]
        pub struct PDFDocument;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        pub struct PDFPage;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        pub struct PDFAnnotation;
    );

    #[allow(non_snake_case)]
    impl PDFDocument {
        extern_methods!(
            #[unsafe(method(initWithData:))]
            #[unsafe(method_family = init)]
            pub fn initWithData(this: Allocated<Self>, data: &NSData) -> Option<Retained<Self>>;

            #[unsafe(method(pageCount))]
            pub fn pageCount(&self) -> NSUInteger;

            #[unsafe(method(string))]
            pub fn string(&self) -> Option<Retained<NSString>>;

            #[unsafe(method(documentAttributes))]
            pub fn documentAttributes(&self)
                -> Option<Retained<NSDictionary<NSString, AnyObject>>>;

            #[unsafe(method(pageAtIndex:))]
            pub fn pageAtIndex(&self, index: NSUInteger) -> Option<Retained<PDFPage>>;
        );
    }

    #[allow(non_snake_case)]
    impl PDFPage {
        extern_methods!(
            #[unsafe(method(annotations))]
            pub fn annotations(&self) -> Retained<NSArray<PDFAnnotation>>;

            #[unsafe(method(boundsForBox:))]
            pub fn boundsForBox(&self, display_box: PDFDisplayBox) -> NSRect;

            #[unsafe(method(thumbnailOfSize:forBox:))]
            pub fn thumbnailOfSize_forBox(
                &self,
                size: NSSize,
                display_box: PDFDisplayBox,
            ) -> Retained<NSImage>;
        );
    }

    #[allow(non_snake_case)]
    impl PDFAnnotation {
        extern_methods!(
            #[unsafe(method(type))]
            pub fn type_(&self) -> Option<Retained<NSString>>;

            #[unsafe(method(contents))]
            pub fn contents(&self) -> Option<Retained<NSString>>;
        );
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn ns_string(v: &str) -> Retained<NSString> {
        NSString::from_str(v)
    }

    fn attr_str(attrs: &NSDictionary<NSString, AnyObject>, key: &str) -> Option<String> {
        let key = ns_string(key);
        let value = attrs.objectForKey(&key)?;
        // SAFETY: PDFKit attribute values are NSString objects; downcasting
        // is a plain is-kind-of check performed by objc2.
        let string = value.downcast::<NSString>().ok()?;
        Some(string.to_string())
    }

    fn with_document<T>(
        data: &[u8],
        f: impl FnOnce(&PDFDocument) -> Result<T, NativeError>,
    ) -> Result<T, NativeError> {
        let ns_data = NSData::with_bytes(data);
        let doc = PDFDocument::initWithData(PDFDocument::alloc(), &ns_data)
            .ok_or_else(|| NativeError::InvalidData("PDFKit could not open the document".into()))?;
        f(&doc)
    }

    pub fn extract_text(data: &[u8]) -> Result<PdfText, NativeError> {
        with_document(data, |doc| {
            let text = doc.string().map(|s| s.to_string()).unwrap_or_default();
            Ok(PdfText {
                text,
                page_count: doc.pageCount(),
            })
        })
    }

    pub fn extract_metadata(data: &[u8]) -> Result<PdfMetadata, NativeError> {
        with_document(data, |doc| {
            let page_count = doc.pageCount();
            let attrs = doc.documentAttributes();
            let mut meta = PdfMetadata {
                page_count,
                ..Default::default()
            };
            if let Some(attrs) = attrs {
                meta.title = attr_str(&attrs, "Title");
                meta.author = attr_str(&attrs, "Author");
                meta.subject = attr_str(&attrs, "Subject");
                meta.creator = attr_str(&attrs, "Creator");
            }
            Ok(meta)
        })
    }

    pub fn extract_annotations(data: &[u8]) -> Result<Vec<PdfAnnotation>, NativeError> {
        with_document(data, |doc| {
            let mut out = Vec::new();
            let page_count = doc.pageCount();
            for page in 0..page_count {
                let Some(page_obj) = doc.pageAtIndex(page) else {
                    continue;
                };
                for annotation in page_obj.annotations().iter() {
                    out.push(PdfAnnotation {
                        kind: annotation
                            .type_()
                            .map(|t| t.to_string())
                            .unwrap_or_default(),
                        contents: annotation.contents().map(|c| c.to_string()),
                        page,
                    });
                }
            }
            Ok(out)
        })
    }

    pub fn render_page_png(
        data: &[u8],
        page: usize,
        max_dimension: f64,
    ) -> Result<Vec<u8>, NativeError> {
        with_document(data, |doc| {
            let page_obj = doc
                .pageAtIndex(page as NSUInteger)
                .ok_or_else(|| NativeError::InvalidData(format!("page {page} out of range")))?;

            let bounds = page_obj.boundsForBox(PDFDisplayBox::MediaBox);
            let width = bounds.size.width;
            let height = bounds.size.height;
            if width <= 0.0 || height <= 0.0 {
                return Err(NativeError::InvalidData("page has empty bounds".into()));
            }

            let longest = width.max(height);
            let scale = (max_dimension / longest).min(1.0);
            let size = NSSize {
                width: (width * scale).max(1.0),
                height: (height * scale).max(1.0),
            };

            let image = page_obj.thumbnailOfSize_forBox(size, PDFDisplayBox::MediaBox);

            // SAFETY: encoding a freshly created image to PNG data; the
            // returned NSData is owned by us.
            let png = unsafe {
                let tiff = image.TIFFRepresentation().ok_or_else(|| {
                    NativeError::CallFailed("could not get TIFF representation".into())
                })?;
                let rep = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &tiff)
                    .ok_or_else(|| NativeError::CallFailed("could not create bitmap rep".into()))?;
                rep.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &NSDictionary::new(),
                )
            };
            let png = png.ok_or_else(|| NativeError::CallFailed("could not encode PNG".into()))?;
            Ok(png.to_vec())
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn extract_text(_data: &[u8]) -> Result<PdfText, NativeError> {
        Err(NativeError::UnsupportedPlatform)
    }
    pub fn extract_metadata(_data: &[u8]) -> Result<PdfMetadata, NativeError> {
        Err(NativeError::UnsupportedPlatform)
    }
    pub fn extract_annotations(_data: &[u8]) -> Result<Vec<PdfAnnotation>, NativeError> {
        Err(NativeError::UnsupportedPlatform)
    }
    pub fn render_page_png(
        _data: &[u8],
        _page: usize,
        _max_dimension: f64,
    ) -> Result<Vec<u8>, NativeError> {
        Err(NativeError::UnsupportedPlatform)
    }
}

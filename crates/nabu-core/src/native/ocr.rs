//! Native OCR layer using Apple's Vision framework via `objc2-vision`.
//!
//! This module provides real, production-quality OCR using the macOS Vision
//! framework's `VNRecognizeTextRequest`. It supports:
//!
//! - Images (PNG, JPEG, etc.)
//! - Screenshots
//! - Scanned PDFs (page-by-page rendering + OCR)
//! - Multiple pages
//! - Confidence values
//! - Structured text output
//!
//! Every `unsafe` block is documented with the invariant that makes it sound.

use anyhow::{Context, Result};
use objc2::msg_send;
use objc2::rc::{autoreleasepool, Retained};
use objc2::ClassType;
use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSString, NSURL};
use objc2_vision::{
    VNImageRequestHandler, VNRecognizedText, VNRecognizedTextObservation,
    VNRecognizeTextRequest, VNRequestTextRecognitionLevel,
};

/// A single recognized text region with its confidence score.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OcrTextRegion {
    /// The recognized text content.
    pub text: String,
    /// Confidence score in the range [0.0, 1.0] where 1.0 is most confident.
    pub confidence: f32,
}

/// The result of an OCR operation, containing all recognized text regions
/// and the concatenated full text.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OcrResult {
    /// All recognized text regions, in reading order.
    pub regions: Vec<OcrTextRegion>,
    /// All recognized text concatenated into a single string.
    pub full_text: String,
    /// The average confidence across all regions (0.0 if no regions).
    pub average_confidence: f32,
}

impl OcrResult {
    pub fn empty() -> Self {
        Self {
            regions: Vec::new(),
            full_text: String::new(),
            average_confidence: 0.0,
        }
    }

    pub fn from_regions(regions: Vec<OcrTextRegion>) -> Self {
        let full_text = regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let average_confidence = if regions.is_empty() {
            0.0
        } else {
            regions.iter().map(|r| r.confidence).sum::<f32>() / regions.len() as f32
        };

        Self {
            regions,
            full_text,
            average_confidence,
        }
    }
}

/// OCR engine backed by the macOS Vision framework.
pub struct OcrEngine {
    recognition_level: VNRequestTextRecognitionLevel,
    use_language_correction: bool,
}

impl Default for OcrEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OcrEngine {
    pub fn new() -> Self {
        Self {
            recognition_level: VNRequestTextRecognitionLevel::Accurate,
            use_language_correction: true,
        }
    }

    pub fn fast() -> Self {
        Self {
            recognition_level: VNRequestTextRecognitionLevel::Fast,
            use_language_correction: false,
        }
    }

    pub fn with_language_correction(mut self, enabled: bool) -> Self {
        self.use_language_correction = enabled;
        self
    }

    pub fn extract_text(&self, image_path: &str) -> Result<String> {
        let result = self.recognize_text_in_image(image_path)?;
        Ok(result.full_text)
    }

    pub fn extract_text_structured(&self, image_path: &str) -> Result<OcrResult> {
        self.recognize_text_in_image(image_path)
    }

    pub fn extract_text_from_data(&self, image_data: &[u8]) -> Result<OcrResult> {
        self.recognize_text_in_data(image_data)
    }

    pub fn extract_text_from_pdf(&self, pdf_path: &str) -> Result<OcrResult> {
        let mut all_regions = Vec::new();

        let doc =
            crate::native::pdf::PdfDocument::from_path(&std::path::PathBuf::from(pdf_path))
                .context("Failed to load PDF for OCR")?;

        let page_count = doc.page_count();
        for page_idx in 0..page_count {
            if let Some(page) = doc.page(page_idx) {
                if let Some(image_data) = render_page_to_image_data(&page) {
                    let page_result = self.recognize_text_in_data(&image_data)?;
                    all_regions.extend(page_result.regions);
                }
            }
        }

        Ok(OcrResult::from_regions(all_regions))
    }

    fn recognize_text_in_image(&self, image_path: &str) -> Result<OcrResult> {
        autoreleasepool(|_pool| {
            let url = NSURL::fileURLWithPath(&NSString::from_str(image_path));

            // SAFETY: `alloc` returns a valid allocated instance.
            // `initWithURL:options:` is the designated initialiser.
            let handler: Retained<VNImageRequestHandler> = unsafe {
                let cls = <VNImageRequestHandler as ClassType>::class();
                let empty_dict: Retained<NSDictionary<NSString, NSObject>> =
                    msg_send![msg_send![<NSDictionary<NSString, NSObject> as ClassType>::class(), alloc], init];
                msg_send![
                    msg_send![cls, alloc],
                    initWithURL: &*url,
                    options: &*empty_dict,
                ]
            };

            self.perform_ocr(&handler)
        })
    }

    fn recognize_text_in_data(&self, image_data: &[u8]) -> Result<OcrResult> {
        autoreleasepool(|_pool| {
            let nsdata = NSData::with_bytes(image_data);

            // SAFETY: `alloc` returns a valid allocated instance.
            // `initWithData:options:` is the designated initialiser.
            let handler: Retained<VNImageRequestHandler> = unsafe {
                let cls = <VNImageRequestHandler as ClassType>::class();
                let empty_dict: Retained<NSDictionary<NSString, NSObject>> =
                    msg_send![msg_send![<NSDictionary<NSString, NSObject> as ClassType>::class(), alloc], init];
                msg_send![
                    msg_send![cls, alloc],
                    initWithData: &*nsdata,
                    options: &*empty_dict,
                ]
            };

            self.perform_ocr(&handler)
        })
    }

    fn perform_ocr(&self, handler: &VNImageRequestHandler) -> Result<OcrResult> {
        // SAFETY: `alloc` + `init` returns a retained request.
        let request: Retained<VNRecognizeTextRequest> = unsafe {
            let cls = <VNRecognizeTextRequest as ClassType>::class();
            msg_send![msg_send![cls, alloc], init]
        };

        // SAFETY: `request` is a valid `VNRecognizeTextRequest`.
        unsafe {
            msg_send![&*request, setRecognitionLevel: self.recognition_level];
            msg_send![&*request, setUsesLanguageCorrection: self.use_language_correction];
        }

        let requests_array = NSArray::from_slice(&[&*request]);

        // SAFETY: `handler` is a valid `VNImageRequestHandler`.
        unsafe {
            handler
                .performRequests_error(&*requests_array)
                .map_err(|e| anyhow::anyhow!("Vision OCR failed: {}", nserror_to_string(&e)))?;
        }

        let observations = request.results().unwrap_or_else(|| {
            NSArray::from_slice(&[])
        });

        let mut regions = Vec::new();
        let count = observations.count();
        for i in 0..count {
            // SAFETY: `observations` is a valid `NSArray`; `objectAtIndex:`
            // returns a valid element. The index `i` is in range.
            let observation: &VNRecognizedTextObservation = unsafe {
                (&*observations)
                    .objectAtIndex(i)
                    .expect("observation must exist within bounds")
            };

            // SAFETY: `observation` is a valid `VNRecognizedTextObservation`.
            let candidates = unsafe { observation.topCandidates(1) };
            let candidate_count = candidates.count();
            if candidate_count > 0 {
                // SAFETY: `candidates` is a valid array; index 0 is in range.
                let text_recognition: &VNRecognizedText =
                    unsafe { (&*candidates).objectAtIndex(0).expect("candidate must exist") };

                // SAFETY: `text_recognition` is a valid `VNRecognizedText`.
                let text_nsstring = unsafe { text_recognition.string() };
                let confidence = unsafe { text_recognition.confidence() };

                let text = autoreleasepool(|pool| {
                    // SAFETY: `text_nsstring` is a valid `NSString` and `pool`
                    // is the current autorelease pool.
                    unsafe { text_nsstring.to_str(pool) }.to_string()
                });

                regions.push(OcrTextRegion {
                    text,
                    confidence: confidence.0 as f32,
                });
            }
        }

        Ok(OcrResult::from_regions(regions))
    }
}

/// Render a PdfPage to image data for OCR processing.
fn render_page_to_image_data(page: &crate::native::pdf::PdfPage) -> Option<Vec<u8>> {
    let inner = page.inner();
    // SAFETY: `inner` is a valid `PDFPage`.
    unsafe {
        let image: Option<Retained<objc2_foundation::NSObject>> =
            msg_send![&*inner, pageImage];

        image.and_then(|img| {
            let tiff_data: Option<Retained<NSData>> = msg_send![&*img, TIFFRepresentation];
            tiff_data.map(|data| {
                // SAFETY: `data` is a valid `NSData`; `bytes` returns a raw
                // pointer to the underlying buffer.
                let bytes_ptr: *const u8 = unsafe { msg_send![&*data, bytes] };
                let length: usize = unsafe { msg_send![&*data, length] };
                if bytes_ptr.is_null() || length == 0 {
                    Vec::new()
                } else {
                    unsafe { std::slice::from_raw_parts(bytes_ptr, length).to_vec() }
                }
            })
        })
    }
}

/// Convert an `NSError` to a human-readable string.
fn nserror_to_string(error: &NSError) -> String {
    autoreleasepool(|pool| {
        // SAFETY: `error` is a valid `NSError`.
        let desc: Retained<NSString> = unsafe { msg_send![&*error, localizedDescription] };
        // SAFETY: `desc` is a valid `NSString` and `pool` is the current pool.
        unsafe { desc.to_str(pool) }.to_string()
    })
}
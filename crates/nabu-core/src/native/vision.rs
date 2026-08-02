//! macOS Vision OCR (`VNRecognizeTextRequest`).
//!
//! The real implementation lives in the macOS-gated `imp` module and calls the
//! Vision framework through `objc2-vision`; other platforms get a graceful
//! error. No simulated OCR exists anywhere in this crate.

use super::error::NativeError;

/// A single line of recognized text with its confidence score.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f64,
}

/// Recognize text in an image using the macOS Vision framework.
///
/// Accepts raw image bytes (PNG, JPEG, HEIC, ...). Returns recognized text
/// lines sorted by Vision's confidence ordering.
pub fn recognize_text(image_data: &[u8]) -> Result<Vec<OcrResult>, NativeError> {
    imp::recognize_text(image_data)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRecognizedTextObservation, VNRequest,
        VNRequestTextRecognitionLevel,
    };

    pub fn recognize_text(image_data: &[u8]) -> Result<Vec<OcrResult>, NativeError> {
        let data = NSData::with_bytes(image_data);
        let options: Retained<NSDictionary<objc2_vision::VNImageOption, objc2::runtime::AnyObject>> =
            NSDictionary::new();

        // SAFETY: `initWithData_options` is a generated init that transfers
        // ownership; the image data is fully copied into an NSData.
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &data,
            &options,
        );

        // SAFETY: creating a fresh VNRecognizeTextRequest via its init.
        let request = unsafe { VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc()) };
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);

        // Downcast the concrete request to the `VNRequest` base type the
        // handler's requests array expects (a plain is-kind-of cast).
        let request_base: Retained<VNRequest> = request
            .clone()
            .downcast::<VNRequest>()
            .map_err(|_| NativeError::CallFailed("request type cast failed".into()))?;
        let requests: Retained<NSArray<VNRequest>> = NSArray::from_retained_slice(&[request_base]);

        handler
            .performRequests_error(&requests)
            .map_err(|e| NativeError::CallFailed(e.to_string()))?;

        let results = request.results();

        let mut out = Vec::new();
        if let Some(results) = results {
            for observation in results.iter() {
                // Only text observations carry recognized candidates.
                let Ok(text_observation) = observation
                    .clone()
                    .downcast::<VNRecognizedTextObservation>()
                else {
                    continue;
                };
                let candidates = text_observation.topCandidates(1);
                for candidate in candidates.iter() {
                    out.push(OcrResult {
                        text: candidate.string().to_string(),
                        confidence: candidate.confidence() as f64,
                    });
                }
            }
        }
        Ok(out)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn recognize_text(_image_data: &[u8]) -> Result<Vec<OcrResult>, NativeError> {
        Err(NativeError::UnsupportedPlatform)
    }
}

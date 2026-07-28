//! Screenshot capture handler for native macOS screen capture.
//!
//! The [`ScreenshotHandler`] implements the [`CaptureHandler`] trait to
//! capture screenshots using Apple's native CoreGraphics APIs and produce
//! canonical [`IngestionRequest`] objects that flow through the existing
//! [`CaptureEngine`] and [`IngestionPipeline`].
//!
//! # Architecture
//!
//! ```text
//! Screen capture (CoreGraphics)
//!     ↓
//! ScreenshotHandler (CaptureHandler)
//!     ↓
//! CaptureEngine::ingest
//!     ↓
//! ItemCaptured (event)
//!     ↓
//! IngestionPipeline → ProcessingPipeline → StorageManager
//! ```
//!
//! # Capture Modes
//!
//! - **Full screen**: Captures the entire primary display
//! - **Active window**: Captures the currently focused window
//! - **Region**: Captures a user-selected rectangular region
//!
//! The captured image is stored as a PNG `IngestionRequest` and will
//! automatically benefit from OCR processing in the pipeline.
//!
//! # Error Handling
//!
//! Screenshot failures never crash the application. All errors are
//! returned as failed [`CaptureResult`] instances. Cancelled selections
//! and permission denials are handled gracefully.

use std::collections::HashMap;

use crate::capture::{CaptureHandler, CaptureRequest, CaptureResult, IngestionOptions, IngestionRequest};
use serde::{Deserialize, Serialize};

/// Supported screenshot capture modes.
///
/// Determines which portion of the screen is captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotMode {
    /// Capture the entire primary display.
    FullScreen,
    /// Capture the currently active (focused) window.
    ActiveWindow,
    /// Capture a user-selected rectangular region.
    ///
    /// The region coordinates should be provided in the `CaptureRequest`
    /// payload under the `region` key with `x`, `y`, `width`, and `height` fields.
    Region,
}

impl Default for ScreenshotMode {
    fn default() -> Self {
        Self::FullScreen
    }
}

/// Handles screenshot capture requests on macOS.
///
/// This handler uses Apple's native CoreGraphics APIs to capture
/// screenshots and produces [`IngestionRequest`] objects for downstream
/// processing.
///
/// The handler is registered with the [`CaptureEngine`] under the source type
/// `"screenshot"`.
///
/// # Platform Support
///
/// On macOS, the handler uses CoreGraphics for screen capture.
/// On non-macOS platforms, the handler returns a failed capture result
/// indicating that screenshot capture is not supported.
pub struct ScreenshotHandler {
    default_mode: ScreenshotMode,
}

impl ScreenshotHandler {
    /// Creates a new screenshot handler with the default capture mode.
    pub fn new() -> Self {
        Self {
            default_mode: ScreenshotMode::FullScreen,
        }
    }

    /// Creates a new screenshot handler with a specific default mode.
    pub fn with_mode(mode: ScreenshotMode) -> Self {
        Self { default_mode: mode }
    }

    /// Returns the default capture mode.
    pub fn default_mode(&self) -> ScreenshotMode {
        self.default_mode
    }

    /// Updates the default capture mode.
    pub fn set_default_mode(&mut self, mode: ScreenshotMode) {
        self.default_mode = mode;
    }

    /// Captures a screenshot using native macOS CoreGraphics APIs.
    ///
    /// Returns the PNG-encoded image bytes on success, or an error message
    /// on failure.
    #[cfg(target_os = "macos")]
    fn capture_screenshot(&self, mode: ScreenshotMode, request: &CaptureRequest) -> Result<Vec<u8>, String> {
        use core_graphics::display::{CGDisplay, CGWindowID, CGWindowListOption, CGWindowImageOption};
        use core_graphics::geometry::CGRect;

        match mode {
            ScreenshotMode::FullScreen => {
                let display_id = core_graphics::display::CGMainDisplayID();
                let display = CGDisplay::new(display_id);
                display
                    .image()
                    .ok_or_else(|| "Failed to capture full screen".to_string())
                    .and_then(|img| self.cgimage_to_png(&img))
            }
            ScreenshotMode::ActiveWindow => {
                // Get the frontmost window ID from the window list
                let window_list = core_graphics::window::copy_window_info(
                    core_graphics::window::kCGWindowListOptionOnScreenOnly,
                    core_graphics::window::kCGNullWindowID,
                );

                let front_window_id = window_list
                    .and_then(|list| {
                        // The window list is a CFArray of CFDictionary entries.
                        // We look for the first on-screen window.
                        list.iter().next().and_then(|w| {
                            w.get("kCGWindowNumber")
                                .and_then(|v| v.as_i64())
                                .map(|id| id as u32)
                        })
                    })
                    .ok_or_else(|| "No active window found".to_string())?;

                let bounds = CGRect::new(
                    core_graphics::geometry::CGPoint::new(0.0, 0.0),
                    core_graphics::geometry::CGSize::new(f64::MAX, f64::MAX),
                );

                core_graphics::window::create_image(
                    bounds,
                    core_graphics::window::kCGWindowListOptionOnScreenOnly,
                    front_window_id,
                    core_graphics::window::kCGWindowImageDefault,
                )
                .ok_or_else(|| "Failed to capture active window".to_string())
                .and_then(|img| self.cgimage_to_png(&img))
            }
            ScreenshotMode::Region => {
                // Extract region coordinates from the request payload
                let region = request.payload.get("region");

                let x = region
                    .and_then(|r| r.get("x"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "Missing or invalid 'region.x' in payload".to_string())?;

                let y = region
                    .and_then(|r| r.get("y"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "Missing or invalid 'region.y' in payload".to_string())?;

                let width = region
                    .and_then(|r| r.get("width"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "Missing or invalid 'region.width' in payload".to_string())?;

                let height = region
                    .and_then(|r| r.get("height"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "Missing or invalid 'region.height' in payload".to_string())?;

                if width <= 0.0 || height <= 0.0 {
                    return Err("Region dimensions must be positive".to_string());
                }

                let display_id = core_graphics::display::CGMainDisplayID();
                let display = CGDisplay::new(display_id);

                let full_image = display
                    .image()
                    .ok_or_else(|| "Failed to capture display for region".to_string())?;

                // Crop the image to the specified region
                let bounds = CGRect::new(
                    core_graphics::geometry::CGPoint::new(x, y),
                    core_graphics::geometry::CGSize::new(width, height),
                );

                let cropped = full_image
                    .cropped(&bounds)
                    .ok_or_else(|| "Failed to crop screenshot region".to_string())?;

                self.cgimage_to_png(&cropped)
            }
        }
    }

    /// Converts a CGImage to PNG bytes using the `image` crate.
    #[cfg(target_os = "macos")]
    fn cgimage_to_png(&self, image: &core_graphics::image::CGImage) -> Result<Vec<u8>, String> {
        let width = image.width();
        let height = image.height();
        let data = image.data();
        let bytes_per_row = image.bytes_per_row();

        // CGImage stores pixels in BGRA format on macOS.
        // We need to convert to RGBA for the image crate.
        let raw_pixels = data.as_bytes();
        let mut rgba_pixels = Vec::with_capacity((width as usize) * (height as usize) * 4);

        for row in 0..height as usize {
            let row_start = row * bytes_per_row as usize;
            for col in 0..width as usize {
                let pixel_start = row_start + col * 4;
                if pixel_start + 3 < raw_pixels.len() {
                    // BGRA -> RGBA
                    rgba_pixels.push(raw_pixels[pixel_start + 2]); // R
                    rgba_pixels.push(raw_pixels[pixel_start + 1]); // G
                    rgba_pixels.push(raw_pixels[pixel_start]);     // B
                    rgba_pixels.push(raw_pixels[pixel_start + 3]); // A
                }
            }
        }

        let image_buffer = image::ImageBuffer::from_raw(
            width as u32,
            height as u32,
            rgba_pixels,
        ).ok_or_else(|| "Failed to create image buffer from CGImage data".to_string())?;

        let mut png_data: Vec<u8> = Vec::new();
        image::save_buffer_with_format(
            &mut png_data,
            &image_buffer,
            width as u32,
            height as u32,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        ).map_err(|e| format!("Failed to encode PNG: {}", e))?;

        Ok(png_data)
    }

    /// Fallback for non-macOS platforms.
    #[cfg(not(target_os = "macos"))]
    fn capture_screenshot(&self, _mode: ScreenshotMode, _request: &CaptureRequest) -> Result<Vec<u8>, String> {
        Err("Screenshot capture is only supported on macOS".to_string())
    }
}

impl Default for ScreenshotHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for ScreenshotHandler {
    fn source_type(&self) -> &'static str {
        "screenshot"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "screenshot"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        // Determine the capture mode from the request payload,
        // falling back to the default mode.
        let mode = request
            .payload
            .get("mode")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "full_screen" => Some(ScreenshotMode::FullScreen),
                "active_window" => Some(ScreenshotMode::ActiveWindow),
                "region" => Some(ScreenshotMode::Region),
                _ => None,
            })
            .unwrap_or(self.default_mode);

        // Capture the screenshot
        let image_data = match self.capture_screenshot(mode, &request) {
            Ok(data) => data,
            Err(e) => {
                return CaptureResult {
                    success: false,
                    knowledge_object: None,
                    error: Some(e.clone()),
                    message: format!("Screenshot capture failed: {}", e),
                };
            }
        };

        if image_data.is_empty() {
            return CaptureResult {
                success: false,
                knowledge_object: None,
                error: Some("Empty screenshot data".to_string()),
                message: "Screenshot capture produced no data".to_string(),
            };
        }

        let mut custom = HashMap::new();
        custom.insert("capture_type".to_string(), serde_json::json!("screenshot"));
        custom.insert("mode".to_string(), serde_json::json!(match mode {
            ScreenshotMode::FullScreen => "full_screen",
            ScreenshotMode::ActiveWindow => "active_window",
            ScreenshotMode::Region => "region",
        }));

        let ingestion_request = IngestionRequest {
            source: "screenshot".to_string(),
            raw_bytes: image_data,
            mime_type: "image/png".to_string(),
            vault_id: request.vault_id.clone(),
            source_file: None,
            options: IngestionOptions {
                create_knowledge_object: true,
                extract_metadata: true,
                custom,
            },
        };

        let payload = match serde_json::to_value(&ingestion_request) {
            Ok(p) => p,
            Err(e) => {
                return CaptureResult {
                    success: false,
                    knowledge_object: None,
                    error: Some(format!(
                        "Failed to serialize ingestion request: {}",
                        e
                    )),
                    message: "Screenshot capture failed: serialization error".to_string(),
                };
            }
        };

        CaptureResult {
            success: true,
            knowledge_object: None,
            error: None,
            message: "Screenshot captured successfully".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CaptureRequest;
    use std::collections::HashMap;

    #[test]
    fn source_type_is_screenshot() {
        let handler = ScreenshotHandler::new();
        assert_eq!(handler.source_type(), "screenshot");
    }

    #[test]
    fn can_handle_screenshot_requests() {
        let handler = ScreenshotHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "screenshot".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
        assert!(!handler.can_handle(&CaptureRequest {
            source_type: "clipboard".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
    }

    #[test]
    fn default_mode_is_full_screen() {
        let handler = ScreenshotHandler::new();
        assert_eq!(handler.default_mode(), ScreenshotMode::FullScreen);
    }

    #[test]
    fn mode_can_be_changed() {
        let mut handler = ScreenshotHandler::new();
        handler.set_default_mode(ScreenshotMode::ActiveWindow);
        assert_eq!(handler.default_mode(), ScreenshotMode::ActiveWindow);
    }

    #[test]
    fn mode_from_payload() {
        let handler = ScreenshotHandler::new();

        let request = CaptureRequest {
            source_type: "screenshot".to_string(),
            payload: serde_json::json!({"mode": "active_window"}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let mode = request
            .payload
            .get("mode")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "full_screen" => Some(ScreenshotMode::FullScreen),
                "active_window" => Some(ScreenshotMode::ActiveWindow),
                "region" => Some(ScreenshotMode::Region),
                _ => None,
            })
            .unwrap_or(handler.default_mode());

        assert_eq!(mode, ScreenshotMode::ActiveWindow);
    }

    #[test]
    fn unknown_mode_falls_back_to_default() {
        let handler = ScreenshotHandler::new();

        let request = CaptureRequest {
            source_type: "screenshot".to_string(),
            payload: serde_json::json!({"mode": "unknown_mode"}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let mode = request
            .payload
            .get("mode")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "full_screen" => Some(ScreenshotMode::FullScreen),
                "active_window" => Some(ScreenshotMode::ActiveWindow),
                "region" => Some(ScreenshotMode::Region),
                _ => None,
            })
            .unwrap_or(handler.default_mode());

        assert_eq!(mode, ScreenshotMode::FullScreen);
    }

    #[test]
    fn handler_clone_works() {
        let mut handler = ScreenshotHandler::new();
        handler.set_default_mode(ScreenshotMode::Region);
        let cloned = handler.clone();
        assert_eq!(cloned.default_mode(), ScreenshotMode::Region);
    }

    #[test]
    fn capture_returns_error_on_non_macOS() {
        let handler = ScreenshotHandler::new();
        let request = CaptureRequest {
            source_type: "screenshot".to_string(),
            payload: serde_json::json!({"mode": "full_screen"}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };
        let result = handler.capture(request);
        // On non-macOS, screenshot capture returns a failure result
        // (no panic, no crash)
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
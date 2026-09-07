//! macOS Vision framework OCR (Phase 4 native integration).
//!
//! Runs on-device text recognition through Apple's JXA (JavaScript for
//! Automation) bridge, which can import the Vision and Quartz frameworks
//! directly. No new crates, no network calls, works on macOS 11+.

use std::path::Path;

use crate::hidden_command;

#[cfg(target_os = "macos")]
const OCR_IMAGE_SCRIPT: &str = r#"
ObjC.import('Vision');
ObjC.import('AppKit');

function run(argv) {
  const path = argv[0];
  const data = $.NSData.dataWithContentsOfFile(path);
  if (data.isNil()) return JSON.stringify({ ok: false, error: 'Cannot read image file' });

  const handler = $.VNImageRequestHandler.alloc.initWithDataOptions(data, $());
  const request = $.VNRecognizeTextRequest.alloc.init;
  request.recognitionLevel = $.VNRequestTextRecognitionLevelAccurate;
  request.usesLanguageCorrection = true;

  const error = Ref();
  const success = handler.performRequestsError($([request]), error);
  if (!success) {
    const detail = error[0] && !error[0].isNil()
      ? error[0].localizedDescription.js
      : 'Unknown Vision error';
    return JSON.stringify({ ok: false, error: detail });
  }

  const results = request.results;
  if (results.isNil()) return JSON.stringify({ ok: false, error: 'No text recognized' });
  const lines = [];
  for (let i = 0; i < results.count; i += 1) {
    const observation = results.objectAtIndex(i);
    const candidates = observation.topCandidates(1);
    if (candidates.count > 0) lines.push(candidates.objectAtIndex(0).string.js);
  }
  return JSON.stringify({ ok: true, text: lines.join('\n') });
}
"#;

#[cfg(target_os = "macos")]
const PDF_PNG_SCRIPT: &str = r#"
ObjC.import('Quartz');
ObjC.import('AppKit');

function run(argv) {
  const path = argv[0];
  const outDir = argv[1];
  const url = $.NSURL.fileURLWithPath(path);
  const doc = $.PDFDocument.alloc.initWithURL(url);
  if (doc.isNil()) return JSON.stringify({ ok: false, error: 'Cannot open PDF' });

  const written = [];
  const count = doc.pageCount;
  for (let i = 0; i < count; i += 1) {
    const page = doc.pageAtIndex(i);
    const image = page.thumbnailOfSizeForBox($.NSMakeSize(1600, 2200), $.kPDFDisplayBoxMediaBox);
    const tiff = image.TIFFRepresentation;
    const rep = $.NSBitmapImageRep.imageRepWithData(tiff);
    const png = rep.representationUsingTypeProperties($.NSBitmapImageFileTypePNG, $());
    const out = outDir + '/page-' + (i + 1) + '.png';
    const ok = png.writeToFileAtomically(out, true);
    if (!ok) return JSON.stringify({ ok: false, error: 'Failed to write ' + out });
    written.push(out);
  }
  return JSON.stringify({ ok: true, pages: written });
}
"#;

#[cfg(target_os = "macos")]
fn run_jxa(script: &str, args: &[&str]) -> Result<String, String> {
    let output = hidden_command("/usr/bin/osascript")
        .arg("-l")
        .arg("JavaScript")
        .arg("-e")
        .arg(script)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to run Vision OCR bridge: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Vision OCR bridge failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
fn parse_json_object(raw: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(raw).map_err(|error| format!("Vision OCR bridge returned invalid output: {error}"))
}

/// OCR for a single image file. Returns the recognized text.
#[cfg(target_os = "macos")]
pub fn extract_text_from_image(path: &Path) -> Result<String, String> {
    let raw = run_jxa(OCR_IMAGE_SCRIPT, &[&path.to_string_lossy()])?;
    let parsed = parse_json_object(&raw)?;
    if parsed["ok"].as_bool() != Some(true) {
        return Err(parsed["error"].as_str().unwrap_or("Vision OCR failed").to_string());
    }
    let text = parsed["text"].as_str().unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err("No text recognized in image".into());
    }
    Ok(text)
}

/// OCR for a PDF: rasterize pages through PDFKit, run text recognition on
/// each page image, and join the results as Markdown sections.
#[cfg(target_os = "macos")]
pub fn extract_text_from_pdf(path: &Path) -> Result<String, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix("nabu-ocr-")
        .tempdir()
        .map_err(|error| format!("Failed to create temp directory: {error}"))?;
    let out_dir = temp_dir.path().to_string_lossy().into_owned();
    let raw = run_jxa(PDF_PNG_SCRIPT, &[&path.to_string_lossy(), out_dir.as_str()])?;
    let parsed = parse_json_object(&raw)?;
    if parsed["ok"].as_bool() != Some(true) {
        return Err(parsed["error"].as_str().unwrap_or("Vision OCR failed").to_string());
    }
    let pages = parsed["pages"]
        .as_array()
        .ok_or_else(|| "Vision OCR bridge returned no pages".to_string())?;
    if pages.is_empty() {
        return Err("PDF has no pages".into());
    }

    let mut sections = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let page_path = page.as_str().unwrap_or_default();
        let text = extract_text_from_image(Path::new(page_path)).unwrap_or_default();
        sections.push(format!("## Page {}\n\n{}", index + 1, text));
    }
    let combined = sections.join("\n\n").trim().to_string();
    if combined.is_empty() {
        return Err("No text recognized in PDF".into());
    }
    Ok(combined)
}

#[cfg(not(target_os = "macos"))]
pub fn extract_text_from_image(_path: &Path) -> Result<String, String> {
    Err("Vision OCR is only available on macOS".into())
}

#[cfg(not(target_os = "macos"))]
pub fn extract_text_from_pdf(_path: &Path) -> Result<String, String> {
    Err("Vision OCR is only available on macOS".into())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn missing_image_is_reported_not_panicked() {
        let error = extract_text_from_image(Path::new("/nonexistent/nabu-ocr-test.png"))
            .expect_err("missing file must error");
        assert!(!error.is_empty());
    }

    #[test]
    fn missing_pdf_is_reported_not_panicked() {
        let error = extract_text_from_pdf(Path::new("/nonexistent/nabu-ocr-test.pdf"))
            .expect_err("missing file must error");
        assert!(!error.is_empty());
    }
}

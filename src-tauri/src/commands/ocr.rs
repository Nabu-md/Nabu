use std::path::Path;

/// Extracts text from an image file using the macOS Vision framework.
/// Fully offline; returns an error when the platform is unsupported or no
/// text is recognized.
#[tauri::command]
pub fn ocr_extract_text(image_path: String) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let expanded = crate::commands::expand_tilde(&image_path);
        let path = Path::new(expanded.as_ref());
        if !path.is_file() {
            return Err(format!("Image not found: {image_path}"));
        }
        crate::ocr::extract_text_from_image(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = image_path;
        Err("Vision OCR is only available on macOS".into())
    }
}

/// Extracts text from a PDF by rasterizing each page and running Vision OCR
/// page-by-page. Returns combined Markdown text with page separators.
#[tauri::command]
pub fn ocr_extract_text_from_pdf(pdf_path: String) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let expanded = crate::commands::expand_tilde(&pdf_path);
        let path = Path::new(expanded.as_ref());
        if !path.is_file() {
            return Err(format!("PDF not found: {pdf_path}"));
        }
        crate::ocr::extract_text_from_pdf(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pdf_path;
        Err("Vision OCR is only available on macOS".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn ocr_rejects_missing_files() {
        assert!(ocr_extract_text("/nonexistent/image.png".into()).is_err());
        assert!(ocr_extract_text_from_pdf("/nonexistent/doc.pdf".into()).is_err());
    }
}

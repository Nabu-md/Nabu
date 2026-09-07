//! Document conversion via anydoc (MIT, offline). One-way: documents → Markdown.
//! Scanned PDFs surface a structured `needsOcr` error so the frontend can chain
//! into macOS Vision OCR.

use serde::Serialize;

/// Supported input formats reported to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedFormat {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Structured conversion error. `code: "needsOcr"` tells the frontend to run
/// Vision OCR and merge the result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertCommandError {
    pub code: String,
    pub message: String,
    pub pages: Vec<u32>,
    pub page_count: u32,
}

impl From<anydoc::ConvertError> for ConvertCommandError {
    fn from(err: anydoc::ConvertError) -> Self {
        match err {
            anydoc::ConvertError::NeedsOcr { pages, page_count } => Self {
                code: "needsOcr".into(),
                message: "Scanned PDF pages need OCR".into(),
                pages,
                page_count,
            },
            other => Self {
                code: "unsupported".into(),
                message: other.to_string(),
                pages: Vec::new(),
                page_count: 0,
            },
        }
    }
}

/// Convert any supported document to clean Markdown.
#[tauri::command]
pub fn convert_to_markdown(file_path: String) -> Result<String, ConvertCommandError> {
    anydoc::to_markdown(&file_path).map_err(ConvertCommandError::from)
}

/// Enumerate input formats anydoc supports.
#[tauri::command]
pub fn list_supported_formats() -> Vec<SupportedFormat> {
    vec![
        SupportedFormat {
            name: "PDF".into(),
            extensions: vec!["pdf".into()],
        },
        SupportedFormat {
            name: "Word".into(),
            extensions: vec!["doc".into(), "docx".into(), "docm".into()],
        },
        SupportedFormat {
            name: "OpenDocument Text".into(),
            extensions: vec!["odt".into()],
        },
        SupportedFormat {
            name: "PowerPoint".into(),
            extensions: vec![
                "ppt".into(),
                "pptx".into(),
                "pptm".into(),
                "ppsx".into(),
                "pot".into(),
            ],
        },
        SupportedFormat {
            name: "Rich Text".into(),
            extensions: vec!["rtf".into()],
        },
        SupportedFormat {
            name: "EPUB".into(),
            extensions: vec!["epub".into()],
        },
        SupportedFormat {
            name: "Excel".into(),
            extensions: vec!["xlsx".into(), "xlsm".into(), "xlsb".into(), "xls".into()],
        },
        SupportedFormat {
            name: "OpenDocument Spreadsheet".into(),
            extensions: vec!["ods".into()],
        },
        SupportedFormat {
            name: "OpenDocument Presentation".into(),
            extensions: vec!["odp".into()],
        },
        SupportedFormat {
            name: "CSV".into(),
            extensions: vec!["csv".into()],
        },
    ]
}

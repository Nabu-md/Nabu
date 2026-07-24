mod markdown;
pub use markdown::{parse, Document, ParseError};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      let _ = tauri::WebviewWindowBuilder::new(app, "dictation-pill", tauri::WebviewUrl::App("dictation-pill.html".into())).build()?;
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

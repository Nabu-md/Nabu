mod markdown;
pub use markdown::{parse, Document, ParseError};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      let window = tauri::window::WindowBuilder::new(app, "dictation-pill")
        .title("Dictation")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .inner_size(200.0, 50.0)
        .build()?;
      
      let _webview = tauri::webview::WebviewBuilder::new(
        app,
        "dictation-pill-webview",
        tauri::webview::WebviewUrl::App("dictation-pill.html".into()),
      )
      .build()?;
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

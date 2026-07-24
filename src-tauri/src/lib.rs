mod markdown;
pub use markdown::{parse, Document, ParseError};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| if cfg!(debug_assertions) {
      app.handle().plugin(
        tauri_plugin_log::Builder::default()
          .level(log::LevelFilter::Info)
          .build(),
      )?;
      tauri::WebviewWindowBuilder::new(
        app,
        "dictation-pill",
        tauri::WebviewUrl::App("dictation-pill.html".into()),
      )
      .title("Dictation")
      .transparent(true)
      .decorations(false)
      .always_on_top(true)
      .build()?;
      Ok(())
    } else {
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

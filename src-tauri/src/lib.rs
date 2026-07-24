pub mod commands;
pub mod models;
pub mod settings;
pub mod vault;
pub mod template_manager;
pub mod export_engine;
pub mod native;

mod markdown;
pub use markdown::{parse, Document, ParseError};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      crate::commands::complete_setup,
      crate::commands::open_settings,
      crate::commands::note_create_file,
      crate::commands::note_daily,
      crate::commands::get_settings,
      crate::commands::settings_set_all
    ])
    .setup(|app| {
      let _ = tauri::WebviewWindowBuilder::new(app, "dictation-pill", tauri::WebviewUrl::App("dictation-pill.html".into())).build()?;
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

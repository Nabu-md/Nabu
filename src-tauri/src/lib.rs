mod commands;
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
      crate::commands::vault_open,
      crate::commands::note_create_file,
      crate::commands::note_delete_file,
      crate::commands::note_rename_file,
      crate::commands::note_daily,
      crate::commands::start_dictation,
      crate::commands::settings_get,
      crate::commands::settings_set,
      crate::commands::settings_get_feature_toggles,
      crate::commands::settings_set_feature_toggle,
      crate::commands::open_settings,
      crate::commands::settings_get_all,
      crate::commands::settings_set_all,
      crate::commands::stage_files,
      crate::commands::search,
      crate::commands::complete_setup,
      crate::commands::get_graph_data,
      crate::commands::export_note,
      crate::commands::filter_graph_by_tag,
      crate::commands::annotate_pdf,
      crate::commands::run_ocr,
      crate::commands::run_dictation
    ])
    .setup(|app| {
      let _ = tauri::WebviewWindowBuilder::new(app, "dictation-pill", tauri::WebviewUrl::App("dictation-pill.html".into())).build()?;
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

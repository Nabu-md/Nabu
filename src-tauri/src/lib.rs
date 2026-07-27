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
  let settings_path = std::env::var("NABU_SETTINGS_PATH")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|_| {
      dirs_next().join("nabu").join("settings.json")
    });

  let settings_store = crate::settings::SettingsStore::load(&settings_path)
    .unwrap_or_else(|_| crate::settings::SettingsStore::new(settings_path));

  tauri::Builder::default()
    .manage(settings_store)
    .invoke_handler(tauri::generate_handler![
      crate::commands::check_vault_exists,
      crate::commands::get_current_vault,
      crate::commands::select_vault_dialog,
      crate::commands::create_vault_dialog,
      crate::commands::open_dictation_pill,
      crate::commands::close_dictation_pill,
      crate::commands::toggle_dictation_pill,
      crate::commands::start_dictation,
      crate::commands::stop_dictation,
      crate::commands::complete_setup,
      crate::commands::open_settings,
      crate::commands::note_create_file,
      crate::commands::note_daily,
      crate::commands::get_settings,
      crate::commands::settings_get,
      crate::commands::settings_set,
      crate::commands::settings_set_all
    ])
    .setup(|_app| {
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

fn dirs_next() -> std::path::PathBuf {
  std::env::var("HOME")
    .or_else(|_| std::env::var("USERPROFILE"))
    .map(|h| std::path::PathBuf::from(h).join(".config"))
    .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

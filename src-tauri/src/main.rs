// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // CLI subcommands (e.g. `nabu research "query" --depth 3`) run the
    // backend logic directly without launching the Tauri app.
    if std::env::args().nth(1).as_deref() == Some("research") {
        std::process::exit(match nabu_lib::run_research_cli() {
            Ok(()) => 0,
            Err(message) => {
                eprintln!("{message}");
                1
            }
        });
    }

    nabu_lib::run();
}
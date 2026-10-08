mod commands;
pub mod secrets;
pub mod settings;
mod state;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // A machine without a keychain still runs; saving a key then explains why it can't.
            if let Err(e) = secrets::use_native_store() {
                eprintln!("{e}");
            }
            let config_dir = app.path().app_config_dir()?;
            app.manage(state::AppState::new(config_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_drawing,
            commands::drawing_info,
            commands::set_sheet,
            commands::set_layer_hidden,
            commands::render_batch,
            commands::pick,
            commands::entity_info,
            commands::run_command,
            commands::undo,
            commands::redo,
            commands::takeoff,
            commands::totals,
            commands::save_project,
            commands::save_drawing,
            commands::save_drawing_as,
            commands::export_takeoff,
            commands::get_settings,
            commands::set_settings,
            commands::set_api_key,
            commands::has_api_key,
            commands::delete_api_key,
            commands::app_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Qiyas");
}

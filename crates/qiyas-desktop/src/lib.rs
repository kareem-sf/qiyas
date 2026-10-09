mod ai;
mod commands;
mod mcp;
#[cfg(debug_assertions)]
mod mock;
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
            mcp::restart(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_drawing,
            commands::drawing_info,
            commands::set_sheet,
            commands::set_layer_hidden,
            commands::batch_version,
            commands::render_batch,
            commands::atlas_pages,
            commands::pick,
            commands::snap,
            commands::get_drafting,
            commands::set_drafting,
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
            commands::import_boq,
            commands::boq,
            commands::compare,
            commands::conflicts,
            commands::export_conflicts,
            commands::get_settings,
            commands::set_settings,
            commands::set_api_key,
            commands::has_api_key,
            commands::delete_api_key,
            commands::app_dir,
            commands::mcp_status,
            commands::ai_send,
            commands::ai_cancel,
            commands::ai_clear,
            commands::ai_status,
            commands::ai_set_language,
            commands::ai_undo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Qiyas");
}

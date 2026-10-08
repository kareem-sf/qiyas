mod commands;
mod state;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
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
            commands::export_takeoff,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Qiyas");
}

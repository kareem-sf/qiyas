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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Qiyas");
}

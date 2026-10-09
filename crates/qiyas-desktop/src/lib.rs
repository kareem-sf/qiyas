mod ai;
mod commands;
mod mcp;
#[cfg(debug_assertions)]
mod mock;
mod pdf;
pub mod secrets;
pub mod settings;
mod state;
mod tabs;

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
            pdf::use_bundled_pdfium(app.handle());
            mcp::restart(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_drawing,
            commands::recents,
            commands::clear_recents,
            commands::views,
            tabs::tabs,
            tabs::activate_tab,
            tabs::close_tab,
            commands::drawing_info,
            commands::set_sheet,
            commands::set_layer_hidden,
            commands::reload_xrefs,
            commands::set_xref_path,
            commands::batch_version,
            commands::render_batch,
            commands::atlas_pages,
            commands::pick,
            commands::boundary_at,
            commands::pick_window,
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
            commands::export_table,
            commands::import_boq,
            commands::boq,
            commands::compare,
            commands::conflicts,
            commands::export_conflicts,
            commands::get_settings,
            commands::set_settings,
            commands::aliases,
            commands::set_api_key,
            commands::has_api_key,
            commands::delete_api_key,
            commands::app_dir,
            commands::mcp_status,
            commands::ai_send,
            commands::ai_cancel,
            commands::ai_clear,
            commands::ai_status,
            commands::ai_log,
            commands::ai_set_language,
            commands::ai_undo,
            pdf::pdf_page_count,
            pdf::pdf_page_size,
            pdf::pdf_thumbnails,
            pdf::attach_pdf,
            pdf::is_dirty,
            pdf::calibrate,
            pdf::underlay_image,
            pdf::image_size,
            pdf::attach_image,
            pdf::image_display,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Qiyas");
}

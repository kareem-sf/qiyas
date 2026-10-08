//! IPC commands for the frontend. Each one locks the session, does its work and maps errors to text.
//! All are `async` so Tauri runs them off the main thread and a slow open or batch never freezes
//! the window. None awaits, so no lock is held across an `.await`.

use std::path::Path;
use std::sync::PoisonError;

use qiyas_core::{
    Command, CoreError, DrawingInfo, EntityInfo, Measurement, Outcome, Session, Total,
};
use tauri::ipc::Response;
use tauri::State;

use crate::state::AppState;

/// Runs `f` on the open session, or fails with "No drawing is open".
fn with_session<R>(
    state: &AppState,
    f: impl FnOnce(&mut Session) -> Result<R, CoreError>,
) -> Result<R, String> {
    // A panic in an earlier command leaves the session as it was; keep using it.
    let mut guard = state.0.lock().unwrap_or_else(PoisonError::into_inner);
    let session = guard.as_mut().ok_or("No drawing is open")?;
    f(session).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_drawing(path: String, state: State<'_, AppState>) -> Result<DrawingInfo, String> {
    // Read the file before taking the lock, so a slow open doesn't block other commands.
    let session = Session::open(Path::new(&path)).map_err(|e| e.to_string())?;
    let info = session.info();
    *state.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(session);
    Ok(info)
}

#[tauri::command]
pub async fn drawing_info(state: State<'_, AppState>) -> Result<DrawingInfo, String> {
    with_session(&state, |s| Ok(s.info()))
}

#[tauri::command]
pub async fn set_sheet(block: String, state: State<'_, AppState>) -> Result<(), String> {
    with_session(&state, |s| s.set_sheet(&block))
}

#[tauri::command]
pub async fn set_layer_hidden(
    name: String,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_session(&state, |s| {
        s.set_layer_hidden(&name, hidden);
        Ok(())
    })
}

/// The current sheet's line batch, in the binary layout of `Batch::to_bytes`.
#[tauri::command]
pub async fn render_batch(state: State<'_, AppState>) -> Result<Response, String> {
    with_session(&state, |s| Ok(Response::new(s.batch().to_bytes())))
}

#[tauri::command]
pub async fn pick(
    x: f64,
    y: f64,
    tol: f64,
    state: State<'_, AppState>,
) -> Result<Option<u64>, String> {
    with_session(&state, |s| Ok(s.pick([x, y], tol)))
}

#[tauri::command]
pub async fn entity_info(handle: u64, state: State<'_, AppState>) -> Result<EntityInfo, String> {
    with_session(&state, |s| s.entity_info(handle))
}

/// Runs one command on the command bus; see `qiyas_core::Command`.
#[tauri::command]
pub async fn run_command(cmd: Command, state: State<'_, AppState>) -> Result<Outcome, String> {
    with_session(&state, |s| s.run(cmd))
}

/// Undoes the last take-off change; false when there was none.
#[tauri::command]
pub async fn undo(state: State<'_, AppState>) -> Result<bool, String> {
    with_session(&state, |s| Ok(s.undo()))
}

/// Redoes the last undone take-off change; false when there was none.
#[tauri::command]
pub async fn redo(state: State<'_, AppState>) -> Result<bool, String> {
    with_session(&state, |s| Ok(s.redo()))
}

#[tauri::command]
pub async fn takeoff(state: State<'_, AppState>) -> Result<Vec<Measurement>, String> {
    with_session(&state, |s| Ok(s.takeoff.items.clone()))
}

#[tauri::command]
pub async fn totals(state: State<'_, AppState>) -> Result<Vec<Total>, String> {
    with_session(&state, |s| Ok(s.takeoff.totals()))
}

/// Saves the take-off to the drawing's `.qiyas` project file; returns the path written.
#[tauri::command]
pub async fn save_project(state: State<'_, AppState>) -> Result<String, String> {
    with_session(&state, |s| Ok(s.save_project()?.display().to_string()))
        .map_err(|e| format!("The take-off could not be saved: {e}"))
}

/// Saves the drawing over its own file, measurements embedded, and its project file.
#[tauri::command]
pub async fn save_drawing(state: State<'_, AppState>) -> Result<(), String> {
    with_session(&state, |s| s.save())
}

/// Saves the drawing to `path` (.dwg or .dxf), measurements embedded, with its project file next
/// to it; the session then works on the new file.
#[tauri::command]
pub async fn save_drawing_as(path: String, state: State<'_, AppState>) -> Result<(), String> {
    with_session(&state, |s| s.save_as(Path::new(&path)))
}

/// Writes the take-off and its totals to an Excel workbook at `path`.
#[tauri::command]
pub async fn export_takeoff(path: String, state: State<'_, AppState>) -> Result<(), String> {
    // Copy the take-off out, so writing the file doesn't hold the lock.
    let (items, totals, title) = with_session(&state, |s| {
        let title = match s.drawing.path.file_name() {
            Some(name) => format!("{} take-off", name.to_string_lossy()),
            None => "Take-off".to_string(),
        };
        Ok((s.takeoff.items.clone(), s.takeoff.totals(), title))
    })?;
    qiyas_boq::export_takeoff(Path::new(&path), &items, &totals, &title).map_err(|e| e.to_string())
}

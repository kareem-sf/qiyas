//! IPC commands for the frontend. Each one locks the session, does its work and maps errors to text.

use std::path::Path;
use std::sync::PoisonError;

use qiyas_core::{CoreError, DrawingInfo, EntityInfo, Session};
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
pub fn open_drawing(path: String, state: State<'_, AppState>) -> Result<DrawingInfo, String> {
    // Read the file before taking the lock, so a slow open doesn't block other commands.
    let session = Session::open(Path::new(&path)).map_err(|e| e.to_string())?;
    let info = session.info();
    *state.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(session);
    Ok(info)
}

#[tauri::command]
pub fn drawing_info(state: State<'_, AppState>) -> Result<DrawingInfo, String> {
    with_session(&state, |s| Ok(s.info()))
}

#[tauri::command]
pub fn set_sheet(block: String, state: State<'_, AppState>) -> Result<(), String> {
    with_session(&state, |s| s.set_sheet(&block))
}

#[tauri::command]
pub fn set_layer_hidden(
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
pub fn render_batch(state: State<'_, AppState>) -> Result<Response, String> {
    with_session(&state, |s| Ok(Response::new(s.batch().to_bytes())))
}

#[tauri::command]
pub fn pick(x: f64, y: f64, tol: f64, state: State<'_, AppState>) -> Result<Option<u64>, String> {
    with_session(&state, |s| Ok(s.pick([x, y], tol)))
}

#[tauri::command]
pub fn entity_info(handle: u64, state: State<'_, AppState>) -> Result<EntityInfo, String> {
    with_session(&state, |s| s.entity_info(handle))
}

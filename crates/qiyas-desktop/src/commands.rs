//! IPC commands for the frontend. Each one locks the session, does its work and maps errors to text.
//! All are `async` so Tauri runs them off the main thread and a slow open or batch never freezes
//! the window. None awaits, so no lock is held across an `.await`.

use std::path::Path;
use std::sync::{MutexGuard, PoisonError};

use qiyas_ai::prompt::Lang;
use qiyas_ai::{Agent, AgentEvent};
use qiyas_core::{
    Boq, Command, Conflict, CoreError, Drafting, DrawingInfo, EntityInfo, Measurement, Outcome,
    Picked, Session, Snap, Source, Total,
};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::mpsc;

use crate::ai::{self, AiRun, Turn};
use crate::mcp::{self, McpStatus};
use crate::secrets;
use crate::settings::{self, Settings};
use crate::state::AppState;

/// Runs `f` on the open session, or fails with "No drawing is open".
pub(crate) fn with_session<R>(
    state: &AppState,
    f: impl FnOnce(&mut Session) -> Result<R, CoreError>,
) -> Result<R, String> {
    // A panic in an earlier command leaves the session as it was; keep using it.
    let mut guard = state.session.lock().unwrap_or_else(PoisonError::into_inner);
    let session = guard.as_mut().ok_or("No drawing is open")?;
    f(session).map_err(|e| e.to_string())
}

/// Opens a drawing; the assistant starts a new conversation about it.
#[tauri::command]
pub async fn open_drawing<R: Runtime>(
    path: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<DrawingInfo, String> {
    // Read the file before taking the lock, so a slow open doesn't block other commands.
    let session = Session::open(Path::new(&path)).map_err(|e| e.to_string())?;
    let info = session.info();
    // Stop a running turn first, so it can't run another tool on the new drawing.
    new_conversation(&app, &state);
    *state.session.lock().unwrap_or_else(PoisonError::into_inner) = Some(session);
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

/// Looks for every xref again and reopens it; returns the drawing's info with the new list.
#[tauri::command]
pub async fn reload_xrefs(state: State<'_, AppState>) -> Result<DrawingInfo, String> {
    with_session(&state, |s| {
        s.reload_xrefs();
        Ok(s.info())
    })
}

/// Uses the file `path` for the xref `name` (`None` goes back to looking for it), reloads the
/// xrefs and returns the drawing's info. The window saves the project after, which remembers it.
#[tauri::command]
pub async fn set_xref_path(
    name: String,
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<DrawingInfo, String> {
    with_session(&state, |s| {
        s.set_xref_path(&name, path.as_deref().map(Path::new))?;
        Ok(s.info())
    })
}

/// Names the current sheet's line batch; it changes only when the batch would.
#[tauri::command]
pub async fn batch_version(state: State<'_, AppState>) -> Result<u64, String> {
    with_session(&state, |s| Ok(s.batch_version()))
}

/// The current sheet's batch (lines and text glyph quads), in the binary layout of
/// `Batch::to_bytes`. When `if_version` is the current `batch_version`, only the 8 bytes
/// `NOT_MODIFIED`: the window keeps its batch. `px_per_unit` is the zoom to draw the glyphs for
/// (screen pixels per drawing unit); left out, the batch in hand keeps its zoom.
#[tauri::command]
pub async fn render_batch(
    if_version: Option<u64>,
    px_per_unit: Option<f64>,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    with_session(&state, |s| {
        Ok(Response::new(batch_response(s, if_version, px_per_unit)))
    })
}

/// The glyph atlas pages changed after `since_version` (all when left out), in the binary layout
/// of `Atlas::to_bytes`: `u64` version, `u32` page count, then per page `u32` index, width,
/// height and byte length and the R8 bytes.
#[tauri::command]
pub async fn atlas_pages(
    since_version: Option<u64>,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    with_session(&state, |s| Ok(Response::new(s.atlas_bytes(since_version))))
}

/// "QINM" and four zero bytes, in place of a batch that is unchanged. A batch starts with "BYIQ".
const NOT_MODIFIED: [u8; 8] = [0x51, 0x49, 0x4E, 0x4D, 0, 0, 0, 0];

fn batch_response(session: &Session, if_version: Option<u64>, px_per_unit: Option<f64>) -> Vec<u8> {
    session
        .batch_bytes_if(if_version, px_per_unit)
        .unwrap_or_else(|| NOT_MODIFIED.to_vec())
}

#[tauri::command]
pub async fn pick(
    x: f64,
    y: f64,
    tol: f64,
    state: State<'_, AppState>,
) -> Result<Option<Picked>, String> {
    with_session(&state, |s| Ok(s.pick([x, y], tol)))
}

/// The object snap within `tol` (drawing units) of `(x, y)`, with the session's snap modes;
/// `from_x`, `from_y` is the previous point, for perpendicular snaps. None while snaps are off.
#[tauri::command]
pub async fn snap(
    x: f64,
    y: f64,
    tol: f64,
    from_x: Option<f64>,
    from_y: Option<f64>,
    state: State<'_, AppState>,
) -> Result<Option<Snap>, String> {
    let from = from_x.zip(from_y).map(|(fx, fy)| [fx, fy]);
    with_session(&state, |s| {
        Ok(s.drafting
            .osnap
            .then(|| s.snap([x, y], tol, &s.drafting.snap_modes, from))
            .flatten())
    })
}

/// The drawing's drafting settings: snaps, ortho, polar, grid and display precision.
#[tauri::command]
pub async fn get_drafting(state: State<'_, AppState>) -> Result<Drafting, String> {
    with_session(&state, |s| Ok(s.drafting.clone()))
}

/// Replaces the drafting settings (not an undo step); the window saves the project after.
#[tauri::command]
pub async fn set_drafting(drafting: Drafting, state: State<'_, AppState>) -> Result<(), String> {
    with_session(&state, |s| {
        s.run(Command::SetDrafting { drafting }).map(|_| ())
    })
}

/// One entity; `chain` (outermost block reference first) for an entity inside blocks.
#[tauri::command]
pub async fn entity_info(
    handle: u64,
    chain: Option<Vec<u64>>,
    state: State<'_, AppState>,
) -> Result<EntityInfo, String> {
    with_session(&state, |s| {
        s.entity_info(handle, chain.as_deref().unwrap_or_default())
    })
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

/// Writes a table of extracted data (`Command::ExtractData`) to an Excel workbook at `path`, one
/// sheet "Data".
#[tauri::command]
pub async fn export_table(
    path: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
) -> Result<(), String> {
    qiyas_boq::export_table(Path::new(&path), &columns, &rows).map_err(|e| e.to_string())
}

/// Reads the BOQ in the Excel workbook at `path` and makes it the drawing's BOQ (an undo step);
/// returns it.
#[tauri::command]
pub async fn import_boq(path: String, state: State<'_, AppState>) -> Result<Boq, String> {
    // Read the workbook before taking the lock, so a slow read doesn't block other commands.
    let boq = qiyas_boq::import_boq(Path::new(&path)).map_err(|e| e.to_string())?;
    with_session(&state, |s| {
        s.run(Command::SetBoq { boq: boq.clone() })?;
        Ok(boq)
    })
}

/// The drawing's BOQ; null when none is imported.
#[tauri::command]
pub async fn boq(state: State<'_, AppState>) -> Result<Option<Boq>, String> {
    with_session(&state, |s| Ok(s.boq.clone()))
}

/// Compares the BOQ and the take-off with the schedules and block counts of every sheet and keeps
/// the conflicts (an undo step); returns them. `tolerance_default` is the allowed difference in
/// percent for BOQ items with none of their own (5 when null).
#[tauri::command]
pub async fn compare(
    tolerance_default: Option<f64>,
    state: State<'_, AppState>,
) -> Result<Vec<Conflict>, String> {
    let tolerance = tolerance_default.unwrap_or(qiyas_takeoff::DEFAULT_TOLERANCE);
    with_session(&state, |s| {
        qiyas_takeoff::compare_session(s, tolerance, Source::User)
    })
}

/// The conflicts of the last comparison, with their statuses and notes.
#[tauri::command]
pub async fn conflicts(state: State<'_, AppState>) -> Result<Vec<Conflict>, String> {
    with_session(&state, |s| Ok(s.conflicts.clone()))
}

/// Writes the conflicts to an Excel workbook at `path`.
#[tauri::command]
pub async fn export_conflicts(path: String, state: State<'_, AppState>) -> Result<(), String> {
    // Copy the conflicts out, so writing the file doesn't hold the lock.
    let conflicts = with_session(&state, |s| Ok(s.conflicts.clone()))?;
    qiyas_boq::report::export_conflicts(Path::new(&path), &conflicts).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone())
}

/// Checks and keeps new settings: written to disk first, then cached. Another provider, address
/// or model starts a new assistant conversation; MCP turned on or off, or another port or token,
/// restarts the MCP server.
#[tauri::command]
pub async fn set_settings<R: Runtime>(
    settings: Settings,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    settings::validate(&settings)?;
    let (changed, mcp_changed) = {
        let mut cached = state
            .settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        settings::save(&state.config_dir, &settings)
            .map_err(|e| format!("The settings could not be saved: {e}"))?;
        let changed = ai::answerer_changed(&cached, &settings);
        let mcp_changed = mcp::needs_restart(&cached, &settings);
        *cached = settings;
        (changed, mcp_changed)
    };
    if changed {
        new_conversation(&app, &state);
    }
    if mcp_changed {
        mcp::restart(&app);
    }
    Ok(())
}

/// Runs a keychain call off the async workers: the OS may show a prompt and wait.
async fn keychain<R: Send + 'static>(
    f: impl FnOnce() -> Result<R, String> + Send + 'static,
) -> Result<R, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

/// Stores the provider's API key in the OS keychain. The key never comes back to the window. The
/// assistant's next message starts a new conversation with it.
#[tauri::command]
pub async fn set_api_key<R: Runtime>(
    provider: String,
    key: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    keychain(move || secrets::set_api_key(&provider, &key)).await?;
    new_conversation(&app, &state);
    Ok(())
}

#[tauri::command]
pub async fn has_api_key(provider: String) -> Result<bool, String> {
    keychain(move || Ok(secrets::get_api_key(&provider)?.is_some())).await
}

#[tauri::command]
pub async fn delete_api_key<R: Runtime>(
    provider: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    keychain(move || secrets::delete_api_key(&provider)).await?;
    new_conversation(&app, &state);
    Ok(())
}

/// The folder of the running app, where the `qiyas-mcp` bridge sits next to it. In a dev build
/// (`pnpm tauri dev`) that is `target/debug`, where `cargo build -p qiyas-mcp` puts the bridge.
#[tauri::command]
pub async fn app_dir() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("The app's folder is unknown")?;
    Ok(dir.display().to_string())
}

/// Whether the MCP server is on, listening, or why it could not start.
#[tauri::command]
pub async fn mcp_status(state: State<'_, AppState>) -> Result<McpStatus, String> {
    Ok(state
        .mcp
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .status
        .clone())
}

// ---- The in-app assistant ----

fn ai_run(state: &AppState) -> MutexGuard<'_, AiRun> {
    state.ai.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Forgets the assistant's conversation (cancelling a running turn) and tells the window, which
/// clears its transcript on `ai:cleared`.
fn new_conversation<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    ai_run(state).new_conversation();
    let _ = app.emit("ai:cleared", ());
}

/// Sent as `ai:done` when a turn has ended; `error` when it failed.
#[derive(Serialize, Clone)]
struct AiDone {
    error: Option<String>,
}

/// Sends `text` to the assistant and returns at once; the turn runs in the background and sends
/// each `AgentEvent` as `ai:event`, then `ai:done`. Fails with "busy" while a turn runs, and with
/// "no_api_key" or "no_base_url" when the settings can't reach a provider.
#[tauri::command]
pub async fn ai_send<R: Runtime>(
    text: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("The message is empty".into());
    }
    let (kept, generation) = ai_run(&state).reserve()?;
    // Gives the claim back on every way out before the turn starts, a panic included.
    let mut claim = Claim {
        state: &state,
        generation,
        armed: true,
    };
    let language = ai_run(&state).language();
    let settings = state
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let mut agent = match kept {
        Some(agent) => agent,
        None => new_agent(&state, &settings, language).await?,
    };
    // Effort may have changed since the last message.
    agent.set_settings(ai::agent_settings(&settings, language));
    let turn = ai_run(&state).start(agent, generation);
    // `start` settles the claim either way.
    claim.armed = false;
    tauri::async_runtime::spawn(supervise(app, turn?, text));
    Ok(())
}

/// A turn claimed by `ai_send` but not started yet.
struct Claim<'a> {
    state: &'a AppState,
    generation: u64,
    armed: bool,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        if self.armed {
            ai_run(self.state).release(None, self.generation);
        }
    }
}

/// A new agent for the settings' provider, with its key from the keychain (or the mock).
async fn new_agent(state: &AppState, settings: &Settings, language: Lang) -> Result<Agent, String> {
    #[cfg(test)]
    let test_provider = ai_run(state).test_provider.clone();
    #[cfg(test)]
    if let Some(make) = test_provider {
        return Ok(Agent::new(
            make(),
            state.session.clone(),
            ai::agent_settings(settings, language),
        ));
    }
    let provider = match ai::mock_provider(&state.session) {
        Some(mock) => mock,
        None => {
            let name = settings.provider.name();
            let key = match keychain(move || secrets::get_api_key(name)).await {
                Ok(key) => key,
                // A local OpenAI-compatible server needs no key, so no keychain is fine.
                Err(_) if settings.provider == settings::Provider::OpenaiCompatible => None,
                Err(e) => return Err(e),
            };
            ai::build_provider(settings, key)?
        }
    };
    Ok(Agent::new(
        provider,
        state.session.clone(),
        ai::agent_settings(settings, language),
    ))
}

/// Runs one turn, forwarding its events to the window while its conversation is current.
/// Runs the turn on its own task and always ends it with `ai:done`: if the task panics, the
/// conversation is dropped and the error is `CRASHED`.
async fn supervise<R: Runtime>(app: AppHandle<R>, turn: Turn, text: String) {
    let generation = turn.generation;
    let task = tauri::async_runtime::spawn(run_turn(app.clone(), turn, text));
    let error = match task.await {
        Ok((agent, result)) => {
            ai_run(&app.state::<AppState>()).finish(agent, generation);
            result.err().map(|e| e.to_string())
        }
        Err(_) => {
            ai_run(&app.state::<AppState>()).crashed();
            Some(ai::CRASHED.to_string())
        }
    };
    let _ = app.emit("ai:done", AiDone { error });
}

/// Runs one turn, forwarding its events to the window while its conversation is current, and
/// gives the agent back.
async fn run_turn<R: Runtime>(
    app: AppHandle<R>,
    turn: Turn,
    text: String,
) -> (Agent, Result<(), qiyas_ai::AiError>) {
    let Turn {
        mut agent,
        cancel,
        generation,
    } = turn;
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(64);
    let forward = async {
        while let Some(event) = rx.recv().await {
            let state = app.state::<AppState>();
            if ai_run(&state).generation() == generation {
                let _ = app.emit("ai:event", &event);
            }
        }
    };
    // The agent drops the sender when the turn ends, which ends `forward`.
    let (result, ()) = tokio::join!(agent.run_turn(text, tx, cancel), forward);
    (agent, result)
}

/// Undoes the assistant's last change, only if it is still the newest change to the take-off.
/// False when the user (or nothing) changed it since.
#[tauri::command]
pub async fn ai_undo(state: State<'_, AppState>) -> Result<bool, String> {
    with_session(&state, |s| Ok(s.undo_if(qiyas_core::Source::Ai)))
}

/// Stops the running turn; it ends with a "cancelled" stop.
#[tauri::command]
pub async fn ai_cancel(state: State<'_, AppState>) -> Result<(), String> {
    ai_run(&state).cancel();
    Ok(())
}

/// Starts a new conversation.
#[tauri::command]
pub async fn ai_clear<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    new_conversation(&app, &state);
    Ok(())
}

/// The window's language ("en" or "ar") for the assistant's prompt; a change starts a new
/// conversation.
#[tauri::command]
pub async fn ai_set_language<R: Runtime>(
    lang: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if ai_run(&state).set_language(Lang::from_code(&lang)) {
        let _ = app.emit("ai:cleared", ());
    }
    Ok(())
}

#[derive(Serialize)]
pub struct AiStatus {
    /// "anthropic", "openai_compatible", or "mock".
    provider: String,
    model: String,
    /// A key for the provider is in the keychain.
    has_key: bool,
    busy: bool,
    message_count: usize,
}

#[tauri::command]
pub async fn ai_status(state: State<'_, AppState>) -> Result<AiStatus, String> {
    let settings = state
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let mock = ai::mock_provider(&state.session).is_some();
    let has_key = if mock {
        true
    } else {
        let name = settings.provider.name();
        keychain(move || secrets::get_api_key(name))
            .await
            .is_ok_and(|key| key.is_some())
    };
    let run = ai_run(&state);
    Ok(AiStatus {
        provider: if mock {
            "mock"
        } else {
            settings.provider.name()
        }
        .to_string(),
        model: if mock { "mock".into() } else { settings.model },
        has_key,
        busy: run.busy(),
        message_count: run.message_count(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{channel, Receiver};
    use std::sync::Arc;
    use std::time::Duration;

    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
    use tauri::webview::InvokeRequest;
    use tauri::{App, Listener, WebviewWindow, WebviewWindowBuilder};

    use super::*;

    struct Ipc {
        _dir: tempfile::TempDir,
        _app: App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
        /// (event name, JSON payload) as the window would get them.
        events: Receiver<(String, Value)>,
    }

    impl Ipc {
        /// The app on the mock runtime with the canned AI provider and the sample drawing open.
        fn open() -> Ipc {
            std::env::set_var("QIYAS_AI_MOCK", "1");
            let dir = tempfile::tempdir().unwrap();
            let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
            let app = mock_builder()
                .invoke_handler(tauri::generate_handler![
                    open_drawing,
                    takeoff,
                    run_command,
                    undo,
                    boq,
                    compare,
                    conflicts,
                    export_conflicts,
                    ai_undo,
                    ai_send,
                    ai_cancel,
                    ai_clear,
                    ai_status,
                    get_settings,
                    set_settings,
                    mcp_status,
                    snap,
                    get_drafting,
                    set_drafting,
                    render_batch,
                    crate::pdf::pdf_page_count,
                    crate::pdf::pdf_thumbnails,
                    crate::pdf::attach_pdf,
                    crate::pdf::is_dirty,
                    crate::pdf::calibrate,
                    crate::pdf::underlay_image,
                    crate::pdf::image_size,
                    crate::pdf::attach_image,
                    crate::pdf::image_display
                ])
                .build(mock_context(noop_assets()))
                .unwrap();
            app.manage(AppState::new(dir.path().join("config")));
            let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            let (tx, events) = channel();
            for name in ["ai:event", "ai:done", "ai:cleared", crate::pdf::PROGRESS] {
                let tx = tx.clone();
                app.listen_any(name, move |e| {
                    let payload = serde_json::from_str(e.payload()).unwrap_or(Value::Null);
                    let _ = tx.send((name.to_string(), payload));
                });
            }
            let ipc = Ipc {
                _dir: dir,
                _app: app,
                webview,
                events,
            };
            ipc.call("open_drawing", json!({ "path": path })).unwrap();
            assert_eq!(
                ipc.next().0,
                "ai:cleared",
                "a new drawing, a new conversation"
            );
            ipc
        }

        fn call(&self, cmd: &str, body: Value) -> Result<Value, Value> {
            let request = InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                // The app's own origin, so the commands are allowed.
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            };
            get_ipc_response(&self.webview, request).map(|b| b.deserialize().unwrap())
        }

        /// A command that answers raw bytes.
        fn bytes(&self, cmd: &str, body: Value) -> Vec<u8> {
            let request = InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            };
            match get_ipc_response(&self.webview, request).unwrap() {
                InvokeResponseBody::Raw(bytes) => bytes,
                InvokeResponseBody::Json(json) => panic!("not bytes: {json}"),
            }
        }

        fn next(&self) -> (String, Value) {
            self.events.recv_timeout(Duration::from_secs(10)).unwrap()
        }

        /// The `ai:event` payloads up to `ai:done`, and its payload.
        fn turn(&self) -> (Vec<Value>, Value) {
            let mut events = Vec::new();
            loop {
                match self.next() {
                    (name, done) if name == "ai:done" => return (events, done),
                    (_, event) => events.push(event),
                }
            }
        }
    }

    #[test]
    fn a_turn_streams_its_events_and_measures_on_the_session() {
        let ipc = Ipc::open();
        ipc.call("ai_send", json!({ "text": "Measure the wall" }))
            .unwrap();
        assert_eq!(
            ipc.call("ai_send", json!({ "text": "And again" })),
            Err(json!("busy")),
            "one turn at a time"
        );
        let (events, done) = ipc.turn();
        assert_eq!(done, json!({ "error": null }));

        let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        let first_call = kinds.iter().position(|k| *k == "tool_call").unwrap();
        assert!(kinds[..first_call].iter().all(|k| *k == "text"));
        assert_eq!(kinds[first_call + 1], "tool_result");
        assert_eq!(kinds.last(), Some(&"done"));
        let call = &events[first_call];
        assert_eq!(call["name"], "measure_area");
        assert_eq!(call["mutating"], true);
        let result = &events[first_call + 1];
        assert_eq!(result["is_error"], false, "{result}");
        assert_eq!(result["handles"].as_array().unwrap().len(), 1);

        let items = ipc.call("takeoff", json!({})).unwrap();
        assert_eq!(items[0]["source"], "ai");

        let status = ipc.call("ai_status", json!({})).unwrap();
        assert_eq!(status["provider"], "mock");
        assert_eq!(status["busy"], false);
        assert_eq!(status["message_count"], 4);

        ipc.call("ai_clear", json!({})).unwrap();
        assert_eq!(ipc.next().0, "ai:cleared");
        let status = ipc.call("ai_status", json!({})).unwrap();
        assert_eq!(status["message_count"], 0);
    }

    /// Panics in its first turn (counted across the providers it makes), then answers "ok".
    struct PanicsFirst(Arc<AtomicUsize>);
    impl qiyas_ai::Provider for PanicsFirst {
        fn stream_turn<'a>(
            &'a self,
            _: qiyas_ai::TurnRequest,
            _: qiyas_ai::provider::Sink<'a>,
        ) -> qiyas_ai::provider::BoxFuture<'a, Result<qiyas_ai::TurnResult, qiyas_ai::AiError>>
        {
            let first = self.0.fetch_add(1, Ordering::SeqCst) == 0;
            Box::pin(async move {
                assert!(!first, "the provider broke");
                Ok(qiyas_ai::TurnResult {
                    content: vec![qiyas_ai::ContentBlock::Text { text: "ok".into() }],
                    stop_reason: qiyas_ai::StopReason::EndTurn,
                    usage: Default::default(),
                })
            })
        }
    }

    #[test]
    fn a_panicking_turn_ends_with_an_error_and_frees_the_assistant() {
        let ipc = Ipc::open();
        let calls = Arc::new(AtomicUsize::new(0));
        {
            let state = ipc.webview.state::<AppState>();
            let calls = calls.clone();
            ai_run(&state).test_provider =
                Some(Arc::new(move || Box::new(PanicsFirst(calls.clone()))));
        }
        ipc.call("ai_send", json!({ "text": "Measure" })).unwrap();
        let (_, done) = ipc.turn();
        assert_eq!(done, json!({ "error": "crashed" }));
        let status = ipc.call("ai_status", json!({})).unwrap();
        assert_eq!(status["busy"], false);
        assert_eq!(status["message_count"], 0, "the conversation was dropped");

        ipc.call("ai_send", json!({ "text": "Again" })).unwrap();
        let (events, done) = ipc.turn();
        assert_eq!(done, json!({ "error": null }));
        assert_eq!(events.last().unwrap()["type"], "done");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn ai_undo_takes_back_only_the_assistants_newest_change() {
        let ipc = Ipc::open();
        ipc.call("ai_send", json!({ "text": "Measure the wall" }))
            .unwrap();
        ipc.turn();
        // The user measures after the assistant.
        let cmd = json!({ "type": "measure_distance", "points": [[0.0, 0.0], [1.0, 0.0]] });
        ipc.call("run_command", json!({ "cmd": cmd })).unwrap();
        assert_eq!(ipc.call("ai_undo", json!({})), Ok(json!(false)));
        assert_eq!(
            ipc.call("takeoff", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(ipc.call("undo", json!({})), Ok(json!(true)));
        assert_eq!(ipc.call("ai_undo", json!({})), Ok(json!(true)));
        assert_eq!(ipc.call("takeoff", json!({})), Ok(json!([])));
    }

    #[test]
    fn snap_follows_the_drafting_settings() {
        let ipc = Ipc::open();
        let at = json!({ "x": 4.03, "y": 2.97, "tol": 0.05 });
        let found = ipc.call("snap", at.clone()).unwrap();
        assert_eq!(found["kind"], "end");
        assert_eq!(found["point"], json!([4.0, 3.0]));

        let mut drafting = ipc.call("get_drafting", json!({})).unwrap();
        assert_eq!(drafting["osnap"], true);
        drafting["osnap"] = json!(false);
        ipc.call("set_drafting", json!({ "drafting": drafting }))
            .unwrap();
        assert_eq!(ipc.call("snap", at).unwrap(), Value::Null);
        assert_eq!(ipc.call("get_drafting", json!({})).unwrap()["osnap"], false);

        drafting["grid_spacing"] = json!(-1.0);
        assert!(ipc
            .call("set_drafting", json!({ "drafting": drafting }))
            .is_err());
    }

    #[test]
    fn boq_reads_the_sessions_bill_and_follows_undo() {
        let ipc = Ipc::open();
        assert_eq!(ipc.call("boq", json!({})), Ok(Value::Null));
        let item = json!({
            "code": "A.1", "description": "Walls", "unit": "m", "quantity": 12.0,
            "rate": null, "section": "Masonry", "tolerance": null, "row": 3
        });
        let bill = json!({
            "source": "boq.xlsx", "imported": "2026-10-08T00:00:00Z",
            "items": [item], "skipped_rows": 1
        });
        let cmd = json!({ "type": "set_boq", "boq": bill });
        ipc.call("run_command", json!({ "cmd": cmd })).unwrap();
        assert_eq!(ipc.call("boq", json!({})), Ok(bill));
        assert_eq!(ipc.call("undo", json!({})), Ok(json!(true)));
        assert_eq!(ipc.call("boq", json!({})), Ok(Value::Null));
    }

    #[test]
    fn compare_keeps_the_conflicts_and_exports_them() {
        let ipc = Ipc::open();
        let item = json!({
            "code": "A.1", "description": "Roof waterproofing", "unit": "m2", "quantity": 12.0,
            "rate": null, "section": "", "tolerance": null, "row": 3
        });
        let bill = json!({
            "source": "boq.xlsx", "imported": "2026-10-08T00:00:00Z",
            "items": [item], "skipped_rows": 0
        });
        let cmd = json!({ "type": "set_boq", "boq": bill });
        ipc.call("run_command", json!({ "cmd": cmd })).unwrap();
        let found = ipc
            .call("compare", json!({ "toleranceDefault": null }))
            .unwrap();
        assert_eq!(found[0]["kind"], "not_in_drawings", "{found}");
        assert_eq!(ipc.call("conflicts", json!({})), Ok(found.clone()));

        let id = found[0]["id"].clone();
        let cmd =
            json!({ "type": "resolve_conflict", "id": id, "status": "ignored", "note": "Later" });
        ipc.call("run_command", json!({ "cmd": cmd })).unwrap();
        assert_eq!(
            ipc.call("conflicts", json!({})).unwrap()[0]["status"],
            "ignored"
        );

        let path = ipc._dir.path().join("conflicts.xlsx");
        ipc.call("export_conflicts", json!({ "path": path }))
            .unwrap();
        assert!(path.exists());

        assert_eq!(ipc.call("undo", json!({})), Ok(json!(true)));
        assert_eq!(ipc.call("undo", json!({})), Ok(json!(true)));
        assert_eq!(ipc.call("conflicts", json!({})), Ok(json!([])));
    }

    #[test]
    fn a_cancelled_turn_says_so_and_ends() {
        let ipc = Ipc::open();
        ipc.call("ai_send", json!({ "text": "Measure the wall" }))
            .unwrap();
        ipc.call("ai_cancel", json!({})).unwrap();
        let (events, done) = ipc.turn();
        assert_eq!(done, json!({ "error": null }));
        assert_eq!(
            events.last(),
            Some(&json!({ "type": "stopped", "reason": "cancelled" }))
        );
        let status = ipc.call("ai_status", json!({})).unwrap();
        assert_eq!(status["busy"], false);
    }

    /// Calls `mcp_status` until `done` holds for it, for up to 5 s.
    fn mcp_status_until(ipc: &Ipc, done: impl Fn(&Value) -> bool) -> Value {
        for _ in 0..100 {
            let status = ipc.call("mcp_status", json!({})).unwrap();
            if done(&status) {
                return status;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("mcp_status never got there");
    }

    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    #[test]
    fn the_mcp_server_follows_the_settings() {
        let ipc = Ipc::open();
        let status = ipc.call("mcp_status", json!({})).unwrap();
        assert_eq!(
            status,
            json!({ "enabled": false, "listening": false, "addr": null, "error": null })
        );

        // On: it listens on the port.
        let port = free_port();
        let mut settings = ipc.call("get_settings", json!({})).unwrap();
        settings["mcp_enabled"] = json!(true);
        settings["mcp_port"] = json!(port);
        ipc.call("set_settings", json!({ "settings": settings }))
            .unwrap();
        let status = mcp_status_until(&ipc, |s| s["listening"] == true);
        assert_eq!(status["addr"], format!("127.0.0.1:{port}"));
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());

        // A port another program holds: the status says why.
        let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        settings["mcp_port"] = json!(taken.local_addr().unwrap().port());
        ipc.call("set_settings", json!({ "settings": settings }))
            .unwrap();
        let status = mcp_status_until(&ipc, |s| !s["error"].is_null());
        assert_eq!(status["listening"], false);
        assert!(status["error"].as_str().unwrap().contains("127.0.0.1"));

        // Off: the first port is free again.
        settings["mcp_enabled"] = json!(false);
        ipc.call("set_settings", json!({ "settings": settings }))
            .unwrap();
        let status = ipc.call("mcp_status", json!({})).unwrap();
        assert_eq!(
            (&status["enabled"], &status["listening"]),
            (&json!(false), &json!(false))
        );
        let mut freed = false;
        for _ in 0..100 {
            if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                freed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(freed, "the server kept port {port}");
    }

    #[test]
    fn a_pdf_page_attaches_as_an_underlay_and_its_picture_is_drawn() {
        if !qiyas_pdf::available() {
            eprintln!("skipped: needs pdfium (scripts/pdfium.sh)");
            return;
        }
        let ipc = Ipc::open();
        let pdf = qiyas_pdf::fixtures::write_wall(ipc._dir.path());
        assert_eq!(
            ipc.call("pdf_page_count", json!({ "path": pdf })),
            Ok(json!(1))
        );
        let thumbs = ipc.bytes(
            "pdf_thumbnails",
            json!({ "path": pdf, "first": 1, "count": 20, "px": 64 }),
        );
        let u32_at = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        assert_eq!(
            [
                u32_at(&thumbs, 0),
                u32_at(&thumbs, 4),
                u32_at(&thumbs, 8),
                u32_at(&thumbs, 12)
            ],
            [1, 1, 64, 32]
        );
        assert_eq!(thumbs.len(), 16 + 64 * 32 * 4);

        let attach = json!({
            "path": pdf, "page": 1, "x": 0.0, "y": 0.0, "scale": 0.01, "mode": "underlay"
        });
        assert_eq!(ipc.call("is_dirty", json!({})), Ok(json!(false)));
        let handle = ipc.call("attach_pdf", attach).unwrap();
        assert!(handle.as_u64().unwrap() > 0);
        assert_eq!(ipc.call("is_dirty", json!({})), Ok(json!(true)));
        let batch = qiyas_render_batch(&ipc.bytes("render_batch", json!({})));
        assert_eq!(batch, vec![(0, true)]);

        // 2 units wide at 300 px per unit: a 1024 px picture (zoom rounded up to 512).
        let picture = ipc.bytes("underlay_image", json!({ "id": 0, "pxPerUnit": 300.0 }));
        let (w, h) = (u32_at(&picture, 0), u32_at(&picture, 4));
        assert_eq!((w, h), (1024, 512));
        assert_eq!(picture.len(), 8 + (w * h * 4) as usize);
        let (name, last) = std::iter::from_fn(|| ipc.events.try_recv().ok())
            .filter(|(n, _)| n == crate::pdf::PROGRESS)
            .last()
            .expect("progress was sent");
        assert_eq!((name.as_str(), last), (crate::pdf::PROGRESS, json!(100)));
        // The same zoom again comes from the cache.
        assert_eq!(
            ipc.bytes("underlay_image", json!({ "id": 0, "pxPerUnit": 290.0 })),
            picture
        );

        let calibrate = json!({ "x1": 0.5, "y1": 0.5, "x2": 1.5, "y2": 0.5, "distance": 3.0 });
        ipc.call("calibrate", calibrate).unwrap();
        let outside = json!({ "x1": -5.0, "y1": -5.0, "x2": 1.0, "y2": 0.0, "distance": 3.0 });
        assert_eq!(
            ipc.call("calibrate", outside),
            Err(json!("Pick the first point on a PDF underlay or an image"))
        );
        assert_eq!(ipc.call("undo", json!({})), Ok(json!(true)));
        assert_eq!(ipc.call("undo", json!({})), Ok(json!(true)));
        let batch = qiyas_render_batch(&ipc.bytes("render_batch", json!({})));
        assert!(batch.is_empty());
    }

    #[test]
    fn an_image_attaches_draws_calibrates_and_takes_its_display_settings() {
        let ipc = Ipc::open();
        let png = qiyas_core::fixtures::write_png(ipc._dir.path(), "pic.png");
        assert_eq!(
            ipc.call("image_size", json!({ "path": png })),
            Ok(json!([200, 100]))
        );
        let attach = json!({ "path": png, "x": 5.0, "y": 5.0, "width": 20.0 });
        let handle = ipc.call("attach_image", attach).unwrap().as_u64().unwrap();
        assert_eq!(ipc.call("is_dirty", json!({})), Ok(json!(true)));
        let batch = qiyas_render_batch(&ipc.bytes("render_batch", json!({})));
        assert_eq!(batch, vec![(0, true)]);

        // 20 units at 5 px per unit, rounded up to 8: 160 x 80.
        let u32_at = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        let picture = ipc.bytes("underlay_image", json!({ "id": 0, "pxPerUnit": 5.0 }));
        assert_eq!((u32_at(&picture, 0), u32_at(&picture, 4)), (160, 80));
        assert_eq!(picture.len(), 8 + 160 * 80 * 4);
        assert_eq!(&picture[8..12], &[255, 0, 0, 255]);

        assert_eq!(
            ipc.call("image_display", json!({ "handle": handle })),
            Ok(json!({ "brightness": 50, "contrast": 50, "fade": 0 }))
        );
        let cmd = json!({
            "type": "set_image_display", "handle": handle, "brightness": 60, "contrast": 45, "fade": 20
        });
        assert_eq!(
            ipc.call("run_command", json!({ "cmd": cmd })),
            Ok(json!({ "type": "changed" }))
        );
        assert_eq!(
            ipc.call("image_display", json!({ "handle": handle })),
            Ok(json!({ "brightness": 60, "contrast": 45, "fade": 20 }))
        );
        assert_eq!(
            ipc.call("image_display", json!({ "handle": 1 })),
            Ok(json!(null))
        );

        // Calibrated by a point on it: 20 units become 10.
        let calibrate = json!({ "x1": 5.0, "y1": 5.0, "x2": 25.0, "y2": 5.0, "distance": 10.0 });
        ipc.call("calibrate", calibrate).unwrap();
        let picture = ipc.bytes("underlay_image", json!({ "id": 0, "pxPerUnit": 5.0 }));
        assert_eq!((u32_at(&picture, 0), u32_at(&picture, 4)), (80, 40));
        for _ in 0..3 {
            assert_eq!(ipc.call("undo", json!({})), Ok(json!(true)));
        }
        assert!(qiyas_render_batch(&ipc.bytes("render_batch", json!({}))).is_empty());
    }

    /// The image table of a batch (`Batch::to_bytes`, kind 11): (id, version below 2^53).
    fn qiyas_render_batch(bytes: &[u8]) -> Vec<(u32, bool)> {
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let k = u32_at(56) as usize;
        (0..k)
            .map(|i| {
                (
                    u32_at(60 + 12 * i),
                    u32_at(64 + 12 * i) as usize,
                    u32_at(68 + 12 * i) as usize,
                )
            })
            .filter(|(kind, ..)| *kind == 11)
            .flat_map(|(_, at, len)| {
                (at..at + len).step_by(16).map(move |e| {
                    let version = u64::from_le_bytes(bytes[e + 8..e + 16].try_into().unwrap());
                    (
                        u32::from_le_bytes(bytes[e..e + 4].try_into().unwrap()),
                        version < 1 << 53,
                    )
                })
            })
            .collect()
    }
}

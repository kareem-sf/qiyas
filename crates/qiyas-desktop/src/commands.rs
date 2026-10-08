//! IPC commands for the frontend. Each one locks the session, does its work and maps errors to text.
//! All are `async` so Tauri runs them off the main thread and a slow open or batch never freezes
//! the window. None awaits, so no lock is held across an `.await`.

use std::path::Path;
use std::sync::{MutexGuard, PoisonError};

use qiyas_ai::prompt::Lang;
use qiyas_ai::{Agent, AgentEvent};
use qiyas_core::{
    Boq, Command, CoreError, DrawingInfo, EntityInfo, Measurement, Outcome, Session, Total,
};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::mpsc;

use crate::ai::{self, AiRun, Turn};
use crate::secrets;
use crate::settings::{self, Settings};
use crate::state::AppState;

/// Runs `f` on the open session, or fails with "No drawing is open".
fn with_session<R>(
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

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone())
}

/// Checks and keeps new settings: written to disk first, then cached. Another provider, address
/// or model starts a new assistant conversation.
#[tauri::command]
pub async fn set_settings<R: Runtime>(
    settings: Settings,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    settings::validate(&settings)?;
    let changed = {
        let mut cached = state
            .settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        settings::save(&state.config_dir, &settings)
            .map_err(|e| format!("The settings could not be saved: {e}"))?;
        let changed = ai::answerer_changed(&cached, &settings);
        *cached = settings;
        changed
    };
    if changed {
        new_conversation(&app, &state);
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

/// The folder of the running app, where the `qiyas-mcp` bridge sits next to it.
#[tauri::command]
pub async fn app_dir() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("The app's folder is unknown")?;
    Ok(dir.display().to_string())
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
    use tauri::ipc::{CallbackFn, InvokeBody};
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
                    ai_undo,
                    ai_send,
                    ai_cancel,
                    ai_clear,
                    ai_status
                ])
                .build(mock_context(noop_assets()))
                .unwrap();
            app.manage(AppState::new(dir.path().join("config")));
            let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            let (tx, events) = channel();
            for name in ["ai:event", "ai:done", "ai:cleared"] {
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
}

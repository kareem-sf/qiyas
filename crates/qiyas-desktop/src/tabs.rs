//! File tabs: several drawings open at once. The active one's session sits in
//! `AppState::session`, where every IPC command, the assistant and the MCP server find it; the
//! others are parked in `AppState::parked` (without their batch and pick index, rebuilt on the
//! next draw or pick) and swapped in when their tab is chosen.

use std::path::Path;
use std::sync::{MutexGuard, PoisonError};

use qiyas_core::{DrawingInfo, Session};
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::commands::new_conversation;
use crate::pdf::pictures;
use crate::settings::same_file;
use crate::state::{AppState, ParkedTab};

/// Refusal of `close_tab` for a drawing with unsaved changes, without `force`.
pub const UNSAVED: &str = "unsaved";

/// One file tab.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct TabInfo {
    pub id: u32,
    pub path: String,
    /// The file name.
    pub title: String,
    /// The drawing itself has changes not saved yet.
    pub dirty: bool,
}

/// Every tab in order, and the one on screen (null when none is open).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Tabs {
    pub tabs: Vec<TabInfo>,
    pub active: Option<u32>,
}

fn lock<T>(m: &std::sync::Mutex<T>) -> MutexGuard<'_, T> {
    // A panic in an earlier command leaves the data as it was; keep using it.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn tab_info(id: u32, s: &Session) -> TabInfo {
    let path = &s.drawing.path;
    TabInfo {
        id,
        path: path.display().to_string(),
        title: path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        ),
        dirty: s.is_dirty(),
    }
}

/// The tabs as they are now; path and dirty are read off the sessions, so they never go stale.
pub(crate) fn snapshot(state: &AppState) -> Tabs {
    let list = lock(&state.tabs);
    let session = lock(&state.session);
    let parked = lock(&state.parked);
    let tabs = list
        .order
        .iter()
        .filter_map(|&id| {
            if list.active == Some(id) {
                session.as_ref().map(|s| tab_info(id, s))
            } else {
                parked
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| tab_info(id, &p.session))
            }
        })
        .collect();
    Tabs {
        tabs,
        active: list.active,
    }
}

/// The tab whose drawing is the file `path`, if one is open.
pub(crate) fn find_path(state: &AppState, path: &Path) -> Option<u32> {
    let list = lock(&state.tabs);
    let session = lock(&state.session);
    let parked = lock(&state.parked);
    if let (Some(id), Some(s)) = (list.active, session.as_ref()) {
        if same_file(&s.drawing.path, path) {
            return Some(id);
        }
    }
    parked
        .iter()
        .find(|p| same_file(&p.session.drawing.path, path))
        .map(|p| p.id)
}

/// Puts `session` on screen in a new tab after the others; the drawing on screen before is
/// parked. Returns the new tab's id.
pub(crate) fn add(state: &AppState, session: Session) -> u32 {
    let mut list = lock(&state.tabs);
    let mut active = lock(&state.session);
    let mut parked = lock(&state.parked);
    let id = list.next_id;
    list.next_id += 1;
    if let (Some(old_id), Some(mut old)) = (list.active, active.take()) {
        old.park();
        parked.push(ParkedTab {
            id: old_id,
            session: old,
        });
    }
    pictures(state).show(session.epoch());
    *active = Some(session);
    list.order.push(id);
    list.active = Some(id);
    id
}

/// Swaps the parked session of tab `id` onto the screen, parking the one there. The assistant
/// starts a new conversation (it was about the other drawing). Nothing happens when `id` is on
/// screen already.
pub(crate) fn activate<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    id: u32,
) -> Result<(), String> {
    let mut list = lock(&state.tabs);
    if list.active == Some(id) {
        return Ok(());
    }
    let mut session = lock(&state.session);
    let mut parked = lock(&state.parked);
    let at = parked
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| format!("No tab {id}"))?;
    // Stop a running turn first, so it can't run another tool on the other drawing.
    new_conversation(app, state);
    let incoming = parked.remove(at).session;
    let outgoing = session.replace(incoming);
    if let (Some(old_id), Some(mut old)) = (list.active, outgoing) {
        old.park();
        parked.push(ParkedTab {
            id: old_id,
            session: old,
        });
    }
    list.active = Some(id);
    pictures(state).show(session.as_ref().map_or(0, Session::epoch));
    Ok(())
}

/// Deletes the autosave of a drawing closed without saving (best effort: logged).
fn forget_autosave(s: &Session) {
    if let Err(e) = qiyas_core::discard_autosave(&s.drawing.path) {
        eprintln!(
            "The autosave of {} could not be deleted: {e}",
            s.drawing.path.display()
        );
    }
}

/// Closes tab `id`. One with unsaved drawing changes is refused with `UNSAVED` unless `force`
/// (the window asks first; closed without saving, its autosave is deleted). Closing the tab on screen shows the next one (else the one before);
/// the assistant then starts a new conversation.
fn close<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    id: u32,
    force: bool,
) -> Result<(), String> {
    let mut list = lock(&state.tabs);
    let Some(pos) = list.order.iter().position(|&t| t == id) else {
        return Err(format!("No tab {id}"));
    };
    let mut session = lock(&state.session);
    let mut parked = lock(&state.parked);
    if list.active == Some(id) {
        if !force && session.as_ref().is_some_and(Session::is_dirty) {
            return Err(UNSAVED.into());
        }
        // Closed without saving: its autosave goes too.
        if let Some(s) = session.as_ref().filter(|s| s.is_dirty()) {
            forget_autosave(s);
        }
        new_conversation(app, state);
        list.order.remove(pos);
        let next = list
            .order
            .get(pos)
            .or_else(|| pos.checked_sub(1).and_then(|p| list.order.get(p)))
            .copied();
        *session = None;
        list.active = None;
        if let Some(next) = next {
            if let Some(at) = parked.iter().position(|p| p.id == next) {
                *session = Some(parked.remove(at).session);
                list.active = Some(next);
            }
        }
        pictures(state).show(session.as_ref().map_or(0, Session::epoch));
    } else {
        let at = parked.iter().position(|p| p.id == id);
        if !force && at.is_some_and(|at| parked[at].session.is_dirty()) {
            return Err(UNSAVED.into());
        }
        if let Some(at) = at {
            let gone = parked.remove(at);
            if gone.session.is_dirty() {
                forget_autosave(&gone.session);
            }
        }
        list.order.remove(pos);
    }
    Ok(())
}

/// Every open drawing's tab, and the one on screen.
#[tauri::command]
pub async fn tabs(state: State<'_, AppState>) -> Result<Tabs, String> {
    Ok(snapshot(&state))
}

/// Shows the drawing of tab `id`; returns its info.
#[tauri::command]
pub async fn activate_tab<R: Runtime>(
    id: u32,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<DrawingInfo, String> {
    activate(&app, &state, id)?;
    crate::commands::with_session(&state, |s| Ok(s.info()))
}

/// Closes tab `id` (see `close`); returns the tabs left.
#[tauri::command]
pub async fn close_tab<R: Runtime>(
    id: u32,
    force: Option<bool>,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Tabs, String> {
    close(&app, &state, id, force.unwrap_or(false))?;
    Ok(snapshot(&state))
}

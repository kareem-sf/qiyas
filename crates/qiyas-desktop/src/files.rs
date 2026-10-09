//! Files beyond open and save: the system clipboard (`arboard`), autosave and recovery of
//! drawings, Recover (a damaged file opened with a report of what was dropped), and sheet sets.

use std::path::Path;
use std::sync::{MutexGuard, PoisonError};

use qiyas_core::{
    recoverable, sheet_set_entries, sheet_set_entry as entry_for, Drawing, DrawingInfo, ReadReport,
    Recoverable, Session, SheetSet, SheetSetDrawing, SheetSetEntry,
};
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::commands::{new_conversation, remember_recent, with_session};
use crate::state::AppState;
use crate::tabs;

fn lock<T>(m: &std::sync::Mutex<T>) -> MutexGuard<'_, T> {
    // A panic in an earlier command leaves the data as it was; keep using it.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `f` on the app's system clipboard, opening it on first use.
fn with_clipboard<R>(
    state: &AppState,
    f: impl FnOnce(&mut arboard::Clipboard) -> Result<R, arboard::Error>,
) -> Result<R, arboard::Error> {
    let mut held = lock(&state.clipboard);
    if held.is_none() {
        *held = Some(arboard::Clipboard::new()?);
    }
    f(held.as_mut().expect("just opened"))
}

/// The text on the system clipboard.
pub(crate) fn clipboard_text(state: &AppState) -> Result<String, String> {
    with_clipboard(state, |c| c.get_text())
        .map_err(|e| format!("The clipboard could not be read: {e}"))
}

/// Puts `text` on the system clipboard; the app keeps it there while it runs.
pub(crate) fn set_clipboard_text(state: &AppState, text: &str) -> Result<(), String> {
    with_clipboard(state, |c| c.set_text(text.to_string()))
        .map_err(|e| format!("The clipboard could not be written: {e}"))
}

/// What one autosave round did: the autosaves written, and why others could not be.
#[derive(Serialize, Debug, Default)]
pub struct AutosaveRun {
    pub written: Vec<String>,
    pub failed: Vec<String>,
}

/// Autosaves every open drawing with unsaved changes, the one on screen and those parked in
/// other tabs (see `Session::autosave`).
#[tauri::command]
pub async fn autosave_drawings(state: State<'_, AppState>) -> Result<AutosaveRun, String> {
    let mut run = AutosaveRun::default();
    let mut one = |s: &Session| match s.autosave() {
        Ok(Some(path)) => run.written.push(path.display().to_string()),
        Ok(None) => {}
        Err(e) => run
            .failed
            .push(format!("{}: {e}", s.drawing.path.display())),
    };
    // The lock order of `tabs`: tabs, session, parked.
    let _list = lock(&state.tabs);
    if let Some(s) = lock(&state.session).as_ref() {
        one(s);
    }
    for p in lock(&state.parked).iter() {
        one(&p.session);
    }
    Ok(run)
}

/// The autosaves of `paths` newer than their drawings (the recent drawings at launch, or the
/// drawing about to open).
#[tauri::command]
pub async fn recoverable_drawings(paths: Vec<String>) -> Result<Vec<Recoverable>, String> {
    Ok(paths
        .iter()
        .filter_map(|p| recoverable(Path::new(p)))
        .collect())
}

/// Opens the autosave of the drawing `path` in a new tab as a new drawing with unsaved changes
/// beside it (`Session::open_autosave`); the window then offers Save As.
#[tauri::command]
pub async fn recover_autosave<R: Runtime>(
    path: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<DrawingInfo, String> {
    let session = Session::open_autosave(Path::new(&path)).map_err(|e| e.to_string())?;
    let info = session.info();
    new_conversation(&app, &state);
    tabs::add(&state, session);
    Ok(info)
}

/// Deletes the autosave of the drawing `path`.
#[tauri::command]
pub async fn discard_autosave(path: String) -> Result<(), String> {
    qiyas_core::discard_autosave(Path::new(&path)).map_err(|e| e.to_string())
}

/// A drawing Recover opened, and what its reader dropped.
#[derive(Serialize)]
pub struct Recovered {
    pub info: DrawingInfo,
    pub report: ReadReport,
}

/// RECOVER: opens the (damaged) drawing at `path` in a new tab through the failsafe reader and
/// says what it read and dropped. A drawing open already is refused: close it first, so it is
/// read again from the file.
#[tauri::command]
pub async fn recover_drawing<R: Runtime>(
    path: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Recovered, String> {
    if tabs::find_path(&state, Path::new(&path)).is_some() {
        return Err("This drawing is open: close its tab first to recover it from the file".into());
    }
    let session = Session::open(Path::new(&path)).map_err(|e| e.to_string())?;
    let recovered = Recovered {
        info: session.info(),
        report: session.read_report(),
    };
    new_conversation(&app, &state);
    tabs::add(&state, session);
    remember_recent(&state, &path);
    Ok(recovered)
}

/// A sheet set and its drawings as the window lists them.
#[derive(Serialize)]
pub struct SheetSetView {
    pub set: SheetSet,
    pub entries: Vec<SheetSetEntry>,
}

fn view(qss: &Path, set: SheetSet) -> SheetSetView {
    SheetSetView {
        entries: sheet_set_entries(qss, &set),
        set,
    }
}

/// Reads the sheet set at `path` (a `.qss` file).
#[tauri::command]
pub async fn sheet_set_open(path: String) -> Result<SheetSetView, String> {
    let qss = Path::new(&path);
    let set = SheetSet::load(qss).map_err(|e| e.to_string())?;
    Ok(view(qss, set))
}

/// Writes `set` to `path` (a `.qss` file; nothing else is written).
#[tauri::command]
pub async fn sheet_set_save(path: String, set: SheetSet) -> Result<SheetSetView, String> {
    let qss = Path::new(&path);
    set.save(qss).map_err(|e| e.to_string())?;
    Ok(view(qss, set))
}

/// The layout names of `s`, in tab order.
fn layouts(s: &Session) -> Vec<String> {
    s.drawing
        .sheets()
        .into_iter()
        .filter(|sheet| !sheet.is_model)
        .map(|sheet| sheet.name)
        .collect()
}

/// The layouts of the drawing at `path`: its open tab's, else read from the file.
fn layouts_of(state: &AppState, path: &Path) -> Result<Vec<String>, String> {
    let _list = lock(&state.tabs);
    if let Some(s) = lock(&state.session).as_ref() {
        if crate::settings::same_file(&s.drawing.path, path) {
            return Ok(layouts(s));
        }
    }
    if let Some(p) = lock(&state.parked)
        .iter()
        .find(|p| crate::settings::same_file(&p.session.drawing.path, path))
    {
        return Ok(layouts(&p.session));
    }
    let drawing = Drawing::open(path).map_err(|e| e.to_string())?;
    Ok(drawing
        .sheets()
        .into_iter()
        .filter(|sheet| !sheet.is_model)
        .map(|sheet| sheet.name)
        .collect())
}

/// The sheet set drawing for the file `drawing` with all its layouts, its path relative to the
/// sheet set `qss`; refused when it is outside the set's folder.
#[tauri::command]
pub async fn sheet_set_entry(
    qss: String,
    drawing: String,
    state: State<'_, AppState>,
) -> Result<SheetSetDrawing, String> {
    let layouts = layouts_of(&state, Path::new(&drawing))?;
    entry_for(Path::new(&qss), Path::new(&drawing), layouts).map_err(|e| e.to_string())
}

/// A new sheet set `name` at `qss` (not written yet) holding every open drawing in its folder
/// with all their layouts; `skipped` names the open drawings outside it.
#[derive(Serialize)]
pub struct FromTabs {
    pub set: SheetSet,
    pub skipped: Vec<String>,
}

#[tauri::command]
pub async fn sheet_set_from_tabs(
    qss: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<FromTabs, String> {
    let qss = Path::new(&qss);
    let mut set = SheetSet {
        name,
        drawings: Vec::new(),
    };
    let mut skipped = Vec::new();
    let mut add = |s: &Session| match entry_for(qss, &s.drawing.path, layouts(s)) {
        Ok(d) => set.drawings.push(d),
        Err(_) => skipped.push(s.drawing.path.display().to_string()),
    };
    let list = lock(&state.tabs);
    let session = lock(&state.session);
    let parked = lock(&state.parked);
    for id in &list.order {
        if list.active == Some(*id) {
            if let Some(s) = session.as_ref() {
                add(s);
            }
        } else if let Some(p) = parked.iter().find(|p| p.id == *id) {
            add(&p.session);
        }
    }
    Ok(FromTabs { set, skipped })
}

/// The paper-space viewports of the sheet `sheet` (the one on screen when left out) and how
/// the window draws model space inside each (`Session::paper_views_of`).
#[tauri::command]
pub async fn paper_views(
    sheet: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<qiyas_core::PaperView>, String> {
    with_session(&state, |s| {
        Ok(match sheet {
            Some(sheet) => s.paper_views_of(&sheet),
            None => s.paper_views(),
        })
    })
}

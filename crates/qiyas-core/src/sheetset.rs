//! Sheet sets: a `.qss` file (JSON) naming a set of drawings and the layouts of each that belong
//! to it, `{ "name": …, "drawings": [{ "path": …, "layouts": [names] }] }`. Paths are relative to
//! the `.qss` file, written with `/`, and stay inside its folder (or one below it): a drawing
//! elsewhere is refused, and so is a file whose paths climb out or are absolute, so the set
//! never opens or names anything outside its folder. Saving writes the `.qss` file only. An
//! empty `layouts` means all of the drawing's layouts. Printing the set comes with plotting
//! (Phase 4).

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::write_atomic;
use crate::CoreError;

fn invalid(why: impl Into<String>) -> CoreError {
    CoreError::InvalidCommand(why.into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetSet {
    pub name: String,
    pub drawings: Vec<SheetSetDrawing>,
}

/// One drawing of a sheet set: its path relative to the `.qss` file, and its layouts in the set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetSetDrawing {
    pub path: String,
    #[serde(default)]
    pub layouts: Vec<String>,
}

/// A drawing of a sheet set as the window lists it: `full` its path on this computer, `missing`
/// when no file is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SheetSetEntry {
    pub path: String,
    pub layouts: Vec<String>,
    pub full: String,
    pub missing: bool,
}

/// The folder of the `.qss` file `qss`.
fn folder(qss: &Path) -> &Path {
    qss.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// `path` is a relative path that stays inside the folder: plain names only, no `..`, no root.
fn inside(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && !path.starts_with(['/', '\\'])
        && p.components().all(|c| matches!(c, Component::Normal(_)))
}

fn check(set: &SheetSet) -> Result<(), CoreError> {
    match set.drawings.iter().find(|d| !inside(&d.path)) {
        Some(d) => Err(invalid(format!(
            "{} is outside the sheet set's folder: a sheet set keeps its drawings beside it",
            d.path
        ))),
        None => Ok(()),
    }
}

impl SheetSet {
    /// Reads the sheet set at `qss`; refused when a path reaches outside its folder.
    pub fn load(qss: &Path) -> Result<SheetSet, CoreError> {
        let text = std::fs::read_to_string(qss)?;
        let set: SheetSet = serde_json::from_str(&text)
            .map_err(|e| invalid(format!("{} is no sheet set: {e}", qss.display())))?;
        check(&set)?;
        Ok(set)
    }

    /// Writes the sheet set to `qss` (a `.qss` file), atomically; refused when a path reaches
    /// outside its folder.
    pub fn save(&self, qss: &Path) -> Result<(), CoreError> {
        let is_qss = qss
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("qss"));
        if !is_qss {
            return Err(invalid("Save the sheet set as a .qss file"));
        }
        check(self)?;
        let text =
            serde_json::to_string_pretty(self).map_err(|e| CoreError::Project(e.to_string()))?;
        Ok(write_atomic(qss, text.as_bytes())?)
    }
}

/// The sheet set drawing for the file `drawing` (inside the folder of `qss`), with `layouts`.
pub fn sheet_set_entry(
    qss: &Path,
    drawing: &Path,
    layouts: Vec<String>,
) -> Result<SheetSetDrawing, CoreError> {
    let base = folder(qss).canonicalize()?;
    let full = drawing.canonicalize()?;
    let relative = full.strip_prefix(&base).map_err(|_| {
        invalid(format!(
            "{} is outside the sheet set's folder {}",
            drawing.display(),
            base.display()
        ))
    })?;
    let path = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    Ok(SheetSetDrawing { path, layouts })
}

/// The drawings of `set` (read from `qss`) with their paths on this computer.
pub fn sheet_set_entries(qss: &Path, set: &SheetSet) -> Vec<SheetSetEntry> {
    let base = folder(qss);
    set.drawings
        .iter()
        .map(|d| {
            let full: PathBuf = d.path.split('/').fold(base.to_path_buf(), |p, c| p.join(c));
            SheetSetEntry {
                path: d.path.clone(),
                layouts: d.layouts.clone(),
                missing: !full.is_file(),
                full: full.display().to_string(),
            }
        })
        .collect()
}

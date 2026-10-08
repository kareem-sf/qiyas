//! The `.qiyas` project file: the take-off, saved as JSON next to the drawing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{CoreError, Takeoff};

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    /// Always 1 for now.
    pub version: u32,
    /// The drawing's file name, without its folder.
    pub drawing: String,
    pub takeoff: Takeoff,
}

/// The project file of `drawing`: same folder, same stem, extension `qiyas`.
pub fn project_path(drawing: &Path) -> PathBuf {
    drawing.with_extension("qiyas")
}

/// The file name of `path`, or "" when it has none.
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Reads the project file at `path`.
pub(crate) fn load(path: &Path) -> Result<Project, CoreError> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| CoreError::Project(e.to_string()))
}

/// Writes `project` to `path` as pretty JSON: to `<path>.tmp` first, then renamed over `path`,
/// so a crash mid-write never leaves a half-written project.
pub(crate) fn save(path: &Path, project: &Project) -> Result<(), CoreError> {
    let text =
        serde_json::to_string_pretty(project).map_err(|e| CoreError::Project(e.to_string()))?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

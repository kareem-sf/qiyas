//! Autosave and recovery. `Session::autosave` writes a drawing with unsaved changes to
//! `<name>.sv$` beside it (a whole DWG of the drawing as it is in memory, whatever its own
//! format) and its project to `<name>.sv$.qiyas`, each into a temp file renamed over the last
//! (never while a block is edited in place: its temporary copies are on the sheet). The window
//! asks every few minutes. `recoverable` finds an autosave newer than its drawing (or one whose
//! drawing is gone); `Session::open_autosave` opens it as a new, unsaved drawing beside the
//! original (`<name> (recovered).<ext>`) with its take-off, and `discard_autosave` deletes it. A
//! successful save deletes the drawing's autosave.
//!
//! Recover (opening a damaged file) is an ordinary open: the reader is failsafe already;
//! `Session::read_report` says what it read and what it dropped, by kind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use opencadcodec::{DwgWriter, ReadStats};
use serde::Serialize;

use crate::project::{self, write_atomic};
use crate::{CoreError, Session};

/// The autosave of `drawing`: `<stem>.sv$` in its folder.
pub fn autosave_path(drawing: &Path) -> PathBuf {
    drawing.with_extension("sv$")
}

/// The project kept with the autosave of `drawing`: `<stem>.sv$.qiyas`.
fn autosave_project(drawing: &Path) -> PathBuf {
    let mut name = autosave_path(drawing).into_os_string();
    name.push(".qiyas");
    PathBuf::from(name)
}

/// An autosave the window offers back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recoverable {
    /// The drawing it is the autosave of.
    pub drawing: String,
    pub autosave: String,
    /// When it was written, in seconds since 1970.
    pub saved_at: u64,
}

fn modified(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The autosave of `drawing` when there is one newer than the drawing (or the drawing is gone).
pub fn recoverable(drawing: &Path) -> Option<Recoverable> {
    let autosave = autosave_path(drawing);
    let saved = modified(&autosave)?;
    if modified(drawing).is_some_and(|d| d >= saved) {
        return None;
    }
    Some(Recoverable {
        drawing: drawing.display().to_string(),
        autosave: autosave.display().to_string(),
        saved_at: saved.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
    })
}

/// Deletes the autosave of `drawing` and its project; files already gone are no error.
pub fn discard_autosave(drawing: &Path) -> Result<(), CoreError> {
    for path in [autosave_path(drawing), autosave_project(drawing)] {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}

/// Where a recovered copy of `drawing` goes: `<stem> (recovered).<ext>` beside it, numbered when
/// that file exists.
fn recovered_path(drawing: &Path) -> PathBuf {
    let stem = drawing
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = drawing
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dwg".to_string());
    (1..)
        .map(|n| {
            let tag = if n == 1 {
                "recovered".to_string()
            } else {
                format!("recovered {n}")
            };
            drawing.with_file_name(format!("{stem} ({tag}).{ext}"))
        })
        .find(|p| !p.exists())
        .expect("a free name")
}

/// What the failsafe reader made of a file: its records, how many it decoded and skipped, the
/// entities it read, the errors it recovered from, and what it dropped by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReadReport {
    pub records: usize,
    pub decoded: usize,
    pub skipped: usize,
    pub entities: usize,
    pub recovered_errors: usize,
    pub dropped: Vec<Dropped>,
}

/// So many records of one kind dropped: a DWG object type ("LINE", "LAYER"…; "other" for a
/// number the reader could not trust), or what went wrong for those without one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dropped {
    pub kind: String,
    pub count: usize,
}

/// The name of the DWG object type `code`, for the common fixed ones.
fn object_type(code: &str) -> Option<&'static str> {
    Some(match code.trim().parse::<i64>().ok()? {
        1 => "TEXT",
        2 => "ATTRIB",
        3 => "ATTDEF",
        4 => "BLOCK",
        5 => "ENDBLK",
        6 => "SEQEND",
        7 => "INSERT",
        8 => "MINSERT",
        10..=14 => "VERTEX",
        15 | 16 | 29 | 30 => "POLYLINE",
        17 => "ARC",
        18 => "CIRCLE",
        19 => "LINE",
        20..=26 => "DIMENSION",
        27 => "POINT",
        28 => "3DFACE",
        31 => "SOLID",
        32 => "TRACE",
        33 => "SHAPE",
        34 => "VIEWPORT",
        35 => "ELLIPSE",
        36 => "SPLINE",
        40 => "RAY",
        41 => "XLINE",
        42 => "DICTIONARY",
        44 => "MTEXT",
        45 => "LEADER",
        46 => "TOLERANCE",
        47 => "MLINE",
        48..=50 | 52 | 56 | 60 | 62 | 64 | 66 | 68 | 70 => "TABLE CONTROL",
        51 => "LAYER",
        53 => "STYLE",
        57 => "LTYPE",
        61 => "VIEW",
        63 => "UCS",
        65 => "VPORT",
        67 => "APPID",
        69 => "DIMSTYLE",
        77 => "LWPOLYLINE",
        78 => "HATCH",
        79 => "XRECORD",
        82 => "LAYOUT",
        _ => return None,
    })
}

impl ReadReport {
    pub(crate) fn from_stats(stats: &ReadStats) -> ReadReport {
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for d in &stats.diagnostics {
            let kind = match &d.record_type {
                Some(t) => object_type(t)
                    .map(str::to_string)
                    .or_else(|| {
                        // A DXF record type is its name already.
                        let name = t.trim();
                        (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphabetic()))
                            .then(|| name.to_uppercase())
                    })
                    .unwrap_or_else(|| "other".to_string()),
                None => d.code.clone(),
            };
            *kinds.entry(kind).or_insert(0) += 1;
        }
        ReadReport {
            records: stats.source_records,
            decoded: stats.decoded_source_records,
            skipped: stats.skipped_source_records,
            entities: stats.output_entities,
            recovered_errors: stats.recovered_errors,
            dropped: kinds
                .into_iter()
                .map(|(kind, count)| Dropped { kind, count })
                .collect(),
        }
    }
}

impl Session {
    /// Writes the autosave of a drawing with unsaved changes (see the module documentation);
    /// the autosave's path, or `None` when there was nothing to write.
    pub fn autosave(&self) -> Result<Option<PathBuf>, CoreError> {
        if !self.is_dirty() || self.block_edit.is_some() {
            return Ok(None);
        }
        let path = autosave_path(&self.drawing.path);
        let bytes = DwgWriter::write_to_vec(&self.drawing.doc)
            .map_err(|e| CoreError::Unwritable(e.to_string()))?;
        write_atomic(&path, &bytes)?;
        project::save(&autosave_project(&self.drawing.path), &self.project())?;
        Ok(Some(path))
    }

    /// Opens the autosave of `drawing` as a new drawing with unsaved changes beside it (see the
    /// module documentation), with the take-off, BOQ and settings its project kept.
    pub fn open_autosave(drawing: &Path) -> Result<Session, CoreError> {
        let autosave = autosave_path(drawing);
        if !autosave.exists() {
            return Err(CoreError::InvalidCommand(format!(
                "{} has no autosave",
                drawing.display()
            )));
        }
        let name = project::file_name(drawing);
        let mut s = Session::open_with(&autosave, &autosave_project(drawing), &name)?;
        s.drawing.path = recovered_path(drawing);
        s.mark_unsaved();
        Ok(s)
    }

    /// What the reader made of the drawing's file (Recover's report).
    pub fn read_report(&self) -> ReadReport {
        self.drawing.report.clone()
    }
}

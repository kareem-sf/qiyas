//! Autosave and recovery. `Session::autosave` writes a drawing with unsaved changes to
//! `<file name>.sv$` beside it (a whole DWG of the drawing as it is in memory, whatever its own
//! format) and its project to `<file name>.sv$.qiyas`, each into a temp file renamed over the last
//! (never while a block is edited in place: its temporary copies are on the sheet). The window
//! asks every few minutes. `recoverable` finds an autosave newer than its drawing (or one whose
//! drawing is gone); `Session::open_autosave` opens it as a new, unsaved drawing beside the
//! original (`<name> (recovered).<ext>`) with its take-off, and `discard_autosave` deletes it. A
//! successful save deletes the drawing's autosave (and a recovered drawing's, the autosave it came
//! from); so does closing it without saving (the desktop app).
//!
//! Besides the window's timer, a drawing is autosaved after every `AUTOSAVE_EVERY` changes to it
//! (`Session::autosave_due`, which the desktop app's 1 s watch asks; a save or an autosave starts
//! the count again), and the app's panic hook autosaves every drawing with changes, best effort.
//!
//! While a drawing is open the desktop app keeps a lock file beside it, `<file name>.qlock`
//! (`DrawingLock` as JSON: process id, computer and when it was taken), taken by its watch and
//! removed when the drawing is closed or Qiyas quits. A lock left on this computer by a process
//! that has ended means Qiyas closed unexpectedly: `recoverable` then offers the autosave even
//! when it is older than the drawing (`crashed`). A live process's lock, or another computer's,
//! is reported (`locked_elsewhere`) and never taken over; the drawing still opens as usual.
//! `take_lock` cleans a dead lock up when the drawing is opened again, `discard_autosave` when
//! its autosave is dealt with. Qiyas itself never writes a lock without the app asking.
//!
//! Recover (opening a damaged file) is an ordinary open: the reader is failsafe already;
//! `Session::read_report` says what it read and what it dropped, by kind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use opencadcodec::{DwgWriter, ReadStats};
use serde::{Deserialize, Serialize};

use crate::project::{self, write_atomic};
use crate::{CoreError, Session};

/// The autosave of `drawing`: `<file name>.sv$` in its folder (`plan.dwg.sv$`), so a DWG and a
/// DXF of one name keep their own.
pub fn autosave_path(drawing: &Path) -> PathBuf {
    let mut name = drawing.as_os_str().to_owned();
    name.push(".sv$");
    PathBuf::from(name)
}

/// The project kept with the autosave of `drawing`: `<file name>.sv$.qiyas`.
fn autosave_project(drawing: &Path) -> PathBuf {
    let mut name = autosave_path(drawing).into_os_string();
    name.push(".qiyas");
    PathBuf::from(name)
}

/// A drawing is autosaved after this many changes to it (steps, undos and redos) since it was
/// last saved or autosaved, besides the window's timer.
pub const AUTOSAVE_EVERY: u64 = 20;

/// The lock file of `drawing` while Qiyas has it open: `<file name>.qlock` (`plan.dwg.qlock`).
pub fn lock_path(drawing: &Path) -> PathBuf {
    let mut name = drawing.as_os_str().to_owned();
    name.push(".qlock");
    PathBuf::from(name)
}

/// Who has a drawing open: the process, its computer, and when it took the lock (seconds since
/// 1970).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawingLock {
    pub pid: u32,
    pub host: String,
    pub started_at: u64,
}

/// What a lock beside a drawing means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockStatus {
    /// This process's.
    Mine,
    /// A process on this computer that has ended: Qiyas closed unexpectedly (a stale lock).
    Dead,
    /// Another process on this computer, still running.
    Alive,
    /// A process on another computer (a shared folder): whether it still runs can't be told, so
    /// the drawing counts as open there.
    OtherHost,
}

impl DrawingLock {
    /// This process's lock, taken now.
    fn here() -> DrawingLock {
        DrawingLock {
            pid: std::process::id(),
            host: host_name(),
            started_at: secs(SystemTime::now()),
        }
    }

    pub fn status(&self) -> LockStatus {
        if self.host != host_name() {
            LockStatus::OtherHost
        } else if self.pid == std::process::id() {
            LockStatus::Mine
        } else if process_alive(self.pid) {
            LockStatus::Alive
        } else {
            LockStatus::Dead
        }
    }
}

/// The lock beside `drawing`, when there is one Qiyas can read (an unreadable one counts as none).
pub fn read_lock(drawing: &Path) -> Option<DrawingLock> {
    serde_json::from_slice(&std::fs::read(lock_path(drawing)).ok()?).ok()
}

/// Locks `drawing` for this process (the desktop app, once it is open): writes the lock file,
/// whole, unless a running process or another computer has one there (left as it is: false). A
/// dead or unreadable lock is replaced: stale locks are cleaned up when the drawing is opened again.
pub fn take_lock(drawing: &Path) -> Result<bool, CoreError> {
    match read_lock(drawing).as_ref().map(DrawingLock::status) {
        Some(LockStatus::Alive | LockStatus::OtherHost) => return Ok(false),
        Some(LockStatus::Mine) => return Ok(true),
        Some(LockStatus::Dead) | None => {}
    }
    let json = serde_json::to_vec(&DrawingLock::here())
        .map_err(|e| CoreError::Unwritable(e.to_string()))?;
    write_atomic(&lock_path(drawing), &json)?;
    Ok(true)
}

/// Removes this process's lock beside `drawing` (closed, or Qiyas quits); another's, or none, is
/// left as it is.
pub fn release_lock(drawing: &Path) -> Result<(), CoreError> {
    if read_lock(drawing).is_some_and(|l| l.status() == LockStatus::Mine) {
        remove_if_there(&lock_path(drawing))?;
    }
    Ok(())
}

/// Whether process `pid` on this computer still runs. A process of another user counts
/// (`EPERM`); a zombie too, until its parent reaps it.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    match libc::pid_t::try_from(pid) {
        // SAFETY: signal 0 sends nothing; it only asks whether the process exists. A pid of 0 or
        // less would mean a process group, so those never get here.
        Ok(pid) if pid > 0 => {
            let sent = unsafe { libc::kill(pid, 0) };
            sent == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
        }
        _ => false,
    }
}

/// Whether process `pid` still runs, from `tasklist` (std only, no console window). When it
/// can't be told, it counts as running: then a drawing is never taken for a crash.
#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let filter = format!("PID eq {pid}");
    std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH", "/FI", &filter])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_or(true, |out| {
            String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
        })
}

#[cfg(not(any(unix, windows)))]
fn process_alive(_pid: u32) -> bool {
    true
}

/// This computer's name.
#[cfg(unix)]
fn host_name() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: gethostname writes at most `buf.len()` bytes into `buf`.
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[cfg(windows)]
fn host_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_default()
}

#[cfg(not(any(unix, windows)))]
fn host_name() -> String {
    String::new()
}

/// An autosave the window offers back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recoverable {
    /// The drawing it is the autosave of.
    pub drawing: String,
    pub autosave: String,
    /// When it was written, in seconds since 1970.
    pub saved_at: u64,
    /// When the drawing was last written, in seconds since 1970; `None` when it is gone.
    pub drawing_at: Option<u64>,
    /// Qiyas closed unexpectedly with the drawing open: a lock on this computer whose process
    /// has ended.
    pub crashed: bool,
    /// The drawing is open in another Qiyas: a running process's lock, or another computer's.
    pub locked_elsewhere: Option<DrawingLock>,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The autosave of `drawing` when there is one newer than the drawing (or the drawing is gone),
/// or Qiyas closed unexpectedly with the drawing open (a dead lock: then whatever its age).
pub fn recoverable(drawing: &Path) -> Option<Recoverable> {
    let autosave = autosave_path(drawing);
    let saved = modified(&autosave)?;
    let drawing_at = modified(drawing);
    let lock = read_lock(drawing);
    let status = lock.as_ref().map(DrawingLock::status);
    let crashed = status == Some(LockStatus::Dead);
    if !crashed && drawing_at.is_some_and(|d| d >= saved) {
        return None;
    }
    Some(Recoverable {
        drawing: drawing.display().to_string(),
        autosave: autosave.display().to_string(),
        saved_at: secs(saved),
        drawing_at: drawing_at.map(secs),
        crashed,
        locked_elsewhere: lock
            .filter(|_| matches!(status, Some(LockStatus::Alive | LockStatus::OtherHost))),
    })
}

fn remove_if_there(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Deletes the autosave of `drawing` and its project; files already gone are no error. A dead
/// lock beside the drawing goes too: the crash it told of is dealt with.
pub fn discard_autosave(drawing: &Path) -> Result<(), CoreError> {
    for path in [autosave_path(drawing), autosave_project(drawing)] {
        remove_if_there(&path)?;
    }
    if read_lock(drawing).is_some_and(|l| l.status() == LockStatus::Dead) {
        remove_if_there(&lock_path(drawing))?;
    }
    Ok(())
}

/// What one autosave round did: the autosaves written, and why others could not be.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AutosaveRun {
    pub written: Vec<String>,
    pub failed: Vec<String>,
}

/// Autosaves each of `sessions` with unsaved changes (`Session::autosave`): the window's timer,
/// the desktop app's watch (those due) and its panic hook.
pub fn autosave_all<'a>(sessions: impl IntoIterator<Item = &'a Session>) -> AutosaveRun {
    let mut run = AutosaveRun::default();
    for s in sessions {
        match s.autosave() {
            Ok(Some(path)) => run.written.push(path.display().to_string()),
            Ok(None) => {}
            Err(e) => run
                .failed
                .push(format!("{}: {e}", s.drawing.path.display())),
        }
    }
    run
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
    /// The changes to the drawing (steps, undos and redos) since it was last saved, or an
    /// autosave of it was written or tried.
    pub fn changes_since_autosave(&self) -> u64 {
        self.document_revision
            .saturating_sub(self.saved_revision.max(self.autosave_revision.get()))
    }

    /// `AUTOSAVE_EVERY` changes or more since then: the desktop app's watch autosaves it now.
    pub fn autosave_due(&self) -> bool {
        self.changes_since_autosave() >= AUTOSAVE_EVERY
    }

    /// Writes the autosave of a drawing with unsaved changes (see the module documentation);
    /// the autosave's path, or `None` when there was nothing to write.
    pub fn autosave(&self) -> Result<Option<PathBuf>, CoreError> {
        if !self.is_dirty() || self.block_edit.is_some() {
            return Ok(None);
        }
        // Counted whether it is written or not: a failing disk is tried again by the window's
        // timer, not after every change.
        self.autosave_revision.set(self.document_revision);
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
        // Its first save deletes the autosave, so it is not offered again.
        s.recovered_from = Some(drawing.to_path_buf());
        Ok(s)
    }

    /// What the reader made of the drawing's file (Recover's report).
    pub fn read_report(&self) -> ReadReport {
        self.drawing.report.clone()
    }
}

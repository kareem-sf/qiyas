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
//! (`DrawingLock` as JSON: process id, computer and when that process started), taken by its
//! watch and removed when the drawing is closed or Qiyas quits. A lock on this computer is alive
//! while its process runs and started when the lock says (a pid taken over by another process
//! is dead). A dead lock means Qiyas closed unexpectedly: `recoverable` then offers the autosave
//! even when it is older than the drawing (`crashed`). A live process's lock, or another
//! computer's, is reported (`locked_elsewhere`), never taken over, and its autosave is never
//! discarded; the drawing still opens as usual. `recoverable` answers whenever there is an
//! autosave or such a lock; the window decides what to show.
//!
//! When a crashed drawing is opened again (`take_lock`, or an autosave before it) the crash's
//! autosave is kept aside as a crash copy, `<file name>.sv$.crash-<yyyymmdd-hhmmss>` (when it was
//! written, UTC) with its project, at most `CRASH_COPIES` of them, and the dead lock goes. The
//! window asks about a drawing's autosave before every open, so a crash copy has been offered
//! once already (as the autosave the crash left): `recoverable` lists it under `older_copies`,
//! never as the autosave offered; Recover or Discard one by its path.
//!
//! An autosave never writes over another Qiyas's: while another process (or computer) holds the
//! drawing's lock (Open anyway), this process autosaves to `<file name>.sv$.<pid>` instead, offered
//! last and deleted by a save or Don't Save like the main one. Qiyas itself never writes a lock
//! without the app asking.
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

/// The project kept with the autosave (or crash copy) `autosave`: its name and `.qiyas`.
fn project_of(autosave: &Path) -> PathBuf {
    let mut name = autosave.as_os_str().to_owned();
    name.push(".qiyas");
    PathBuf::from(name)
}

/// Crash copies kept per drawing; the oldest go.
pub const CRASH_COPIES: usize = 3;

/// What a crash copy's name has after its drawing's.
const CRASH_TAG: &str = ".sv$.crash-";

/// A crash copy of `drawing`'s autosave written at `saved`:
/// `<file name>.sv$.crash-<yyyymmdd-hhmmss>`, UTC.
fn crash_copy_path(drawing: &Path, saved: SystemTime) -> PathBuf {
    let t = time::OffsetDateTime::from(saved);
    let mut name = drawing.as_os_str().to_owned();
    name.push(format!(
        "{CRASH_TAG}{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    ));
    PathBuf::from(name)
}

/// This process's autosave of `drawing` while another Qiyas holds its lock:
/// `<file name>.sv$.<pid>`.
fn instance_autosave(drawing: &Path, pid: u32) -> PathBuf {
    let mut name = autosave_path(drawing).into_os_string();
    name.push(format!(".{pid}"));
    PathBuf::from(name)
}

/// The drawing an autosave copy belongs to: a crash copy (`<file>.sv$.crash-…`) or an instance
/// autosave (`<file>.sv$.<pid>`); `None` for any other path (a drawing, `<file>.sv$`, a project).
pub fn autosave_copy_of(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let (stem, rest) = name.rsplit_once(".sv$.")?;
    let copy = rest.starts_with("crash-") && !rest.ends_with(".qiyas") && !rest.ends_with(".tmp")
        || !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit());
    copy.then(|| path.with_file_name(stem))
}

/// The autosave copies beside `drawing` (not their projects), by name: its crash copies
/// (`crash`, so oldest first), or the instance autosaves.
fn autosave_copies(drawing: &Path, crash: bool) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (drawing.parent(), drawing.file_name()) else {
        return Vec::new();
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let prefix = format!("{}{CRASH_TAG}", name.to_string_lossy());
    let mut copies: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| drawing.with_file_name(e.file_name()))
        .filter(|p| {
            autosave_copy_of(p).is_some_and(|d| d.file_name() == Some(name))
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(&prefix) == crash)
        })
        .collect();
    copies.sort();
    copies
}

/// The crash copies of `drawing` (not their projects), oldest first.
fn crash_copies(drawing: &Path) -> Vec<PathBuf> {
    autosave_copies(drawing, true)
}

/// Qiyas closed unexpectedly with `drawing` open (its lock is dead) and it is opened again: the
/// autosave the crash left is kept as a crash copy with its project before anything writes over
/// it, at most `CRASH_COPIES` of them (the oldest go), and the dead lock goes.
fn keep_crash_autosave(drawing: &Path) -> Result<(), CoreError> {
    let autosave = autosave_path(drawing);
    if let Some(saved) = modified(&autosave) {
        let copy = crash_copy_path(drawing, saved);
        std::fs::rename(&autosave, &copy)?;
        match std::fs::rename(project_of(&autosave), project_of(&copy)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        let copies = crash_copies(drawing);
        for old in &copies[..copies.len().saturating_sub(CRASH_COPIES)] {
            remove_with_project(old)?;
        }
    }
    remove_if_there(&lock_path(drawing))?;
    Ok(())
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

/// Two readings of one process's start time may differ by this much, in seconds.
const START_SLACK: u64 = 2;

impl DrawingLock {
    /// This process's lock.
    fn here() -> DrawingLock {
        DrawingLock {
            pid: std::process::id(),
            host: host_name(),
            started_at: own_start(),
        }
    }

    /// What the lock means. Another computer's counts as open there. On this computer its
    /// process must still run and have started when the lock says; otherwise the pid now belongs
    /// to another process (after a restart, say) and the lock is dead. Where a start time can't be
    /// read (Windows), the pid alone decides.
    pub fn status(&self) -> LockStatus {
        let same_start = |start: u64| start.abs_diff(self.started_at) <= START_SLACK;
        if self.host != host_name() {
            LockStatus::OtherHost
        } else if self.pid == std::process::id() {
            if same_start(own_start()) {
                LockStatus::Mine
            } else {
                LockStatus::Dead
            }
        } else if process_alive(self.pid) && process_start(self.pid).is_none_or(same_start) {
            LockStatus::Alive
        } else {
            LockStatus::Dead
        }
    }
}

/// When this process started, in seconds since 1970; where that can't be read, when it first
/// asked.
fn own_start() -> u64 {
    static START: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *START.get_or_init(|| {
        process_start(std::process::id()).unwrap_or_else(|| secs(SystemTime::now()))
    })
}

/// The lock beside `drawing`, when there is one Qiyas can read (an unreadable one counts as none).
pub fn read_lock(drawing: &Path) -> Option<DrawingLock> {
    serde_json::from_slice(&std::fs::read(lock_path(drawing)).ok()?).ok()
}

/// Locks `drawing` for this process (the desktop app, once it is open): writes the lock file,
/// whole, unless a running process or another computer has one there (left as it is: false). A
/// dead or unreadable lock is replaced: stale locks are cleaned up when the drawing is opened
/// again, and a dead one's autosave is kept as a crash copy first.
pub fn take_lock(drawing: &Path) -> Result<bool, CoreError> {
    match read_lock(drawing).as_ref().map(DrawingLock::status) {
        Some(LockStatus::Alive | LockStatus::OtherHost) => return Ok(false),
        Some(LockStatus::Mine) => return Ok(true),
        Some(LockStatus::Dead) => keep_crash_autosave(drawing)?,
        None => {}
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
/// (`EPERM`); a zombie does not.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let exists = match libc::pid_t::try_from(pid) {
        // SAFETY: signal 0 sends nothing; it only asks whether the process exists. A pid of 0 or
        // less would mean a process group, so those never get here.
        Ok(p) if p > 0 => {
            let sent = unsafe { libc::kill(p, 0) };
            sent == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
        }
        _ => false,
    };
    exists && !zombie(pid)
}

/// The state letter and start time (seconds since 1970) of process `pid`, from
/// `/proc/<pid>/stat` (its 3rd and 22nd fields, in clock ticks after boot) and `/proc/stat`'s
/// boot time.
#[cfg(target_os = "linux")]
fn proc_stat(pid: u32) -> Option<(char, u64)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name, in parentheses, may hold spaces and parentheses itself.
    let mut fields = stat.rsplit_once(')')?.1.split_whitespace();
    let state = fields.next()?.chars().next()?;
    let ticks: u64 = fields.nth(18)?.parse().ok()?;
    // SAFETY: sysconf only reads a configuration value.
    let per_second = u64::try_from(unsafe { libc::sysconf(libc::_SC_CLK_TCK) })
        .ok()
        .filter(|&t| t > 0)?;
    let boot: u64 = std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("btime "))?
        .trim()
        .parse()
        .ok()?;
    Some((state, boot + ticks / per_second))
}

/// A zombie still answers `kill` until its parent reaps it, but it has ended.
#[cfg(target_os = "linux")]
fn zombie(pid: u32) -> bool {
    proc_stat(pid).is_some_and(|(state, _)| matches!(state, 'Z' | 'X'))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn zombie(_pid: u32) -> bool {
    false
}

/// When process `pid` started, in seconds since 1970; `None` when it can't be read.
#[cfg(target_os = "linux")]
fn process_start(pid: u32) -> Option<u64> {
    proc_stat(pid).map(|(_, start)| start)
}

/// When process `pid` started, from `ps -o etime=` (macOS and other unix; best effort, to the
/// second, untested here); `None` when `ps` can't tell.
#[cfg(all(unix, not(target_os = "linux")))]
fn process_start(pid: u32) -> Option<u64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "etime=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let elapsed = elapsed_secs(String::from_utf8_lossy(&out.stdout).trim())?;
    secs(SystemTime::now()).checked_sub(elapsed)
}

/// `ps`'s elapsed time, `[[dd-]hh:]mm:ss`, in seconds.
#[cfg(any(test, all(unix, not(target_os = "linux"))))]
fn elapsed_secs(etime: &str) -> Option<u64> {
    let (days, clock) = match etime.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().ok()?, clock),
        None => (0, etime),
    };
    let parts: Vec<&str> = clock.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let mut total = 0;
    for part in parts {
        total = total * 60 + part.parse::<u64>().ok()?;
    }
    Some(days * 86_400 + total)
}

/// Windows (and others): the start time is not read; the pid alone decides.
#[cfg(not(unix))]
fn process_start(_pid: u32) -> Option<u64> {
    None
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

/// What the window may offer back for a drawing: an autosave, a crash, or another Qiyas that has
/// it open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recoverable {
    /// The drawing it is about.
    pub drawing: String,
    /// The autosave offered: `<file>.sv$`, else this drawing's newest instance autosave
    /// (`<file>.sv$.<pid>`); `None` when there is none.
    pub autosave: Option<String>,
    /// When it was written, in seconds since 1970.
    pub saved_at: Option<u64>,
    /// When the drawing was last written, in seconds since 1970; `None` when it is gone.
    pub drawing_at: Option<u64>,
    /// Qiyas closed unexpectedly with the drawing open: a dead lock on this computer.
    pub crashed: bool,
    /// The drawing is open in another Qiyas: a running process's lock, or another computer's.
    pub locked_elsewhere: Option<DrawingLock>,
    /// Crash copies kept from earlier crashes, newest first: offered once already (as the
    /// autosave the crash left), so listed here, never as `autosave`. Recover or Discard one by
    /// its path.
    pub older_copies: Vec<OlderCopy>,
}

/// A crash copy kept beside a drawing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OlderCopy {
    pub path: String,
    /// When it was written (the crashed session's last autosave), in seconds since 1970.
    pub saved_at: u64,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// An autosave of a drawing, as offered.
struct Offer {
    path: PathBuf,
    saved: SystemTime,
}

/// The autosave offered for `drawing`: its `<file>.sv$`, whatever its age; else its newest
/// instance autosave (`<file>.sv$.<pid>`, last).
fn offered(drawing: &Path) -> Option<Offer> {
    let autosave = autosave_path(drawing);
    if let Some(saved) = modified(&autosave) {
        return Some(Offer {
            path: autosave,
            saved,
        });
    }
    autosave_copies(drawing, false)
        .into_iter()
        .filter_map(|path| Some((modified(&path)?, path)))
        .max_by_key(|(saved, _)| *saved)
        .map(|(saved, path)| Offer { path, saved })
}

/// What there is to say about `drawing` before it opens: the autosave offered (see `offered`),
/// whether Qiyas crashed with it open, who else has it open, and the crash copies kept. `None`
/// when there is none of these (this process's own lock is no news); the window decides what to
/// show.
pub fn recoverable(drawing: &Path) -> Option<Recoverable> {
    let lock = read_lock(drawing);
    let status = lock.as_ref().map(DrawingLock::status);
    let dead = status == Some(LockStatus::Dead);
    let elsewhere = matches!(status, Some(LockStatus::Alive | LockStatus::OtherHost));
    let offer = offered(drawing);
    let older_copies: Vec<OlderCopy> = crash_copies(drawing)
        .into_iter()
        .rev()
        .filter_map(|path| {
            Some(OlderCopy {
                saved_at: secs(modified(&path)?),
                path: path.display().to_string(),
            })
        })
        .collect();
    if offer.is_none() && !dead && !elsewhere && older_copies.is_empty() {
        return None;
    }
    Some(Recoverable {
        drawing: drawing.display().to_string(),
        autosave: offer.as_ref().map(|o| o.path.display().to_string()),
        saved_at: offer.as_ref().map(|o| secs(o.saved)),
        drawing_at: modified(drawing).map(secs),
        crashed: dead,
        locked_elsewhere: lock.filter(|_| elsewhere),
        older_copies,
    })
}

fn remove_if_there(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

fn remove_with_project(autosave: &Path) -> Result<(), CoreError> {
    remove_if_there(autosave)?;
    remove_if_there(&project_of(autosave))?;
    Ok(())
}

/// Refuses while another Qiyas has `drawing` open (its autosave is that Qiyas's safety net);
/// else the lock's status.
fn refuse_open_elsewhere(drawing: &Path) -> Result<Option<LockStatus>, CoreError> {
    let lock = read_lock(drawing);
    let status = lock.as_ref().map(DrawingLock::status);
    match (lock, status) {
        (Some(l), Some(LockStatus::Alive | LockStatus::OtherHost)) => {
            Err(CoreError::InvalidCommand(format!(
                "{} is open in another Qiyas (process {} on {}): its autosave stays",
                drawing.display(),
                l.pid,
                l.host
            )))
        }
        _ => Ok(status),
    }
}

/// Deletes the autosave of the drawing `path` and its project (a save, Don't Save): this
/// process's instance autosave in any case, then `<file>.sv$`, never a crash copy; files already
/// gone are no error. A dead lock beside the drawing goes too: the crash it told of is dealt
/// with. `<file>.sv$` is kept, with an error, while another Qiyas has the drawing open. `path` may
/// also be an autosave copy (`autosave_copy_of`: a recovered drawing's first save, or Discard of
/// an older copy): then that copy and its project go.
pub fn discard_autosave(path: &Path) -> Result<(), CoreError> {
    if autosave_copy_of(path).is_some() {
        return remove_with_project(path);
    }
    remove_with_project(&instance_autosave(path, std::process::id()))?;
    let status = refuse_open_elsewhere(path)?;
    remove_with_project(&autosave_path(path))?;
    if status == Some(LockStatus::Dead) {
        remove_if_there(&lock_path(path))?;
    }
    Ok(())
}

/// Discard in the recovery dialog: deletes the autosave `recoverable` offers for `drawing` (its
/// `<file>.sv$`, else an instance autosave) with its project, and a dead lock beside it; crash
/// copies stay (each is discarded by its path). Refused while another Qiyas has the drawing open.
pub fn discard_recoverable(drawing: &Path) -> Result<(), CoreError> {
    let dead = refuse_open_elsewhere(drawing)? == Some(LockStatus::Dead);
    if let Some(offer) = offered(drawing) {
        remove_with_project(&offer.path)?;
    }
    if dead {
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
        let drawing = &self.drawing.path;
        let path = match read_lock(drawing).as_ref().map(DrawingLock::status) {
            // Opened again after a crash, before the app's watch took the lock: what the crash
            // autosaved is kept aside before this session's autosave takes its place.
            Some(LockStatus::Dead) => {
                keep_crash_autosave(drawing)?;
                autosave_path(drawing)
            }
            // Another Qiyas has it open (Open anyway): never over that Qiyas's autosave.
            Some(LockStatus::Alive | LockStatus::OtherHost) => {
                instance_autosave(drawing, std::process::id())
            }
            Some(LockStatus::Mine) | None => autosave_path(drawing),
        };
        let bytes = DwgWriter::write_to_vec(&self.drawing.doc)
            .map_err(|e| CoreError::Unwritable(e.to_string()))?;
        write_atomic(&path, &bytes)?;
        project::save(&project_of(&path), &self.project())?;
        Ok(Some(path))
    }

    /// Opens an autosave as a new drawing with unsaved changes beside its drawing (see the
    /// module documentation), with the take-off, BOQ and settings its project kept: for a
    /// drawing, the one `recoverable` offers; for an autosave copy (`autosave_copy_of`: a crash
    /// copy from `older_copies`, or an instance autosave), that copy.
    pub fn open_autosave(path: &Path) -> Result<Session, CoreError> {
        let none = || CoreError::InvalidCommand(format!("{} has no autosave", path.display()));
        let (drawing, opened) = match autosave_copy_of(path) {
            Some(drawing) if path.exists() => (drawing, path.to_path_buf()),
            Some(_) => return Err(none()),
            None => (path.to_path_buf(), offered(path).ok_or_else(none)?.path),
        };
        let name = project::file_name(&drawing);
        let mut s = Session::open_with(&opened, &project_of(&opened), &name)?;
        s.drawing.path = recovered_path(&drawing);
        s.mark_unsaved();
        // Its first save deletes what it came from (`discard_autosave` of the drawing, or of the
        // copy), so it is not offered again.
        s.recovered_from = Some(if opened == autosave_path(&drawing) {
            drawing
        } else {
            opened
        });
        Ok(s)
    }

    /// What the reader made of the drawing's file (Recover's report).
    pub fn read_report(&self) -> ReadReport {
        self.drawing.report.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_elapsed_time_reads_minutes_hours_and_days() {
        assert_eq!(elapsed_secs("05:07"), Some(307));
        assert_eq!(elapsed_secs("01:02:03"), Some(3723));
        assert_eq!(elapsed_secs("2-01:02:03"), Some(2 * 86_400 + 3723));
        assert_eq!(elapsed_secs(""), None);
        assert_eq!(elapsed_secs("7"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_process_start_time_is_read_from_proc() {
        let mine = process_start(std::process::id()).unwrap();
        let now = secs(SystemTime::now());
        assert!(mine <= now && now - mine < 600, "{mine} {now}");
        assert_eq!(own_start(), mine);
        // Pid 1 started first.
        assert!(process_start(1).unwrap() <= mine);
        assert!(!zombie(std::process::id()));
    }
}

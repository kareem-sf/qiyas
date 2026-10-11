//! Crash recovery: a lock file beside each open drawing (`<file>.qlock`: pid, host and start
//! time as JSON), a dead one meaning Qiyas closed unexpectedly; the autosave after every 20
//! document changes; every open drawing autosaved in one go.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use opencadcodec::Handle;
use qiyas_core::fixtures::{sample_doc, write_drawing, write_sample};
use qiyas_core::{
    autosave_all, autosave_path, discard_autosave, lock_path, read_lock, recoverable, release_lock,
    take_lock, Command, DrawSpec, DrawingLock, LockStatus, Outcome, Session, AUTOSAVE_EVERY,
};

/// The drawing the child process opens (see `recovery_child`).
const CHILD: &str = "QIYAS_RECOVERY_CHILD";

fn line(s: &mut Session, y: f64) -> u64 {
    match s
        .run(Command::Draw(DrawSpec::Line {
            a: [100.0, y],
            b: [101.0, y],
        }))
        .unwrap()
    {
        Outcome::Added { handles } => handles[0],
        other => panic!("{other:?}"),
    }
}

fn measure(s: &mut Session) {
    s.run(Command::MeasureDistance {
        points: vec![[0.0, 0.0], [3.0, 4.0]],
    })
    .unwrap();
}

/// Makes the file at `path` look `secs` seconds old.
fn modified_ago(path: &Path, secs: u64) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(secs))
        .unwrap();
}

fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// This test binary run again as a child process; `drawing` set makes it `recovery_child`'s.
fn child(drawing: Option<&Path>) -> Process {
    let mut p = Process::new(std::env::current_exe().unwrap());
    p.args([
        "recovery_child",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ]);
    p.env_remove(CHILD);
    if let Some(drawing) = drawing {
        p.env(CHILD, drawing);
    }
    p
}

/// The pid of a process that has ended (and been reaped).
fn dead_pid() -> u32 {
    let mut c = child(None).stdout(Stdio::null()).spawn().unwrap();
    let pid = c.id();
    c.wait().unwrap();
    pid
}

/// This computer's name as a lock records it.
fn this_host() -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dxf");
    assert!(take_lock(&path).unwrap());
    let host = read_lock(&path).unwrap().host;
    release_lock(&path).unwrap();
    host
}

/// Not a test by itself: the child process of
/// `a_killed_qiyas_leaves_a_dead_lock_and_its_autosave_is_offered`. Does nothing unless
/// `QIYAS_RECOVERY_CHILD` names a drawing: then opens it, draws a line, measures, autosaves,
/// locks it as the app does, says `READY:<the line's handle>` and waits to be killed.
#[test]
fn recovery_child() {
    let Some(path) = std::env::var_os(CHILD) else {
        return;
    };
    let path = PathBuf::from(path);
    let mut s = Session::open(&path).unwrap();
    let added = line(&mut s, 100.0);
    measure(&mut s);
    s.autosave().unwrap().expect("an autosave");
    assert!(take_lock(&path).unwrap());
    let mut out = std::io::stdout();
    writeln!(out, "READY:{added}").unwrap();
    out.flush().unwrap();
    // Killed long before; never left behind if the parent dies first.
    std::thread::sleep(Duration::from_secs(120));
}

/// Kills the child process when the test ends, passed or failed.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_killed_qiyas_leaves_a_dead_lock_and_its_autosave_is_offered() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    modified_ago(&path, 3600);
    let mut running = Running(child(Some(&path)).stdout(Stdio::piped()).spawn().unwrap());
    let pid = running.0.id();
    let out = BufReader::new(running.0.stdout.take().unwrap());
    let added: u64 = out
        .lines()
        .map_while(Result::ok)
        .find_map(|l| {
            l.split_once("READY:")
                .map(|(_, h)| h.trim().parse().unwrap())
        })
        .expect("the child opened, changed, autosaved and locked the drawing");

    // While it runs, the drawing is open in another Qiyas: reported, not a crash.
    let live = recoverable(&path).expect("its autosave is newer than the drawing");
    assert!(!live.crashed);
    assert_eq!(live.locked_elsewhere.as_ref().map(|l| l.pid), Some(pid));
    assert_eq!(read_lock(&path).unwrap().status(), LockStatus::Alive);
    // Opening it here too leaves the other's lock alone (no read-only open: only a warning).
    assert!(!take_lock(&path).unwrap());
    assert_eq!(read_lock(&path).unwrap().pid, pid);

    // SIGKILL: no clean exit, so the lock stays behind with a dead pid.
    running.0.kill().unwrap();
    running.0.wait().unwrap();
    assert!(lock_path(&path).exists());
    assert_eq!(read_lock(&path).unwrap().status(), LockStatus::Dead);

    // Offered even when the drawing is newer than the autosave: Qiyas closed unexpectedly.
    modified_ago(&autosave_path(&path), 3600);
    modified_ago(&path, 0);
    let crashed = recoverable(&path).expect("a crash offers its autosave");
    assert!(crashed.crashed);
    assert_eq!(crashed.locked_elsewhere, None);
    assert!(crashed.drawing_at.expect("the drawing is there") > crashed.saved_at);
    assert_eq!(crashed.autosave, autosave_path(&path).display().to_string());

    // Recover brings back the child's line and its take-off row.
    let r = Session::open_autosave(&path).unwrap();
    assert!(r.drawing.doc.get_entity(Handle::new(added)).is_some());
    assert_eq!(r.takeoff.items.len(), 1);
    assert!(r.is_dirty());

    // Opening the drawing again cleans the stale lock up: this process holds it now, and the
    // older autosave is no longer offered.
    assert!(take_lock(&path).unwrap());
    let mine = read_lock(&path).unwrap();
    assert_eq!(mine.pid, std::process::id());
    assert_eq!(mine.status(), LockStatus::Mine);
    assert!(recoverable(&path).is_none());
    release_lock(&path).unwrap();
    assert!(!lock_path(&path).exists());
}

#[test]
fn a_lock_file_holds_pid_host_and_start_time_and_is_released() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    assert_eq!(read_lock(&path), None);
    assert!(take_lock(&path).unwrap());
    assert_eq!(lock_path(&path), dir.path().join("sample.dwg.qlock"));

    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(lock_path(&path)).unwrap()).unwrap();
    assert_eq!(json["pid"], std::process::id());
    assert!(json["host"].is_string());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(json["started_at"].as_u64().unwrap().abs_diff(now) < 60);
    let lock = read_lock(&path).unwrap();
    assert_eq!(lock.pid, std::process::id());
    assert_eq!(lock.status(), LockStatus::Mine);
    // Written whole through a temp file: nothing else beside the drawing.
    assert_eq!(files(dir.path()), ["sample.dwg", "sample.dwg.qlock"]);

    // Taking it again keeps it; a lock of our own is no crash and offers nothing.
    assert!(take_lock(&path).unwrap());
    assert!(recoverable(&path).is_none());

    release_lock(&path).unwrap();
    assert_eq!(read_lock(&path), None);
    // Gone already: no error.
    release_lock(&path).unwrap();
    assert_eq!(files(dir.path()), ["sample.dwg"]);
}

#[test]
fn a_lock_from_another_computer_is_locked_elsewhere_never_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    modified_ago(&path, 3600);
    let mut s = Session::open(&path).unwrap();
    line(&mut s, 100.0);
    s.autosave().unwrap().unwrap();
    std::fs::write(
        lock_path(&path),
        r#"{"pid": 1, "host": "elsewhere", "started_at": 1760000000}"#,
    )
    .unwrap();
    let other = DrawingLock {
        pid: 1,
        host: "elsewhere".into(),
        started_at: 1_760_000_000,
    };
    assert_eq!(other.status(), LockStatus::OtherHost);

    let found = recoverable(&path).expect("the autosave is newer");
    assert!(!found.crashed);
    assert_eq!(found.locked_elsewhere, Some(other.clone()));
    // Not ours: never taken over, never removed.
    assert!(!take_lock(&path).unwrap());
    release_lock(&path).unwrap();
    assert_eq!(read_lock(&path), Some(other));

    // An older autosave is not offered for another computer's lock.
    modified_ago(&autosave_path(&path), 7200);
    assert!(recoverable(&path).is_none());
}

#[test]
fn a_dead_lock_offers_an_older_autosave_and_discarding_it_cleans_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    let dead = DrawingLock {
        pid: dead_pid(),
        host: this_host(),
        started_at: 1_760_000_000,
    };
    assert_eq!(dead.status(), LockStatus::Dead);
    std::fs::write(lock_path(&path), serde_json::to_vec(&dead).unwrap()).unwrap();
    // A crash with nothing autosaved: nothing to offer.
    assert!(recoverable(&path).is_none());

    let mut s = Session::open(&path).unwrap();
    line(&mut s, 100.0);
    s.autosave().unwrap().unwrap();
    modified_ago(&autosave_path(&path), 3600);
    let found = recoverable(&path).expect("crashed: offered though older");
    assert!(found.crashed);
    assert_eq!(found.locked_elsewhere, None);

    // Discard: the autosave goes, and the dead lock with it.
    discard_autosave(&path).unwrap();
    assert_eq!(read_lock(&path), None);
    assert_eq!(files(dir.path()), ["sample.dwg"]);
}

#[test]
fn an_unreadable_lock_file_counts_as_no_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    let mut s = Session::open(&path).unwrap();
    line(&mut s, 100.0);
    s.autosave().unwrap().unwrap();
    modified_ago(&autosave_path(&path), 3600);
    std::fs::write(lock_path(&path), "{ \"pid\": 12").unwrap();
    assert_eq!(read_lock(&path), None);
    assert!(recoverable(&path).is_none());
    // Taken over.
    assert!(take_lock(&path).unwrap());
    assert_eq!(read_lock(&path).unwrap().pid, std::process::id());
}

#[test]
fn autosave_is_due_after_twenty_document_changes_and_starts_again_on_save_and_autosave() {
    assert_eq!(AUTOSAVE_EVERY, 20);
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    let mut s = Session::open(&path).unwrap();
    for i in 0..19 {
        line(&mut s, f64::from(i));
    }
    assert_eq!(s.changes_since_autosave(), 19);
    assert!(!s.autosave_due());
    // A take-off row is no change to the drawing.
    measure(&mut s);
    assert!(!s.autosave_due());
    line(&mut s, 19.0);
    assert_eq!(s.changes_since_autosave(), 20);
    assert!(s.autosave_due());

    s.autosave().unwrap().expect("an autosave");
    assert_eq!(s.changes_since_autosave(), 0);
    assert!(!s.autosave_due());

    // Undo counts as a change.
    for i in 0..19 {
        line(&mut s, f64::from(i));
    }
    assert!(s.undo());
    assert!(s.autosave_due());
    s.save().unwrap();
    assert_eq!(s.changes_since_autosave(), 0);
    assert!(!s.autosave_due());
}

#[test]
fn autosave_all_writes_every_drawing_with_changes_and_says_which_failed() {
    let dir = tempfile::tempdir().unwrap();
    let clean = Session::open(&write_drawing(&sample_doc(), &dir.path().join("a.dwg"))).unwrap();
    let mut dirty =
        Session::open(&write_drawing(&sample_doc(), &dir.path().join("b.dwg"))).unwrap();
    line(&mut dirty, 100.0);
    let mut broken =
        Session::open(&write_drawing(&sample_doc(), &dir.path().join("c.dwg"))).unwrap();
    for i in 0..20 {
        line(&mut broken, f64::from(i));
    }
    broken.drawing.path = dir.path().join("gone").join("c.dwg");
    assert!(broken.autosave_due());

    let run = autosave_all([&clean, &dirty, &broken]);
    assert_eq!(
        run.written,
        [autosave_path(&dir.path().join("b.dwg"))
            .display()
            .to_string()]
    );
    assert_eq!(run.failed.len(), 1);
    assert!(
        run.failed[0].starts_with(&broken.drawing.path.display().to_string()),
        "{:?}",
        run.failed
    );
    // A failed attempt counts: the 1 s watch does not try it again every second (the timer does).
    assert!(!broken.autosave_due());
    assert!(!autosave_path(&dir.path().join("a.dwg")).exists());
}

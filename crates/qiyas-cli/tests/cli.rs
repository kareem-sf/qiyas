//! `qiyas-cli` run as the user runs it: the built binary, its exit code, stdout and stderr. The
//! drawings are generated here with opencadcodec, as `qiyas-core/examples/write_test_plan.rs`
//! writes the test plan: three blocks, DOOR 3 times, WINDOW 5, COLUMN 6, in metres.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use calamine::{open_workbook, Data, Reader, Xlsx};
use opencadcodec::entities::Insert;
use opencadcodec::{
    BlockRecord, CadDocument, DwgWriter, DxfVersion, EntityType, Layer, Line, Vector3,
};
use qiyas_core::Session;

/// The plan: layers A-DOOR, A-WINDOW, S-COLUMN; blocks DOOR, WINDOW, COLUMN (one line each);
/// 3 doors, 5 windows and 6 columns in model space.
fn plan() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = 6;
    for name in ["A-DOOR", "A-WINDOW", "S-COLUMN"] {
        let mut layer = Layer::new(name);
        layer.handle = doc.allocate_handle();
        doc.layers.add(layer).unwrap();
    }
    for (name, length) in [("DOOR", 0.9), ("WINDOW", 1.2), ("COLUMN", 0.4)] {
        let mut record = BlockRecord::new(name);
        record.handle = doc.allocate_handle();
        record.block_entity_handle = doc.allocate_handle();
        record.block_end_handle = doc.allocate_handle();
        let owner = record.handle;
        doc.block_records.add(record).unwrap();
        let mut line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, length, 0.0, 0.0));
        line.common_mut().owner_handle = owner;
        doc.add_entity(line).unwrap();
    }
    for (name, layer, n) in [
        ("DOOR", "A-DOOR", 3),
        ("WINDOW", "A-WINDOW", 5),
        ("COLUMN", "S-COLUMN", 6),
    ] {
        for i in 0..n {
            let at = Vector3::new(f64::from(i) * 2.0, 0.0, 0.0);
            let mut insert = EntityType::Insert(Insert::new(name, at));
            insert.common_mut().layer = layer.to_string();
            doc.add_entity(insert).unwrap();
        }
    }
    doc
}

/// The plan as DWG bytes, in `version`.
fn plan_bytes(version: DxfVersion) -> Vec<u8> {
    let mut doc = plan();
    doc.version = version;
    DwgWriter::write_to_vec(&doc).unwrap()
}

fn write_plan(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, plan_bytes(DxfVersion::AC1032)).unwrap();
    path
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qiyas-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The run succeeded with nothing on stderr; its stdout.
fn ok(out: Output) -> String {
    assert!(
        out.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        stdout(&out),
        stderr(&out)
    );
    assert_eq!(stderr(&out), "", "nothing on stderr");
    stdout(&out)
}

/// The run failed (exit 1, or 2 for a usage error) with one line on stderr; that line.
fn failed(out: Output) -> String {
    assert!(
        matches!(out.status.code(), Some(1 | 2)),
        "exit {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        stdout(&out),
        stderr(&out)
    );
    let err = stderr(&out);
    assert_eq!(err.lines().count(), 1, "one line on stderr: {err:?}");
    assert!(err.starts_with("qiyas-cli: "), "{err}");
    err.trim_end().to_string()
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

/// Block counts of a drawing, read back through the core.
fn counts(path: &Path) -> Vec<f64> {
    let mut session = Session::open(path).unwrap();
    ["DOOR", "WINDOW", "COLUMN"]
        .into_iter()
        .map(|name| {
            match session
                .run(qiyas_core::Command::CountBlock {
                    block_name: name.to_string(),
                    layer: None,
                })
                .unwrap()
            {
                qiyas_core::Outcome::Measured { measurement } => measurement.shown,
                other => panic!("{other:?}"),
            }
        })
        .collect()
}

/// The take-off rows of a workbook: description and quantity.
fn rows(xlsx: &Path) -> Vec<(String, f64)> {
    let mut book: Xlsx<_> = open_workbook(xlsx).unwrap();
    let sheet = book.worksheet_range("Take-off").unwrap();
    sheet
        .rows()
        .skip(1)
        .take_while(|r| !matches!(r.first(), None | Some(Data::Empty)))
        .map(|r| {
            let quantity = match &r[2] {
                Data::Float(v) => *v,
                Data::Int(v) => *v as f64,
                other => panic!("{other:?}"),
            };
            (r[0].to_string(), quantity)
        })
        .collect()
}

#[test]
fn convert_goes_dwg_to_dxf_and_back_in_a_chosen_version() {
    let dir = tempfile::tempdir().unwrap();
    let dwg = write_plan(dir.path(), "plan.dwg");
    let dxf = dir.path().join("plan.dxf");
    let text = ok(cli(&["convert", s(&dwg), s(&dxf)]));
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("plan.dxf"), "{text}");
    assert_eq!(counts(&dxf), [3.0, 5.0, 6.0]);
    // A Save As: the project file comes beside the new file (`plan.qiyas`, the stem's); the
    // drawing read is not written.
    assert!(dir.path().join("plan.qiyas").exists());
    assert!(!dir.path().join("plan.dwg.bak").exists());

    let old = dir.path().join("old.dwg");
    let out = cli(&["convert", s(&dxf), s(&old), "--version", "2004", "--json"]);
    let v = json(&ok(out));
    assert_eq!(v["version"], "AC1018");
    assert_eq!(v["output"], s(&old));
    let session = Session::open(&old).unwrap();
    assert_eq!(session.drawing.doc.version, DxfVersion::AC1018);
    assert_eq!(counts(&old), [3.0, 5.0, 6.0]);

    assert_eq!(ok(cli(&["--quiet", "convert", s(&old), s(&dxf)])), "");
    // Written over: the old file is kept as .bak, as the app does.
    assert!(dir.path().join("plan.dxf.bak").exists());
}

#[test]
fn convert_errors_are_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let dwg = write_plan(dir.path(), "plan.dwg");
    let missing = dir.path().join("none.dwg");
    let err = failed(cli(&["convert", s(&missing), s(&dir.path().join("x.dxf"))]));
    assert!(err.contains("none.dwg"), "{err}");
    let err = failed(cli(&["convert", s(&dwg), s(&dir.path().join("x.txt"))]));
    assert!(err.contains("x.txt"), "{err}");
    let err = failed(cli(&[
        "convert",
        s(&dwg),
        s(&dir.path().join("x.dwg")),
        "--version",
        "1999",
    ]));
    assert!(err.contains("2018"), "{err}");
    std::fs::write(dir.path().join("junk.dwg"), b"not a drawing").unwrap();
    let err = failed(cli(&[
        "convert",
        s(&dir.path().join("junk.dwg")),
        s(&dir.path().join("y.dxf")),
    ]));
    assert!(err.contains("can't be opened"), "{err}");
    assert!(!dir.path().join("y.dxf").exists());
}

#[test]
fn usage_errors_are_one_line_and_help_is_not_an_error() {
    failed(cli(&[]));
    failed(cli(&["frobnicate"]));
    failed(cli(&["convert", "only-one.dwg"]));
    let err = failed(cli(&[
        "batch", "folder", "--script", "x.scr", "--jobs", "0",
    ]));
    assert!(err.contains("--jobs") && !err.contains("--help"), "{err}");
    let help = ok(cli(&["--help"]));
    for word in ["convert", "run", "takeoff", "batch"] {
        assert!(help.contains(word), "{help}");
    }
    assert!(ok(cli(&["convert", "--help"])).contains("--version"));
}

#[test]
fn run_drives_a_script_on_a_drawing_and_writes_only_its_export() {
    let dir = tempfile::tempdir().unwrap();
    let dwg = write_plan(dir.path(), "plan.dwg");
    let script = dir.path().join("count.scr");
    std::fs::write(
        &script,
        "; count the three blocks\nCOUNT DOOR\nCOUNT WINDOW\nCOUNT COLUMN\nEXPORT {name}.xlsx\nQUIT\nCOUNT DOOR\n",
    )
    .unwrap();
    let text = ok(cli(&["run", s(&script), s(&dwg)]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "line 2: DOOR count = 3 pcs");
    assert_eq!(lines[1], "line 3: WINDOW count = 5 pcs");
    assert_eq!(lines[2], "line 4: COLUMN count = 6 pcs");
    assert!(lines[3].starts_with("line 5: exported "), "{text}");
    assert_eq!(lines[4], "line 6: quit");
    assert_eq!(lines.len(), 5);
    let xlsx = dir.path().join("plan.xlsx");
    assert_eq!(
        rows(&xlsx),
        [
            ("DOOR count".to_string(), 3.0),
            ("WINDOW count".to_string(), 5.0),
            ("COLUMN count".to_string(), 6.0)
        ]
    );
    // No SAVE or SAVEAS: no project file, no .bak, the drawing untouched.
    assert!(!dir.path().join("plan.qiyas").exists());
    assert!(!dir.path().join("plan.dwg.bak").exists());

    let v = json(&ok(cli(&["run", s(&script), s(&dwg), "--json"])));
    assert_eq!(v["counts"]["DOOR"], 3.0);
    assert_eq!(v["counts"]["COLUMN"], 6.0);
    assert_eq!(v["steps"].as_array().unwrap().len(), 5);
    assert_eq!(v["problems"], 0);
    assert_eq!(ok(cli(&["run", s(&script), s(&dwg), "--quiet"])), "");
}

#[test]
fn run_reads_the_project_beside_the_drawing_and_writes_it_only_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let dwg = write_plan(dir.path(), "plan.dwg");
    // A project file beside the drawing with one measurement in its take-off.
    let mut session = Session::open(&dwg).unwrap();
    session
        .run(qiyas_core::Command::MeasureDistance {
            points: vec![[0.0, 0.0], [3.0, 4.0]],
        })
        .unwrap();
    let project = session.save_project().unwrap();
    drop(session);
    let before = std::fs::read(&project).unwrap();

    let script = dir.path().join("export.scr");
    std::fs::write(&script, "COUNT DOOR\nEXPORT t.xlsx\n").unwrap();
    ok(cli(&["run", s(&script), s(&dwg)]));
    let rows = rows(&dir.path().join("t.xlsx"));
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].1, 5.0);
    assert_eq!(std::fs::read(&project).unwrap(), before, "not written");

    // SAVEAS writes the copy and its project file, with the count in it.
    std::fs::write(&script, "COUNT DOOR\nSAVEAS copy.dxf\n").unwrap();
    ok(cli(&["run", s(&script), s(&dwg)]));
    assert!(dir.path().join("copy.dxf").exists());
    assert_eq!(
        Session::open(&dir.path().join("copy.dxf"))
            .unwrap()
            .takeoff
            .items
            .len(),
        2
    );
    assert_eq!(
        std::fs::read(&project).unwrap(),
        before,
        "still not written"
    );
}

#[test]
fn run_errors_name_the_script_and_line_and_nothing_runs() {
    let dir = tempfile::tempdir().unwrap();
    let dwg = write_plan(dir.path(), "plan.dwg");
    let script = dir.path().join("bad.scr");
    std::fs::write(&script, "EXPORT first.xlsx\nFOO bar\n").unwrap();
    let err = failed(cli(&["run", s(&script), s(&dwg)]));
    assert!(
        err.ends_with("bad.scr: line 2: unknown command FOO"),
        "{err}"
    );
    assert!(
        !dir.path().join("first.xlsx").exists(),
        "checked whole first"
    );

    std::fs::write(&script, "EXPORT ../outside.xlsx\n").unwrap();
    let err = failed(cli(&["run", s(&script), s(&dwg)]));
    assert!(err.contains("line 1") && err.contains("outside"), "{err}");

    // A step that fails stops the run on its line.
    std::fs::write(&script, "COUNT DOOR\nMEASURE FFFF\n").unwrap();
    let err = failed(cli(&["run", s(&script), s(&dwg), "--json"]));
    assert!(err.ends_with("line 2: No entity with handle FFFF"), "{err}");

    // The script must OPEN a drawing when none is given.
    std::fs::write(&script, "COUNT DOOR\n").unwrap();
    let err = failed(cli(&["run", s(&script)]));
    assert!(err.contains("OPEN"), "{err}");
    let err = failed(cli(&["run", s(&dir.path().join("none.scr"))]));
    assert!(err.contains("none.scr"), "{err}");
}

#[test]
fn takeoff_counts_blocks_and_exports_without_writing_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let dwg = write_plan(dir.path(), "plan.dwg");
    let xlsx = dir.path().join("out.xlsx");
    let text = ok(cli(&[
        "takeoff",
        s(&dwg),
        "--count",
        "DOOR",
        "--count",
        "WINDOW",
        "--count",
        "COLUMN",
        "--export",
        s(&xlsx),
    ]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[..3],
        [
            "DOOR count = 3 pcs",
            "WINDOW count = 5 pcs",
            "COLUMN count = 6 pcs"
        ]
    );
    assert!(lines[3].starts_with("exported "), "{text}");
    assert_eq!(rows(&xlsx).len(), 3);
    assert!(!dir.path().join("plan.qiyas").exists());

    let v = json(&ok(cli(&[
        "takeoff",
        s(&dwg),
        "--count",
        "WINDOW",
        "--json",
    ])));
    assert_eq!(v["counts"]["WINDOW"], 5.0);
    assert_eq!(v["export"], serde_json::Value::Null);
    // Nothing to do is a usage error.
    failed(cli(&["takeoff", s(&dwg)]));
}

/// The script of Review Focus 3: count three blocks, export the take-off, quit.
const COUNT_SCRIPT: &str = "COUNT DOOR\nCOUNT WINDOW\nCOUNT COLUMN\nEXPORT {name}.xlsx\nQUIT\n";

#[test]
fn batch_of_fifty_drawings_skips_the_corrupt_one_and_fails_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let plans = dir.path().join("plans");
    let out = dir.path().join("out");
    std::fs::create_dir_all(&plans).unwrap();
    let bytes = plan_bytes(DxfVersion::AC1032);
    for i in 1..=50 {
        let name = format!("plan-{i:02}.dwg");
        if i == 17 {
            // Cut off in the middle: the header is there, the rest of the file is not, and the
            // failsafe reader finds nothing usable, so it does not open at all.
            std::fs::write(plans.join(&name), &bytes[..bytes.len() / 2]).unwrap();
        } else {
            std::fs::write(plans.join(&name), &bytes).unwrap();
        }
    }
    // Not drawings: left out of the batch.
    std::fs::write(plans.join("notes.txt"), "x").unwrap();
    std::fs::write(plans.join("plan-01.dwg.bak"), "x").unwrap();
    let script = dir.path().join("count.scr");
    std::fs::write(&script, COUNT_SCRIPT).unwrap();

    let run = cli(&[
        "batch",
        s(&plans),
        "--script",
        s(&script),
        "--out",
        s(&out),
        "--jobs",
        "2",
    ]);
    let text = stdout(&run);
    let err = failed(run);
    assert_eq!(err, "qiyas-cli: 1 of 50 drawings failed");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        51,
        "a line per drawing and the summary: {text}"
    );
    for (i, line) in lines[..50].iter().enumerate() {
        let name = format!("plan-{:02}.dwg", i + 1);
        assert!(
            line.starts_with(s(&plans.join(&name))),
            "folder order: {line}"
        );
        if i + 1 == 17 {
            assert!(
                line.contains(" error ") && line.contains("can't be opened"),
                "{line}"
            );
        } else {
            // Counts by block name, sorted.
            assert!(line.ends_with(" ok COLUMN=6 DOOR=3 WINDOW=5"), "{line}");
        }
    }
    assert_eq!(lines[50], "50 drawings: 49 ok, 0 with warnings, 1 failed");
    let workbooks = std::fs::read_dir(&out).unwrap().count();
    assert_eq!(workbooks, 49, "every good drawing has its Excel");
    assert!(!out.join("plan-17.xlsx").exists());
    assert_eq!(rows(&out.join("plan-50.xlsx")).len(), 3);
    // Nothing written beside the drawings.
    assert_eq!(std::fs::read_dir(&plans).unwrap().count(), 52);

    let run = cli(&[
        "batch",
        s(&plans),
        "--script",
        s(&script),
        "--out",
        s(&out),
        "--json",
    ]);
    let v = json(&stdout(&run));
    failed(run);
    assert_eq!(
        (v["total"].as_u64(), v["ok"].as_u64(), v["failed"].as_u64()),
        (Some(50), Some(49), Some(1))
    );
    let files = v["files"].as_array().unwrap();
    assert_eq!(files[16]["status"], "error");
    assert!(files[16]["error"]
        .as_str()
        .unwrap()
        .contains("can't be opened"));
    assert_eq!(files[0]["status"], "ok");
    assert_eq!(files[0]["counts"]["WINDOW"], 5.0);
    // --quiet: nothing on stdout, the failure still on stderr.
    let run = cli(&[
        "batch",
        s(&plans),
        "--script",
        s(&script),
        "--out",
        s(&out),
        "--quiet",
    ]);
    assert_eq!(stdout(&run), "");
    failed(run);
}

#[test]
fn batch_reports_damaged_drawings_without_stopping() {
    // One drawing damaged at 64 places in turn (128 bytes of junk each), plus one whose header is
    // junk. The failsafe reader opens some damaged copies cleanly, some with problems (a warning:
    // the script still runs), refuses others, and the pinned codec panics on some: each one is a
    // line of the report and the batch goes on.
    let dir = tempfile::tempdir().unwrap();
    let plans = dir.path().join("plans");
    std::fs::create_dir_all(&plans).unwrap();
    let bytes = plan_bytes(DxfVersion::AC1018);
    let step = bytes.len() / 64;
    for k in 0..64 {
        let mut damaged = bytes.clone();
        let at = k * step;
        for (i, b) in damaged[at..at + 128].iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37) ^ 0xA5;
        }
        std::fs::write(plans.join(format!("d{k:02}.dwg")), damaged).unwrap();
    }
    let mut junk = bytes.clone();
    junk[..6].copy_from_slice(b"JUNK!!");
    std::fs::write(plans.join("junk.dwg"), junk).unwrap();
    let script = dir.path().join("count.scr");
    std::fs::write(&script, COUNT_SCRIPT).unwrap();
    let out = dir.path().join("out");

    let run = cli(&[
        "batch",
        s(&plans),
        "--script",
        s(&script),
        "--out",
        s(&out),
        "--jobs",
        "2",
        "--json",
    ]);
    let v = json(&stdout(&run));
    let err = failed(run);
    let files = v["files"].as_array().unwrap();
    assert_eq!(files.len(), 65);
    let status = |f: &serde_json::Value| f["status"].as_str().unwrap().to_string();
    let junk = files
        .iter()
        .find(|f| f["path"].as_str().unwrap().ends_with("junk.dwg"))
        .unwrap();
    assert_eq!(status(junk), "error");
    let warned: Vec<&serde_json::Value> = files.iter().filter(|f| status(f) == "warning").collect();
    assert!(!warned.is_empty(), "some copies open with problems");
    for f in &warned {
        assert!(f["problems"].as_u64().unwrap() > 0);
        let name = Path::new(f["path"].as_str().unwrap()).file_stem().unwrap();
        assert!(
            out.join(name).with_extension("xlsx").exists(),
            "the script ran on {f}"
        );
    }
    let crashed = files
        .iter()
        .filter(|f| {
            f["error"]
                .as_str()
                .is_some_and(|e| e.starts_with("crashed: "))
        })
        .count();
    // If the codec stops panicking on these, this test no longer shows that a crash is caught.
    assert!(
        crashed > 0,
        "the pinned reader panics on some damaged copies"
    );
    let failed_n = files.iter().filter(|f| status(f) == "error").count();
    assert_eq!(v["failed"].as_u64(), Some(failed_n as u64));
    assert_eq!(err, format!("qiyas-cli: {failed_n} of 65 drawings failed"));
    let workbooks = std::fs::read_dir(&out).unwrap().count();
    assert_eq!(workbooks, 65 - failed_n);
}

#[test]
fn batch_runs_a_sheet_sets_drawings_and_refuses_a_bad_script_up_front() {
    let dir = tempfile::tempdir().unwrap();
    write_plan(dir.path(), "a.dwg");
    write_plan(dir.path(), "b.dwg");
    write_plan(dir.path(), "c.dwg");
    let qss = dir.path().join("set.qss");
    std::fs::write(
        &qss,
        r#"{"name":"Set","drawings":[{"path":"b.dwg"},{"path":"a.dwg"},{"path":"gone.dwg"}]}"#,
    )
    .unwrap();
    let script = dir.path().join("count.scr");
    std::fs::write(&script, COUNT_SCRIPT).unwrap();

    let run = cli(&["batch", s(&qss), "--script", s(&script)]);
    let text = stdout(&run);
    assert_eq!(failed(run), "qiyas-cli: 1 of 3 drawings failed");
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains("b.dwg ok "), "the set's order: {text}");
    assert!(lines[1].contains("a.dwg ok "), "{text}");
    assert!(lines[2].contains("gone.dwg error "), "{text}");
    // Without --out the workbooks go to the script's folder.
    assert!(dir.path().join("a.xlsx").exists() && dir.path().join("b.xlsx").exists());
    assert!(!dir.path().join("c.xlsx").exists());

    // A bad script is refused before any drawing is touched.
    std::fs::write(&script, "COUNT DOOR\nEXPORT {name}.xlsx\nBOGUS\n").unwrap();
    let out = dir.path().join("out");
    let run = cli(&[
        "batch",
        s(dir.path()),
        "--script",
        s(&script),
        "--out",
        s(&out),
    ]);
    assert_eq!(stdout(&run), "");
    let err = failed(run);
    assert!(
        err.ends_with("count.scr: line 3: unknown command BOGUS"),
        "{err}"
    );
    assert!(!out.exists());
    // A folder with no drawings is an error too.
    let empty = dir.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let err = failed(cli(&["batch", s(&empty), "--script", s(&script)]));
    assert!(err.contains("no drawings"), "{err}");
}

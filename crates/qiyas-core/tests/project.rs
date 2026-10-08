use std::path::{Path, PathBuf};

use qiyas_core::{project_path, Command, Project, Session};

/// Writes the sample drawing (DXF, metres) into a fresh temp dir. The dir must outlive the test body.
fn sample() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    (dir, path)
}

fn distance() -> Command {
    Command::MeasureDistance {
        points: vec![[0.0, 0.0], [3.0, 0.0]],
    }
}

#[test]
fn project_round_trip() {
    let (_dir, drawing) = sample();
    let mut session = Session::open(&drawing).unwrap();
    session.run(distance()).unwrap();
    session
        .run(Command::MeasureArea {
            points: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]],
        })
        .unwrap();
    let ids: Vec<_> = session.takeoff.items.iter().map(|m| m.id.clone()).collect();

    let saved = session.save_project().unwrap();
    assert_eq!(saved, project_path(&drawing));
    assert!(!saved.with_extension("qiyas.tmp").exists());

    let reopened = Session::open(&drawing).unwrap();
    let reloaded: Vec<_> = reopened
        .takeoff
        .items
        .iter()
        .map(|m| m.id.clone())
        .collect();
    assert_eq!(reloaded, ids);
    assert!(
        reopened.drawing.problems.is_empty(),
        "{:?}",
        reopened.drawing.problems
    );
    // Loading is not an edit: there is nothing to undo.
    assert!(!reopened.can_undo());
}

#[test]
fn project_path_rule() {
    assert_eq!(
        project_path(Path::new("/x/villa.DWG")),
        PathBuf::from("/x/villa.qiyas")
    );
}

#[test]
fn project_file_is_pretty_json_with_the_drawing_name() {
    let (_dir, drawing) = sample();
    let mut session = Session::open(&drawing).unwrap();
    session.run(distance()).unwrap();
    let text = std::fs::read_to_string(session.save_project().unwrap()).unwrap();
    assert!(text.contains("\n  \"version\": 1"), "{text}");
    let project: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(project.version, 1);
    assert_eq!(project.drawing, "sample.dxf");
    assert_eq!(project.takeoff.items.len(), 1);
}

#[test]
fn corrupt_project_is_backed_up_and_opens_with_a_problem_and_an_empty_takeoff() {
    let (_dir, drawing) = sample();
    let corrupt = b"{ not json";
    std::fs::write(project_path(&drawing), corrupt).unwrap();
    let session = Session::open(&drawing).unwrap();
    assert!(session.takeoff.items.is_empty());
    let backup = drawing.with_file_name("sample.qiyas.bak");
    assert_eq!(std::fs::read(&backup).unwrap(), corrupt);
    assert!(
        session
            .drawing
            .problems
            .iter()
            .any(|p| p
                .starts_with("Project file could not be read (backed up to sample.qiyas.bak): ")),
        "{:?}",
        session.drawing.problems
    );
}

#[test]
fn project_for_another_drawing_still_loads_with_a_problem() {
    let (_dir, drawing) = sample();
    let mut session = Session::open(&drawing).unwrap();
    session.run(distance()).unwrap();
    let path = session.save_project().unwrap();
    let mut project: Project =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    project.drawing = "other.dwg".to_string();
    std::fs::write(&path, serde_json::to_string(&project).unwrap()).unwrap();

    let session = Session::open(&drawing).unwrap();
    assert_eq!(session.takeoff.items.len(), 1);
    assert!(
        session
            .drawing
            .problems
            .contains(&"Project file was saved for other.dwg".to_string()),
        "{:?}",
        session.drawing.problems
    );
}

#[test]
fn drawing_name_matches_ignoring_case() {
    let (_dir, drawing) = sample();
    let session = Session::open(&drawing).unwrap();
    let path = session.save_project().unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("sample.dxf", "SAMPLE.DXF")).unwrap();

    let session = Session::open(&drawing).unwrap();
    assert!(
        session.drawing.problems.is_empty(),
        "{:?}",
        session.drawing.problems
    );
}

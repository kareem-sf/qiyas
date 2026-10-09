//! External references: a host drawing showing `plan-arch.dxf` (the sample drawing) at (100, 0)
//! turned a quarter turn, so a point (x, y) of the xref lands at (100 - y, x).

use std::f64::consts::PI;
use std::path::Path;

use opencadcodec::{CadDocument, EntityType, Layer, Line};
use qiyas_core::fixtures::{add_xref, sample_doc, write_drawing, write_xref_pair};
use qiyas_core::{
    embed_measurements, project_path, Command, Drawing, EntityFilter, Kind, Measurement, Outcome,
    Session, SnapModes,
};

/// The host session; the temp dir must outlive the test body.
fn host() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    let session = Session::open(&path).unwrap();
    (dir, session)
}

fn measure(session: &mut Session, cmd: Command) -> Measurement {
    match session.run(cmd).unwrap() {
        Outcome::Measured { measurement } => *measurement,
        other => panic!("expected a measurement, got {other:?}"),
    }
}

fn segments(session: &Session) -> usize {
    session.batch().segments.len() / 4
}

/// Segments of the sample drawing opened on its own, with `hidden` layers hidden.
fn sample_segments(dir: &Path, hidden: &[&str]) -> usize {
    let path = write_drawing(&sample_doc(), &dir.join("alone.dxf"));
    let mut alone = Session::open(&path).unwrap();
    for name in hidden {
        alone.set_layer_hidden(name, true);
    }
    segments(&alone)
}

/// The host's reference to the xref.
fn xref_insert(session: &Session) -> u64 {
    session
        .drawing
        .entities_in("*Model_Space")
        .find_map(|e| match e {
            EntityType::Insert(i) if i.block_name == "plan-arch" => Some(i.common.handle.value()),
            _ => None,
        })
        .unwrap()
}

/// Handle of the first entity of `kind` on layer A-WALL at the top of the xref drawing.
fn xref_wall(session: &Session, kind: &str) -> u64 {
    let xref = session.xrefs.get("plan-arch").unwrap();
    let doc = &xref.drawing.as_ref().unwrap().doc;
    doc.entities_in_block("*Model_Space")
        .find(|e| e.common().layer == "A-WALL" && format!("{e:?}").starts_with(&format!("{kind}(")))
        .unwrap()
        .common()
        .handle
        .value()
}

#[test]
fn the_xref_is_found_drawn_and_its_layers_listed() {
    let (dir, session) = host();
    let info = session.info();
    assert_eq!(info.xrefs.len(), 1);
    let x = &info.xrefs[0];
    assert_eq!(x.name, "plan-arch");
    assert!(x.found && !x.overlay && !x.unloaded);
    assert!(x.path.ends_with("plan-arch.dxf"), "{}", x.path);
    assert_eq!(x.layer_count, 2);
    let names: Vec<&str> = info.layers.iter().map(|l| l.name.as_str()).collect();
    assert!(names.contains(&"plan-arch|A-WALL"), "{names:?}");
    assert!(names.contains(&"plan-arch|A-DOOR"), "{names:?}");
    assert!(!names.contains(&"plan-arch|0"), "{names:?}");
    let wall = info
        .layers
        .iter()
        .find(|l| l.name == "plan-arch|A-WALL")
        .unwrap();
    assert_eq!(wall.color, [255, 0, 0], "the xref's own colour");
    assert!(info.problems.is_empty(), "{:?}", info.problems);

    // The host's own line, then the whole xref.
    assert_eq!(segments(&session), 1 + sample_segments(dir.path(), &[]));
    // The xref's arc (centre (20, 0), radius 2, upper half) reaches y 22 on the host.
    let [[x0, y0], [x1, y1]] = session.batch().bounds;
    assert!(x0.abs() < 1e-6 && x1 >= 102.0, "x {x0}..{x1}");
    assert!(
        (y0 + 10.0).abs() < 1e-6 && (y1 - 22.0).abs() < 1e-3,
        "y {y0}..{y1}"
    );
}

#[test]
fn opening_with_xrefs_gives_the_set() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    let (drawing, xrefs) = Drawing::open_with_xrefs(&path, 4).unwrap();
    assert_eq!(drawing.path, path);
    assert_eq!(xrefs.entries.len(), 1);
    let x = &xrefs.entries[0];
    assert_eq!(x.path_in_file, "plan-arch.dxf");
    assert_eq!(
        x.found.as_deref(),
        Some(dir.path().join("plan-arch.dxf").as_path())
    );
    assert_eq!(x.path_tried[0], dir.path().join("plan-arch.dxf"));
    assert!(x.drawing.is_some() && x.depth == 1);
    let (_, none) = Drawing::open_with_xrefs(&path, 0).unwrap();
    assert!(none.entries.is_empty());
}

#[test]
fn a_pick_inside_the_xref_names_it_and_the_entity_measures_in_world() {
    let (_dir, mut session) = host();
    let insert = xref_insert(&session);
    let line = xref_wall(&session, "Line");
    // The xref's line (0,0)-(10,0) lands on (100,0)-(100,10).
    let picked = session.pick([100.0, 5.0], 0.1).unwrap();
    assert_eq!(picked.xref.as_deref(), Some("plan-arch"));
    assert_eq!(picked.entity, line);
    assert_eq!(picked.chain, vec![insert]);
    assert_eq!(picked.top, insert);

    let info = session.entity_info(line, &[insert]).unwrap();
    assert_eq!(info.xref.as_deref(), Some("plan-arch"));
    assert_eq!(info.layer, "plan-arch|A-WALL");
    assert!((info.length.unwrap() - 10.0).abs() < 1e-9);

    let m = measure(
        &mut session,
        Command::MeasureEntity {
            handle: line,
            chain: vec![insert],
        },
    );
    assert_eq!(m.kind, Kind::Length);
    assert!((m.value - 10.0).abs() < 1e-9);
    assert_eq!(m.layer, "plan-arch|A-WALL");
    assert_eq!(m.xref.as_deref(), Some("plan-arch"));
    assert_eq!(m.handles, vec![line]);
    assert_eq!(m.chain, vec![insert]);

    // The rectangle 4 x 3 measures its area, and summed handles work too.
    let rect = xref_wall(&session, "LwPolyline");
    let area = measure(
        &mut session,
        Command::MeasureEntity {
            handle: rect,
            chain: vec![insert],
        },
    );
    assert!((area.value - 12.0).abs() < 1e-9);
    let sum = measure(
        &mut session,
        Command::AddMeasurement {
            kind: Kind::Length,
            value: None,
            unit: None,
            height: None,
            points: Vec::new(),
            handles: vec![line, rect],
            chain: vec![insert],
            description: "Walls".to_string(),
            note: String::new(),
        },
    );
    assert!((sum.value - 24.0).abs() < 1e-9);
    assert_eq!(sum.xref.as_deref(), Some("plan-arch"));

    // Snaps reach the xref's curves: the line's far end.
    let modes = SnapModes::default();
    let snap = session.snap([100.02, 10.03], 0.2, &modes, None).unwrap();
    assert!((snap.point[0] - 100.0).abs() < 1e-9 && (snap.point[1] - 10.0).abs() < 1e-9);
    assert_eq!(snap.handle, line);
    assert_eq!(snap.chain, vec![insert]);
}

#[test]
fn an_xref_handle_must_end_the_chain() {
    let (_dir, session) = host();
    let insert = xref_insert(&session);
    let line = xref_wall(&session, "Line");
    assert!(session.entity_info(line, &[insert, insert]).is_err());
    // Without the chain the handle is looked up in the host, where it is something else or nothing.
    let host_info = session.entity_info(line, &[]);
    assert!(host_info.map_or(true, |i| i.xref.is_none()));
}

#[test]
fn hiding_an_xref_layer_hides_only_its_paths() {
    let (dir, mut session) = host();
    let insert = xref_insert(&session);
    let line = xref_wall(&session, "Line");
    let all = segments(&session);
    let walls = sample_segments(dir.path(), &[]) - sample_segments(dir.path(), &["A-WALL"]);
    assert!(walls > 5);

    session.set_layer_hidden("plan-arch|A-WALL", true);
    assert_eq!(
        session.hidden_layers(),
        vec!["plan-arch|A-WALL".to_string()]
    );
    let batch = session.batch();
    assert_eq!(batch.segments.len() / 4, all - walls);
    assert!(!batch
        .ranges
        .iter()
        .any(|r| r.entity == line && batch.chain_of(r) == [insert]));
    assert!(session.pick([100.0, 5.0], 0.1).is_none());

    // Any case hides it; the host's own A-WALL stays.
    session.set_layer_hidden("plan-arch|A-WALL", false);
    session.set_layer_hidden("PLAN-ARCH|a-wall", true);
    assert_eq!(segments(&session), all - walls);
    assert!(session.pick([2.5, -10.0], 0.1).is_some());

    // Hiding the reference's own layer hides the whole xref.
    session.set_layer_hidden("PLAN-ARCH|a-wall", false);
    session.set_layer_hidden("X-REF", true);
    assert_eq!(segments(&session), 1);
}

#[test]
fn counts_finds_and_text_include_the_xref() {
    let (_dir, mut session) = host();
    let insert = xref_insert(&session);

    let count = measure(
        &mut session,
        Command::CountBlock {
            block_name: "door".to_string(),
            layer: None,
        },
    );
    assert_eq!(count.value, 3.0);
    assert!(count.handles.is_empty());
    assert_eq!(count.points.len(), 3);
    // The plain door at (1, 1) lands at (99, 1).
    assert!(count
        .points
        .iter()
        .any(|p| (p[0] - 99.0).abs() < 1e-9 && (p[1] - 1.0).abs() < 1e-9));
    let on_layer = measure(
        &mut session,
        Command::CountBlock {
            block_name: "DOOR".to_string(),
            layer: Some("plan-arch|A-DOOR".to_string()),
        },
    );
    assert_eq!(on_layer.value, 3.0);

    let summary = session.block_summary(None);
    let doors = summary.iter().find(|b| b.block_name == "DOOR").unwrap();
    assert_eq!((doors.layer.as_str(), doors.count), ("plan-arch|A-DOOR", 3));
    session.set_layer_hidden("plan-arch|A-DOOR", true);
    assert!(!session
        .block_summary(None)
        .iter()
        .any(|b| b.block_name == "DOOR"));
    assert!(session
        .block_summary_in_file(None)
        .iter()
        .any(|b| b.block_name == "DOOR" && b.count == 3));

    let walls = session.find_entities(&EntityFilter {
        layer: Some("PLAN-ARCH|A-WALL".to_string()),
        ..EntityFilter::default()
    });
    assert_eq!(walls.total, 3);
    for row in &walls.rows {
        assert_eq!(row.xref.as_deref(), Some("plan-arch"));
        assert_eq!(row.chain, vec![insert]);
    }
    let line = walls.rows.iter().find(|r| r.kind == "Line").unwrap();
    assert!((line.length.unwrap() - 10.0).abs() < 1e-9);
    let [[x0, y0], [x1, y1]] = line.bounds;
    assert!((x0 - 100.0).abs() < 1e-9 && (x1 - 100.0).abs() < 1e-9);
    assert!(y0.abs() < 1e-9 && (y1 - 10.0).abs() < 1e-9);

    // The title text sits on layer "0" in the xref: it takes the reference's layer.
    let on_ref_layer = session.find_entities(&EntityFilter {
        layer: Some("X-REF".to_string()),
        ..EntityFilter::default()
    });
    let kinds: Vec<&str> = on_ref_layer.rows.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(kinds, vec!["Insert", "Text"]);
    assert!(on_ref_layer.rows[0].xref.is_none());
    let within = session.find_entities(&EntityFilter {
        within: Some([[98.5, 0.5], [99.5, 1.5]]),
        ..EntityFilter::default()
    });
    assert!(within
        .rows
        .iter()
        .any(|r| r.block_name.as_deref() == Some("DOOR") && r.xref.is_some()));

    let texts = session.read_text(None, None);
    let title = texts
        .iter()
        .find(|t| t.text == "GROUND FLOOR PLAN")
        .unwrap();
    assert_eq!(title.xref.as_deref(), Some("plan-arch"));
    assert!((title.at[0] - 102.0).abs() < 1e-9 && title.at[1].abs() < 1e-9);
}

#[test]
fn a_missing_xref_is_listed_with_the_paths_tried_and_the_host_still_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    std::fs::remove_file(dir.path().join("plan-arch.dxf")).unwrap();
    let session = Session::open(&path).unwrap();
    let info = session.info();
    assert!(!info.xrefs[0].found);
    assert_eq!(info.xrefs[0].path, "plan-arch.dxf");
    assert_eq!(info.xrefs[0].problems.len(), 1);
    let x = session.xrefs.get("PLAN-ARCH").unwrap();
    assert!(x.found.is_none() && x.drawing.is_none());
    assert!(x.path_tried.contains(&dir.path().join("plan-arch.dxf")));
    assert!(x
        .path_tried
        .contains(&dir.path().join("xrefs").join("plan-arch.dxf")));
    let problem = info
        .problems
        .iter()
        .find(|p| p.starts_with("Xref plan-arch not found; tried: "))
        .unwrap();
    assert!(problem.contains(&dir.path().join("plan-arch.dxf").display().to_string()));
    assert!(!info.layers.iter().any(|l| l.name.contains('|')));
    // Only the host's own line is drawn.
    assert_eq!(segments(&session), 1);
}

#[test]
fn an_xref_in_the_xrefs_folder_is_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    std::fs::create_dir(dir.path().join("xrefs")).unwrap();
    std::fs::rename(
        dir.path().join("plan-arch.dxf"),
        dir.path().join("xrefs").join("plan-arch.dxf"),
    )
    .unwrap();
    let session = Session::open(&path).unwrap();
    assert!(session.info().xrefs[0].found);
}

#[test]
fn a_chosen_file_loads_the_xref_and_is_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    std::fs::create_dir(dir.path().join("elsewhere")).unwrap();
    let moved = dir.path().join("elsewhere").join("arch-v2.dxf");
    std::fs::rename(dir.path().join("plan-arch.dxf"), &moved).unwrap();

    let mut session = Session::open(&path).unwrap();
    assert!(!session.info().xrefs[0].found);
    let before = session.batch_version();
    let lone = segments(&session);
    assert!(session.set_xref_path("nope", Some(&moved)).is_err());
    session.set_xref_path("Plan-Arch", Some(&moved)).unwrap();
    assert!(session.info().xrefs[0].found);
    assert_eq!(session.xrefs.entries[0].path_tried[0], moved);
    assert_ne!(session.batch_version(), before);
    assert!(segments(&session) > lone);
    assert!(session.pick([100.0, 5.0], 0.1).is_some());

    // Saved in the project file, used again on the next open.
    session.save_project().unwrap();
    let text = std::fs::read_to_string(project_path(&path)).unwrap();
    assert!(
        text.contains("xref_overrides") && text.contains("arch-v2.dxf"),
        "{text}"
    );
    let again = Session::open(&path).unwrap();
    assert!(again.info().xrefs[0].found);

    // Forgetting the choice goes back to looking for it.
    session.set_xref_path("plan-arch", None).unwrap();
    assert!(!session.info().xrefs[0].found);
    assert!(session.xref_overrides.is_empty());
}

#[test]
fn reloading_picks_up_a_changed_xref() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    let mut session = Session::open(&path).unwrap();
    let before = (segments(&session), session.batch_version());

    // The xref gains a line (written a moment later, so its time differs).
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut changed = sample_doc();
    let mut extra = EntityType::Line(Line::from_coords(0.0, 30.0, 0.0, 1.0, 30.0, 0.0));
    extra.common_mut().layer = "A-WALL".to_string();
    changed.add_entity(extra).unwrap();
    write_drawing(&changed, &dir.path().join("plan-arch.dxf"));
    assert_eq!(segments(&session), before.0, "not until reloaded");

    session.reload_xrefs();
    assert_eq!(segments(&session), before.0 + 1);
    assert_ne!(session.batch_version(), before.1);
}

#[test]
fn a_nested_xref_is_drawn_and_picked_as_its_reference() {
    let dir = tempfile::tempdir().unwrap();
    // grid.dxf: one line (0,0)-(1,0) on G-GRID. plan-arch.dxf: the sample plus grid at (50, 0).
    let mut grid = CadDocument::new();
    grid.header.insertion_units = 6;
    let mut layer = Layer::new("G-GRID");
    layer.handle = grid.allocate_handle();
    grid.layers.add(layer).unwrap();
    let mut line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    line.common_mut().layer = "G-GRID".to_string();
    grid.add_entity(line).unwrap();
    write_drawing(&grid, &dir.path().join("grid.dxf"));
    let path = write_xref_pair(dir.path());
    let mut arch = sample_doc();
    let nested = add_xref(&mut arch, "grid", "grid.dxf", [50.0, 0.0], 0.0);
    write_drawing(&arch, &dir.path().join("plan-arch.dxf"));

    let mut session = Session::open(&path).unwrap();
    let info = session.info();
    let names: Vec<&str> = info.xrefs.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, vec!["plan-arch", "grid"]);
    assert!(info.xrefs.iter().all(|x| x.found));
    assert_eq!(session.xrefs.entries[1].depth, 2);
    assert!(info.layers.iter().any(|l| l.name == "grid|G-GRID"));

    // The grid line (50,0)-(51,0) in plan-arch lands on (100,50)-(100,51).
    let picked = session.pick([100.0, 50.5], 0.1).unwrap();
    assert_eq!(picked.xref.as_deref(), Some("plan-arch"));
    assert_eq!(picked.entity, nested);
    assert_eq!(picked.chain, vec![xref_insert(&session)]);
    let m = measure(
        &mut session,
        Command::MeasureEntity {
            handle: picked.entity,
            chain: picked.chain.clone(),
        },
    );
    assert_eq!(m.kind, Kind::Count);
    assert_eq!(m.block_name.as_deref(), Some("grid"));

    // Hiding the nested xref's layer hides its line.
    session.set_layer_hidden("grid|G-GRID", true);
    assert!(session.pick([100.0, 50.5], 0.1).is_none());

    // Depth 1 opens only plan-arch.
    let (_, shallow) = Drawing::open_with_xrefs(&path, 1).unwrap();
    assert_eq!(shallow.entries.len(), 1);
}

#[test]
fn xrefs_referencing_each_other_stop() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    // plan-arch references back to the host, and to itself.
    let mut arch = sample_doc();
    add_xref(&mut arch, "host", "host.dxf", [0.0, 0.0], 0.0);
    add_xref(&mut arch, "plan-arch", "plan-arch.dxf", [5.0, 0.0], PI);
    write_drawing(&arch, &dir.path().join("plan-arch.dxf"));
    let session = Session::open(&path).unwrap();
    let info = session.info();
    assert_eq!(info.xrefs.len(), 2, "{:?}", info.xrefs);
    assert!(info
        .problems
        .iter()
        .any(|p| p.contains("Xref host is the drawing itself")));
    assert_eq!(segments(&session), 1 + sample_segments(dir.path(), &[]));
}

#[test]
fn embedding_never_writes_from_xref_handles_and_save_leaves_the_xref_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    let xref_file = dir.path().join("plan-arch.dxf");
    let xref_bytes = std::fs::read(&xref_file).unwrap();
    let mut session = Session::open(&path).unwrap();
    let insert = xref_insert(&session);
    let line = xref_wall(&session, "Line");
    let wall = measure(
        &mut session,
        Command::MeasureEntity {
            handle: line,
            chain: vec![insert],
        },
    );
    let doors = measure(
        &mut session,
        Command::CountBlock {
            block_name: "DOOR".to_string(),
            layer: None,
        },
    );
    let drawn = measure(
        &mut session,
        Command::MeasureDistance {
            points: vec![[0.0, 0.0], [3.0, 4.0]],
        },
    );

    let mut doc = session.drawing.doc.clone();
    embed_measurements(&mut doc, &session.takeoff.items);
    let written = |id: &str| {
        doc.entities()
            .filter(|e| {
                e.common()
                    .extended_data
                    .get_record("QIYAS")
                    .is_some_and(|r| format!("{r:?}").contains(id))
            })
            .count()
    };
    assert_eq!(written(&wall.id), 0, "no polyline from the xref's handle");
    assert_eq!(written(&doors.id), 3, "the xref's doors as points");
    assert_eq!(written(&drawn.id), 1);

    session.save().unwrap();
    assert_eq!(std::fs::read(&xref_file).unwrap(), xref_bytes);
    assert!(!dir.path().join("plan-arch.dxf.bak").exists());
}

#[test]
fn a_layer_frozen_in_the_xref_stays_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xref_pair(dir.path());
    let mut arch = sample_doc();
    arch.layers.get_mut("A-DOOR").unwrap().flags.frozen = true;
    write_drawing(&arch, &dir.path().join("plan-arch.dxf"));
    let mut session = Session::open(&path).unwrap();
    let info = session.info();
    let doors = info
        .layers
        .iter()
        .find(|l| l.name == "plan-arch|A-DOOR")
        .unwrap();
    assert!(doors.frozen);
    assert_eq!(
        segments(&session),
        1 + sample_segments(dir.path(), &[]) - 3,
        "the three door leaves are not drawn"
    );
    let count = measure(
        &mut session,
        Command::CountBlock {
            block_name: "DOOR".to_string(),
            layer: None,
        },
    );
    assert_eq!(count.value, 0.0);
}

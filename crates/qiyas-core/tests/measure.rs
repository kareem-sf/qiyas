use opencadcodec::{DxfWriter, EntityType};
use qiyas_core::{Command, CoreError, Kind, Measurement, Outcome, Session};

/// Opens the sample drawing (DXF, metres) in a session. The temp dir must outlive the test body.
fn sample_session() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let session = Session::open(&path).unwrap();
    (dir, session)
}

/// Runs a command that must add a measurement, and returns it.
fn measure(session: &mut Session, cmd: Command) -> Measurement {
    match session.run(cmd).unwrap() {
        Outcome::Measured { measurement } => *measurement,
        other => panic!("expected a measurement, got {other:?}"),
    }
}

fn area(points: &[[f64; 2]]) -> Command {
    Command::MeasureArea {
        points: points.to_vec(),
    }
}

const RECT_4X3: [[f64; 2]; 4] = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];

/// Handle of the first entity in model space that `is` accepts.
fn handle_of(session: &Session, is: impl Fn(&EntityType) -> bool) -> u64 {
    session
        .drawing
        .entities_in("*Model_Space")
        .find(|e| is(e))
        .expect("entity in the sample")
        .common()
        .handle
        .value()
}

#[test]
fn distance_and_area_commands() {
    let (_dir, mut session) = sample_session();
    let d = measure(
        &mut session,
        Command::MeasureDistance {
            points: vec![[0.0, 0.0], [3.0, 0.0], [3.0, 4.0]],
        },
    );
    assert_eq!(d.kind, Kind::Length);
    assert_eq!(d.value, 7.0);
    assert_eq!(d.unit, "m");
    assert_eq!(d.shown, 7.0);
    assert_eq!(d.description, "Distance");
    assert_eq!(d.sheet, "*Model_Space");
    assert_eq!(d.source, "user");
    assert_eq!(d.points.len(), 3);

    let a = measure(&mut session, area(&RECT_4X3));
    assert_eq!(a.kind, Kind::Area);
    assert_eq!(a.value, 12.0);
    assert_eq!(a.unit, "m2");
    assert_eq!(a.shown, 12.0);
    assert_eq!(a.description, "Area");
    assert!(!a.self_intersecting);
    assert_ne!(a.id, d.id);

    let ids: Vec<_> = session.takeoff.items.iter().map(|m| m.id.clone()).collect();
    assert_eq!(ids, [d.id.clone(), a.id.clone()]);
    assert_eq!(session.takeoff.get(&a.id).unwrap().value, 12.0);

    assert!(matches!(
        session.run(Command::MeasureDistance {
            points: vec![[0.0, 0.0]]
        }),
        Err(CoreError::InvalidCommand(_))
    ));
    assert!(matches!(
        session.run(area(&RECT_4X3[..2])),
        Err(CoreError::InvalidCommand(_))
    ));
    assert_eq!(session.takeoff.items.len(), 2);
}

#[test]
fn bowtie_area_is_flagged() {
    let (_dir, mut session) = sample_session();
    let m = measure(
        &mut session,
        area(&[[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]]),
    );
    assert!(m.self_intersecting);
    assert_eq!(m.value, 2.0);
}

#[test]
fn measure_entity_rect_line_and_insert() {
    let (_dir, mut session) = sample_session();
    let rect = handle_of(&session, |e| matches!(e, EntityType::LwPolyline(_)));
    let line = handle_of(&session, |e| matches!(e, EntityType::Line(_)));
    let door = handle_of(&session, |e| matches!(e, EntityType::Insert(_)));

    let m = measure(&mut session, Command::MeasureEntity { handle: rect });
    assert_eq!((m.kind, m.value, m.unit.as_str()), (Kind::Area, 12.0, "m2"));
    assert_eq!(m.description, "LwPolyline on A-WALL");
    assert_eq!(m.layer, "A-WALL");
    assert_eq!(m.handles, [rect]);

    let m = measure(&mut session, Command::MeasureEntity { handle: line });
    assert_eq!(
        (m.kind, m.value, m.unit.as_str()),
        (Kind::Length, 10.0, "m")
    );
    assert_eq!(m.description, "Line on A-WALL");

    let m = measure(&mut session, Command::MeasureEntity { handle: door });
    assert_eq!(
        (m.kind, m.value, m.unit.as_str()),
        (Kind::Count, 1.0, "pcs")
    );
    assert_eq!(m.block_name.as_deref(), Some("DOOR"));
    assert_eq!(m.layer, "A-DOOR");

    assert!(matches!(
        session.run(Command::MeasureEntity {
            handle: 0xDEAD_BEEF
        }),
        Err(CoreError::NoSuchEntity(0xDEAD_BEEF))
    ));
}

#[test]
fn count_blocks_respects_hidden_layers() {
    let (_dir, mut session) = sample_session();
    let count = |layer: Option<&str>| Command::CountBlock {
        // Block names match case-insensitively, as in DXF.
        block_name: "door".to_string(),
        layer: layer.map(str::to_string),
    };

    let m = measure(&mut session, count(None));
    assert_eq!((m.kind, m.value, m.shown), (Kind::Count, 3.0, 3.0));
    assert_eq!(m.unit, "pcs");
    assert_eq!(m.handles.len(), 3);
    assert_eq!(m.description, "door count");
    assert_eq!(measure(&mut session, count(Some("a-door"))).value, 3.0);
    assert_eq!(measure(&mut session, count(Some("A-WALL"))).value, 0.0);

    let hide = Command::SetLayerHidden {
        name: "a-door".to_string(),
        hidden: true,
    };
    assert!(matches!(session.run(hide).unwrap(), Outcome::Changed));
    let m = measure(&mut session, count(None));
    assert_eq!(m.value, 0.0);
    assert!(m.handles.is_empty());
}

#[test]
fn unknown_units_assume_mm() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unitless.dxf");
    let mut doc = qiyas_core::fixtures::sample_doc();
    doc.header.insertion_units = 0;
    DxfWriter::new(&doc).write_to_file(&path).unwrap();
    let mut session = Session::open(&path).unwrap();
    assert_eq!(session.info().units, "unknown");

    let m = measure(
        &mut session,
        Command::MeasureDistance {
            points: vec![[0.0, 0.0], [1000.0, 0.0]],
        },
    );
    assert_eq!(m.value, 1000.0);
    assert_eq!(m.unit, "m");
    assert_eq!(m.shown, 1.0);

    let m = measure(&mut session, area(&RECT_4X3));
    assert!((m.shown - 12e-6).abs() < 1e-15, "{}", m.shown);
}

#[test]
fn undo_redo_measurements() {
    let (_dir, mut session) = sample_session();
    assert!(!session.can_undo() && !session.can_redo());
    assert!(!session.undo());

    let first = measure(&mut session, area(&RECT_4X3));
    measure(
        &mut session,
        Command::MeasureDistance {
            points: vec![[0.0, 0.0], [1.0, 0.0]],
        },
    );
    assert_eq!(session.takeoff.items.len(), 2);

    assert!(session.undo());
    assert_eq!(session.takeoff.items.len(), 1);
    assert_eq!(session.takeoff.items[0].id, first.id);
    assert!(session.undo());
    assert!(session.takeoff.items.is_empty());
    assert!(!session.undo());
    assert!(session.redo());
    assert_eq!(session.takeoff.items.len(), 1);
    assert!(session.can_undo() && session.can_redo());

    // Description and removal are undoable too.
    let rename = Command::SetDescription {
        id: first.id.clone(),
        text: "Slab".to_string(),
    };
    assert!(matches!(session.run(rename).unwrap(), Outcome::Changed));
    // A new command drops what could have been redone.
    assert!(!session.can_redo());
    assert_eq!(session.takeoff.get(&first.id).unwrap().description, "Slab");
    let remove = Command::RemoveMeasurement {
        id: first.id.clone(),
    };
    assert!(matches!(session.run(remove).unwrap(), Outcome::Removed { id } if id == first.id));
    assert!(session.takeoff.items.is_empty());
    assert!(session.undo());
    assert_eq!(session.takeoff.get(&first.id).unwrap().description, "Slab");
    assert!(session.undo());
    assert_eq!(session.takeoff.get(&first.id).unwrap().description, "Area");

    // An unknown id changes nothing and leaves no undo step.
    assert!(session.undo());
    let gone = Command::RemoveMeasurement {
        id: "nope".to_string(),
    };
    assert!(matches!(session.run(gone).unwrap(), Outcome::Nothing));
    assert!(!session.can_undo());
    assert!(session.can_redo());

    // Sheet and layer changes are not undo steps.
    let sheet = Command::SetSheet {
        block: "*model_space".to_string(),
    };
    assert!(matches!(session.run(sheet).unwrap(), Outcome::Changed));
    assert!(!session.can_undo());
}

#[test]
fn totals_group_by_kind_and_unit() {
    let (_dir, mut session) = sample_session();
    measure(&mut session, area(&RECT_4X3));
    measure(
        &mut session,
        area(&[[10.0, 0.0], [13.0, 0.0], [13.0, 1.0], [10.0, 1.0]]),
    );
    let totals = session.takeoff.totals();
    assert_eq!(totals.len(), 1);
    let t = &totals[0];
    assert_eq!(
        (t.kind, t.unit.as_str(), t.count, t.sum),
        (Kind::Area, "m2", 2, 15.0)
    );

    measure(
        &mut session,
        Command::MeasureDistance {
            points: vec![[0.0, 0.0], [2.0, 0.0]],
        },
    );
    let kinds: Vec<_> = session.takeoff.totals().iter().map(|t| t.kind).collect();
    assert_eq!(kinds, [Kind::Area, Kind::Length]);
}

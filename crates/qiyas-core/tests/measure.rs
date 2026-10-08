use opencadcodec::{DxfWriter, EntityType};
use qiyas_core::{Command, CoreError, Kind, Measurement, Outcome, Session, Source};

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

/// An `AddMeasurement` of `kind` with no value, unit, height, geometry or text; tests fill in
/// what they need.
fn add(kind: Kind) -> Command {
    Command::AddMeasurement {
        kind,
        value: None,
        unit: None,
        height: None,
        points: Vec::new(),
        handles: Vec::new(),
        description: String::new(),
        note: String::new(),
    }
}

/// `cmd` with its `AddMeasurement` fields changed by `f`.
fn with(
    mut cmd: Command,
    f: impl FnOnce(
        &mut Option<f64>,
        &mut Option<String>,
        &mut Option<f64>,
        &mut Vec<[f64; 2]>,
        &mut Vec<u64>,
    ),
) -> Command {
    let Command::AddMeasurement {
        value,
        unit,
        height,
        points,
        handles,
        ..
    } = &mut cmd
    else {
        unreachable!()
    };
    f(value, unit, height, points, handles);
    cmd
}

fn is_invalid(result: Result<Outcome, CoreError>) -> bool {
    matches!(result, Err(CoreError::InvalidCommand(_)))
}

#[test]
fn volume_from_area_and_height() {
    let (_dir, mut session) = sample_session();
    let slab = with(add(Kind::Volume), |_, _, h, p, _| {
        *h = Some(0.2);
        *p = RECT_4X3.to_vec();
    });
    let m = measure(&mut session, slab.clone());
    assert_eq!(m.kind, Kind::Volume);
    assert_eq!(m.unit, "m3");
    assert!((m.value - 2.4).abs() < 1e-12, "{}", m.value);
    assert!((m.shown - 2.4).abs() < 1e-12, "{}", m.shown);
    assert_eq!(m.height, Some(0.2));
    assert_eq!(m.points, RECT_4X3);

    // A volume needs a positive height, and its value comes from the geometry.
    let no_height = with(slab.clone(), |_, _, h, _, _| *h = None);
    let flat = with(slab.clone(), |_, _, h, _, _| *h = Some(0.0));
    let given = with(slab.clone(), |v, _, _, _, _| *v = Some(2.4));
    let unit = with(slab, |_, u, _, _, _| *u = Some("m3".to_string()));
    for cmd in [no_height, flat, given, unit] {
        assert!(is_invalid(session.run(cmd)));
    }

    // From a closed entity too: the 4 x 3 rectangle.
    let rect = handle_of(&session, |e| matches!(e, EntityType::LwPolyline(_)));
    let m = measure(
        &mut session,
        with(add(Kind::Volume), |_, _, h, _, hs| {
            *h = Some(0.5);
            *hs = vec![rect];
        }),
    );
    assert!((m.shown - 6.0).abs() < 1e-9, "{}", m.shown);
    assert_eq!(m.layer, "A-WALL");

    // In millimetres the volume is shown in cubic metres.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mm.dxf");
    let mut doc = qiyas_core::fixtures::sample_doc();
    doc.header.insertion_units = 4;
    DxfWriter::new(&doc).write_to_file(&path).unwrap();
    let mut session = Session::open(&path).unwrap();
    let m = measure(
        &mut session,
        with(add(Kind::Volume), |_, _, h, p, _| {
            *h = Some(200.0);
            *p = vec![[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0], [0.0, 3000.0]];
        }),
    );
    assert!((m.shown - 2.4).abs() < 1e-9, "{}", m.shown);
}

#[test]
fn item_measurement_keeps_its_unit() {
    let (_dir, mut session) = sample_session();
    let bags = with(add(Kind::Item), |v, u, _, _, _| {
        *v = Some(5.0);
        *u = Some("bags".to_string());
    });
    let m = measure(&mut session, bags.clone());
    assert_eq!(m.kind, Kind::Item);
    assert_eq!((m.value, m.shown), (5.0, 5.0));
    assert_eq!(m.unit, "bags");
    assert!(m.points.is_empty() && m.handles.is_empty());

    // An item needs both its value and its unit.
    let no_value = with(bags.clone(), |v, _, _, _, _| *v = None);
    let no_unit = with(bags.clone(), |_, u, _, _, _| *u = None);
    let blank_unit = with(bags, |_, u, _, _, _| *u = Some(" ".to_string()));
    for cmd in [no_value, no_unit, blank_unit] {
        assert!(is_invalid(session.run(cmd)));
    }
}

#[test]
fn add_measurement_requires_geometry() {
    let (_dir, mut session) = sample_session();
    for kind in [Kind::Length, Kind::Area, Kind::Count] {
        match session.run(add(kind)) {
            Err(CoreError::InvalidCommand(why)) => assert_eq!(why, "A measurement needs geometry"),
            other => panic!("expected InvalidCommand, got {other:?}"),
        }
    }
    assert!(is_invalid(
        session.run(with(add(Kind::Volume), |_, _, h, _, _| { *h = Some(1.0) }))
    ));
    assert!(session.takeoff.items.is_empty());

    // Lengths, areas and counts are worked out from the geometry, never given.
    let given = with(add(Kind::Length), |v, _, _, p, _| {
        *v = Some(3.0);
        *p = vec![[0.0, 0.0], [3.0, 0.0]];
    });
    assert!(is_invalid(session.run(given)));
    let height = with(add(Kind::Area), |_, _, h, p, _| {
        *h = Some(1.0);
        *p = RECT_4X3.to_vec();
    });
    assert!(is_invalid(session.run(height)));

    let rect = handle_of(&session, |e| matches!(e, EntityType::LwPolyline(_)));
    let line = handle_of(&session, |e| matches!(e, EntityType::Line(_)));
    let doors: Vec<u64> = session
        .drawing
        .entities_in("*Model_Space")
        .filter(|e| matches!(e, EntityType::Insert(_)))
        .map(|e| e.common().handle.value())
        .collect();
    let m = measure(
        &mut session,
        with(add(Kind::Area), |_, _, _, _, hs| *hs = vec![rect]),
    );
    assert_eq!((m.kind, m.value, m.unit.as_str()), (Kind::Area, 12.0, "m2"));
    assert_eq!(m.handles, [rect]);
    let m = measure(
        &mut session,
        with(add(Kind::Length), |_, _, _, _, hs| *hs = vec![line, rect]),
    );
    assert_eq!((m.kind, m.value), (Kind::Length, 24.0));
    let m = measure(
        &mut session,
        with(add(Kind::Count), |_, _, _, _, hs| *hs = doors.clone()),
    );
    assert_eq!(
        (m.kind, m.value, m.unit.as_str()),
        (Kind::Count, 3.0, "pcs")
    );
    let m = measure(
        &mut session,
        with(add(Kind::Count), |_, _, _, p, _| {
            *p = vec![[1.0, 1.0], [2.0, 2.0]]
        }),
    );
    assert_eq!(m.value, 2.0);
    let m = measure(
        &mut session,
        with(add(Kind::Length), |_, _, _, p, _| {
            *p = vec![[0.0, 0.0], [3.0, 4.0]]
        }),
    );
    assert_eq!(m.value, 5.0);

    // A line has no area; a handle must name an entity.
    assert!(is_invalid(
        session.run(with(add(Kind::Area), |_, _, _, _, hs| *hs = vec![line]))
    ));
    assert!(matches!(
        session.run(with(add(Kind::Length), |_, _, _, _, hs| *hs = vec![0xDEAD_BEEF])),
        Err(CoreError::NoSuchEntity(0xDEAD_BEEF))
    ));
}

#[test]
fn run_as_ai_marks_source() {
    let (_dir, mut session) = sample_session();
    let distance = || Command::MeasureDistance {
        points: vec![[0.0, 0.0], [3.0, 0.0]],
    };
    let sources: Vec<String> = [Source::Ai, Source::Mcp, Source::User]
        .into_iter()
        .map(|source| match session.run_as(distance(), source).unwrap() {
            Outcome::Measured { measurement } => measurement.source,
            other => panic!("expected a measurement, got {other:?}"),
        })
        .collect();
    assert_eq!(sources, ["ai", "mcp", "user"]);
    assert_eq!(measure(&mut session, distance()).source, "user");

    let cmd = Command::AddMeasurement {
        kind: Kind::Area,
        value: None,
        unit: None,
        height: None,
        points: RECT_4X3.to_vec(),
        handles: Vec::new(),
        description: "Slab".to_string(),
        note: "From the AI".to_string(),
    };
    let m = match session.run_as(cmd, Source::Ai).unwrap() {
        Outcome::Measured { measurement } => *measurement,
        other => panic!("expected a measurement, got {other:?}"),
    };
    assert_eq!(
        (m.source.as_str(), m.description.as_str(), m.note.as_str()),
        ("ai", "Slab", "From the AI")
    );
    // An AI change is undoable like any other.
    assert!(session.undo());
    assert!(session.takeoff.get(&m.id).is_none());
}

#[test]
fn set_height_is_undoable() {
    let (_dir, mut session) = sample_session();
    let a = measure(&mut session, area(&RECT_4X3));
    let set = |height: f64| Command::SetHeight {
        id: a.id.clone(),
        height,
    };
    assert!(matches!(session.run(set(0.2)).unwrap(), Outcome::Changed));
    let v = session.takeoff.get(&a.id).unwrap().clone();
    assert_eq!(
        (v.kind, v.unit.as_str(), v.height),
        (Kind::Volume, "m3", Some(0.2))
    );
    assert!((v.value - 2.4).abs() < 1e-12 && (v.shown - 2.4).abs() < 1e-12);
    assert_eq!(v.points, RECT_4X3);

    // A new height on a volume keeps its area.
    session.run(set(0.5)).unwrap();
    let v = session.takeoff.get(&a.id).unwrap();
    assert!((v.shown - 6.0).abs() < 1e-12, "{}", v.shown);

    assert!(session.undo());
    assert!((session.takeoff.get(&a.id).unwrap().shown - 2.4).abs() < 1e-12);
    assert!(session.undo());
    let back = session.takeoff.get(&a.id).unwrap();
    assert_eq!(
        (back.kind, back.value, back.unit.as_str(), back.height),
        (Kind::Area, 12.0, "m2", None)
    );
    assert!(session.redo());
    assert_eq!(session.takeoff.get(&a.id).unwrap().kind, Kind::Volume);

    assert!(is_invalid(session.run(set(0.0))));
    assert!(is_invalid(session.run(set(-1.0))));
    let d = measure(
        &mut session,
        Command::MeasureDistance {
            points: vec![[0.0, 0.0], [1.0, 0.0]],
        },
    );
    assert!(is_invalid(session.run(Command::SetHeight {
        id: d.id,
        height: 1.0
    })));
    let none = Command::SetHeight {
        id: "nope".to_string(),
        height: 1.0,
    };
    assert!(matches!(session.run(none).unwrap(), Outcome::Nothing));
}

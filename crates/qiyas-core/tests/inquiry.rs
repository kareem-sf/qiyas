use std::f64::consts::{PI, TAU};

use opencadcodec::entities::AttributeEntity;
use opencadcodec::{Circle, Color, EntityType, LwPolyline, Vector2, Vector3};
use qiyas_core::{
    Command, CoreError, Drafting, EntityFilter, Field, Kind, Measurement, Outcome, Session,
    SnapKind, POINT_LAYER,
};

/// The sample drawing (metres) with a fountain circle of radius 1.5 at (3.5, 4.5) on A-WALL, MARK
/// attributes D1, D2, D3 on its three DOOR references, and a polyline on A-WALL from (0, 20) with
/// a clockwise half turn to (2, 20) bulging up, then straight up to (2, 25). Precision 4.
fn session() -> (tempfile::TempDir, Session) {
    let mut doc = qiyas_core::fixtures::sample_doc();
    let mut circle = Circle::new();
    circle.center = Vector3::new(3.5, 4.5, 0.0);
    circle.radius = 1.5;
    circle.common.layer = "A-WALL".to_string();
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    let mut path = LwPolyline::new();
    for (x, y) in [(0.0, 20.0), (2.0, 20.0), (2.0, 25.0)] {
        path.add_point(Vector2::new(x, y));
    }
    path.vertices[0].bulge = -1.0;
    path.common.layer = "A-WALL".to_string();
    doc.add_entity(EntityType::LwPolyline(path)).unwrap();
    let doors: Vec<_> = doc
        .entities()
        .filter(|e| matches!(e, EntityType::Insert(_)))
        .map(|e| e.common().handle)
        .collect();
    for (i, h) in doors.into_iter().enumerate() {
        let Some(EntityType::Insert(insert)) = doc.get_entity_mut(h) else {
            unreachable!()
        };
        let at = insert.insert_point;
        insert.attributes.push(
            AttributeEntity::new("MARK".to_string(), format!("D{}", i + 1))
                .with_position(at)
                .with_height(0.2),
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_drawing(&doc, &dir.path().join("inquiry.dxf"));
    let mut session = Session::open(&path).unwrap();
    let drafting = Drafting {
        precision: 4,
        ..Drafting::default()
    };
    session.run(Command::SetDrafting { drafting }).unwrap();
    (dir, session)
}

fn handle_of(session: &Session, is: impl Fn(&EntityType) -> bool) -> u64 {
    session
        .drawing
        .entities_in("*Model_Space")
        .find(|e| is(e))
        .expect("entity in the drawing")
        .common()
        .handle
        .value()
}

fn line(s: &Session) -> u64 {
    handle_of(s, |e| matches!(e, EntityType::Line(_)))
}

/// The Info rows of `cmd` as (label, value), and its title.
fn info(session: &mut Session, cmd: Command) -> (String, Vec<(String, String)>) {
    match session.run(cmd).unwrap() {
        Outcome::Info { title, rows } => (title, rows),
        other => panic!("expected info, got {other:?}"),
    }
}

fn value<'a>(rows: &'a [(String, String)], label: &str) -> &'a str {
    &rows
        .iter()
        .find(|(l, _)| l == label)
        .unwrap_or_else(|| panic!("no {label} in {rows:?}"))
        .1
}

fn geometry(session: &mut Session, handle: u64) -> (String, Vec<(String, String)>) {
    info(
        session,
        Command::MeasureGeometry {
            handle,
            chain: Vec::new(),
            at: None,
        },
    )
}

#[test]
fn circle_arc_and_line_geometry() {
    let (_dir, mut s) = session();
    let undo_before = s.can_undo();

    let circle = handle_of(&s, |e| matches!(e, EntityType::Circle(_)));
    let (title, rows) = geometry(&mut s, circle);
    assert_eq!(title, "Circle");
    assert_eq!(value(&rows, "layer"), "A-WALL");
    assert_eq!(value(&rows, "radius"), "1.5000 m");
    assert_eq!(value(&rows, "diameter"), "3.0000 m");
    assert_eq!(value(&rows, "centre"), "3.5000, 4.5000 m");
    assert_eq!(value(&rows, "perimeter"), format!("{:.4} m", TAU * 1.5));
    assert_eq!(value(&rows, "area"), format!("{:.4} m²", PI * 2.25));

    let arc = handle_of(&s, |e| matches!(e, EntityType::Arc(_)));
    let (title, rows) = geometry(&mut s, arc);
    assert_eq!(title, "Arc");
    assert_eq!(value(&rows, "radius"), "2.0000 m");
    assert_eq!(value(&rows, "centre"), "20.0000, 0.0000 m");
    assert_eq!(value(&rows, "start_angle"), "0.0000°");
    assert_eq!(value(&rows, "end_angle"), "180.0000°");
    assert_eq!(value(&rows, "included_angle"), "180.0000°");
    assert_eq!(value(&rows, "arc_length"), format!("{:.4} m", TAU));

    let l = line(&s);
    let (title, rows) = geometry(&mut s, l);
    assert_eq!(title, "Line");
    assert_eq!(value(&rows, "length"), "10.0000 m");
    assert_eq!(value(&rows, "angle"), "0.0000°");
    assert_eq!(value(&rows, "delta_x"), "10.0000 m");
    assert_eq!(value(&rows, "delta_y"), "0.0000 m");

    // Inquiry is not an undo step and measures nothing.
    assert_eq!(s.can_undo(), undo_before);
    assert!(s.takeoff.items.is_empty());
}

#[test]
fn a_polyline_gives_the_segment_nearest_the_click() {
    let (_dir, mut s) = session();
    let path = handle_of(
        &s,
        |e| matches!(e, EntityType::LwPolyline(p) if p.vertices.len() == 3),
    );
    let at = |p: [f64; 2]| Command::MeasureGeometry {
        handle: path,
        chain: Vec::new(),
        at: Some(p),
    };
    let (_, rows) = info(&mut s, at([1.0, 21.0]));
    assert_eq!(value(&rows, "segment"), "1 / 2");
    assert_eq!(value(&rows, "radius"), "1.0000 m");
    assert_eq!(value(&rows, "included_angle"), "180.0000°");
    assert_eq!(value(&rows, "total_length"), format!("{:.4} m", PI + 5.0));
    let (_, rows) = info(&mut s, at([2.1, 23.0]));
    assert_eq!(value(&rows, "segment"), "2 / 2");
    assert_eq!(value(&rows, "length"), "5.0000 m");
    assert_eq!(value(&rows, "angle"), "90.0000°");
}

#[test]
fn block_reference_and_text_geometry() {
    let (_dir, mut s) = session();
    let door = handle_of(&s, |e| matches!(e, EntityType::Insert(_)));
    let (title, rows) = geometry(&mut s, door);
    assert_eq!(title, "Insert");
    assert_eq!(value(&rows, "block_name"), "DOOR");
    assert_eq!(value(&rows, "insertion"), "1.0000, 1.0000 m");
    assert_eq!(value(&rows, "scale"), "1.0000, 1.0000");
    assert_eq!(value(&rows, "rotation"), "0.0000°");
    assert_eq!(value(&rows, "attribute"), "MARK = D1");

    let text = handle_of(&s, |e| matches!(e, EntityType::Text(_)));
    let (_, rows) = geometry(&mut s, text);
    assert_eq!(value(&rows, "text"), "GROUND FLOOR PLAN");
    assert_eq!(value(&rows, "height"), "0.5000 m");
}

fn points_on_layer(s: &Session) -> Vec<(u64, [f64; 2])> {
    s.drawing
        .doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Point(p) if p.common.layer == POINT_LAYER => {
                Some((p.common.handle.value(), [p.location.x, p.location.y]))
            }
            _ => None,
        })
        .collect()
}

fn measured(s: &mut Session, cmd: Command) -> Measurement {
    match s.run(cmd).unwrap() {
        Outcome::Measured { measurement } => *measurement,
        other => panic!("expected a measurement, got {other:?}"),
    }
}

fn xs(points: &[[f64; 2]]) -> Vec<f64> {
    points.iter().map(|p| (p[0] * 1e9).round() / 1e9).collect()
}

#[test]
fn divide_a_line_into_five_and_undo_it() {
    let (dir, mut s) = session();
    assert!(!s.is_dirty());
    let l = line(&s);
    let m = measured(
        &mut s,
        Command::DivideAlong {
            handle: l,
            chain: Vec::new(),
            count: 5,
        },
    );
    assert_eq!(m.kind, Kind::Count);
    assert_eq!((m.value, m.unit.as_str()), (4.0, "pcs"));
    assert_eq!(m.description, "Divide 5 on A-WALL");
    assert_eq!(m.block_name, None);
    assert_eq!(m.handles.len(), 4);
    assert_eq!(xs(&m.points), [2.0, 4.0, 6.0, 8.0]);

    let placed = points_on_layer(&s);
    let handles: Vec<u64> = placed.iter().map(|(h, _)| *h).collect();
    assert_eq!(handles, m.handles);
    let at: Vec<[f64; 2]> = placed.iter().map(|(_, p)| *p).collect();
    assert_eq!(xs(&at), [2.0, 4.0, 6.0, 8.0]);
    assert!(at.iter().all(|p| p[1].abs() < 1e-12));
    let layer = s.drawing.doc.layers.get(POINT_LAYER).unwrap();
    assert_eq!(layer.color, Color::Index(2));
    assert!(s.is_dirty(), "the drawing itself changed");
    assert_eq!(s.takeoff.items.len(), 1);

    // One undo step takes back the points and the count.
    assert!(s.undo());
    assert!(points_on_layer(&s).is_empty());
    assert!(s.takeoff.items.is_empty());
    assert!(s.redo());
    assert_eq!(points_on_layer(&s).len(), 4);
    assert_eq!(s.takeoff.items[0].id, m.id);

    // Saved without its project file, the drawing keeps the points and the count.
    let out = dir.path().join("divided.dxf");
    s.save_as(&out).unwrap();
    std::fs::remove_file(qiyas_core::project_path(&out)).unwrap();
    let reopened = Session::open(&out).unwrap();
    assert_eq!(points_on_layer(&reopened).len(), 4);
    let count = &reopened.takeoff.items[0];
    assert_eq!((count.kind, count.value), (Kind::Count, 4.0));
    assert_eq!(xs(&count.points), [2.0, 4.0, 6.0, 8.0]);
}

#[test]
fn measure_along_a_line_every_three() {
    let (_dir, mut s) = session();
    let l = line(&s);
    let m = measured(
        &mut s,
        Command::MeasureAlong {
            handle: l,
            chain: Vec::new(),
            step: 3.0,
        },
    );
    assert_eq!(m.value, 3.0);
    assert_eq!(xs(&m.points), [3.0, 6.0, 9.0]);
    assert_eq!(m.description, "Measure every 3 on A-WALL");

    // A step that fits exactly stops before the end.
    let m = measured(
        &mut s,
        Command::MeasureAlong {
            handle: l,
            chain: Vec::new(),
            step: 2.5,
        },
    );
    assert_eq!(xs(&m.points), [2.5, 5.0, 7.5]);
}

#[test]
fn points_follow_arcs_by_true_length_from_the_start() {
    let (_dir, mut s) = session();
    // The sample's arc: radius 2 round (20, 0) from 0 to 180 degrees; its middle is (20, 2).
    let arc = handle_of(&s, |e| matches!(e, EntityType::Arc(_)));
    let m = measured(
        &mut s,
        Command::DivideAlong {
            handle: arc,
            chain: Vec::new(),
            count: 2,
        },
    );
    let [x, y] = m.points[0];
    assert!(
        (x - 20.0).abs() < 1e-9 && (y - 2.0).abs() < 1e-9,
        "{x}, {y}"
    );

    // The polyline starts with a clockwise half turn above (1, 20): one unit along it from (0, 20)
    // is at angle 180° - 1 rad round (1, 20).
    let path = handle_of(
        &s,
        |e| matches!(e, EntityType::LwPolyline(p) if p.vertices.len() == 3),
    );
    let m = measured(
        &mut s,
        Command::MeasureAlong {
            handle: path,
            chain: Vec::new(),
            step: 1.0,
        },
    );
    let [x, y] = m.points[0];
    assert!((x - (1.0 - 1f64.cos())).abs() < 1e-9, "{x}");
    assert!((y - (20.0 + 1f64.sin())).abs() < 1e-9, "{y}");
    // π + 5 long: points at 1 to 8.
    assert_eq!(m.value, 8.0);
    let [x, y] = m.points[3];
    assert!(
        (x - 2.0).abs() < 1e-9 && (y - (20.0 + 4.0 - PI)).abs() < 1e-9,
        "{x}, {y}"
    );

    // A closed outline divides into as many points as parts, from its start.
    let rectangle = handle_of(
        &s,
        |e| matches!(e, EntityType::LwPolyline(p) if p.is_closed),
    );
    let m = measured(
        &mut s,
        Command::DivideAlong {
            handle: rectangle,
            chain: Vec::new(),
            count: 7,
        },
    );
    assert_eq!(m.value, 7.0);
    assert_eq!(m.points[0], [0.0, 0.0]);
    assert_eq!(xs(&m.points[1..2]), [2.0]);
}

#[test]
fn divide_and_measure_refuse_bad_input() {
    let (_dir, mut s) = session();
    let l = line(&s);
    for count in [0, 1, 1001] {
        let err = s.run(Command::DivideAlong {
            handle: l,
            chain: Vec::new(),
            count,
        });
        assert!(matches!(err, Err(CoreError::InvalidCommand(_))), "{count}");
    }
    for step in [0.0, -1.0, f64::NAN, 10.0, 11.0, 0.001] {
        let err = s.run(Command::MeasureAlong {
            handle: l,
            chain: Vec::new(),
            step,
        });
        assert!(matches!(err, Err(CoreError::InvalidCommand(_))), "{step}");
    }
    let door = handle_of(&s, |e| matches!(e, EntityType::Insert(_)));
    let err = s.run(Command::DivideAlong {
        handle: door,
        chain: Vec::new(),
        count: 2,
    });
    assert!(matches!(err, Err(CoreError::InvalidCommand(_))));
    assert!(points_on_layer(&s).is_empty());
    assert!(!s.can_undo());
}

#[test]
fn id_point_in_metres_with_its_snap() {
    let (_dir, mut s) = session();
    let (title, rows) = info(
        &mut s,
        Command::IdPoint {
            p: [3.5, -0.25],
            kind: Some(SnapKind::End),
        },
    );
    assert_eq!(title, "Point");
    assert_eq!(
        rows,
        [
            ("x".to_string(), "3.5000 m".to_string()),
            ("y".to_string(), "-0.2500 m".to_string()),
            ("snap".to_string(), "end".to_string()),
        ]
    );
    let (_, rows) = info(
        &mut s,
        Command::IdPoint {
            p: [0.0, 0.0],
            kind: None,
        },
    );
    assert_eq!(rows.len(), 2);
    assert!(!s.can_undo());
}

#[test]
fn extract_door_attributes_to_a_table() {
    let (_dir, mut s) = session();
    let cmd: Command = serde_json::from_value(serde_json::json!({
        "type": "extract_data",
        "filter": { "block_name": "door", "limit": 5000 },
        "fields": ["handle", "block_name", { "attribute": "mark" }, "length", "x", "y", "sheet", "layer"]
    }))
    .unwrap();
    let Outcome::Table { columns, rows } = s.run(cmd).unwrap() else {
        panic!("expected a table");
    };
    assert_eq!(
        columns,
        [
            "Handle",
            "Block",
            "mark",
            "Length (m)",
            "X (m)",
            "Y (m)",
            "Sheet",
            "Layer"
        ]
    );
    assert_eq!(rows.len(), 3);
    let marks: Vec<&str> = rows.iter().map(|r| r[2].as_str()).collect();
    assert_eq!(marks, ["D1", "D2", "D3"]);
    assert!(rows.iter().all(|r| r[1] == "DOOR" && r[3].is_empty()));
    assert_eq!(
        (rows[0][4].as_str(), rows[0][5].as_str()),
        ("1.0000", "1.0000")
    );
    assert_eq!(rows[2][4], "9.0000");
    assert_eq!(rows[0][6], "Model");
    assert_eq!(rows[0][7], "A-DOOR");
    assert!(rows[0][0].parse::<u64>().is_ok());
    assert!(!s.can_undo());

    // Lengths of lines, in metres; a page at a time.
    let filter = EntityFilter {
        kind: Some("Line".to_string()),
        ..Default::default()
    };
    let Outcome::Table { rows, .. } = s
        .run(Command::ExtractData {
            filter,
            fields: vec![
                Field::Kind,
                Field::Length,
                Field::Area,
                Field::Attribute("MARK".into()),
            ],
        })
        .unwrap()
    else {
        panic!("expected a table");
    };
    assert_eq!(rows, [["Line", "10.0000", "", ""]]);

    let none = s.run(Command::ExtractData {
        filter: EntityFilter::default(),
        fields: Vec::new(),
    });
    assert!(matches!(none, Err(CoreError::InvalidCommand(_))));
}

#[test]
fn a_bound_xref_layer_is_shown_short_but_kept_whole() {
    const BOUND: &str = "02-First floor (arch&furn)$0$02-FIRST FLOOR (ARCH&FURN)$0$A-WALL";
    let mut doc = qiyas_core::fixtures::sample_doc();
    let mut layer = opencadcodec::Layer::new(BOUND);
    layer.handle = doc.allocate_handle();
    doc.layers.add(layer).unwrap();
    let mut wall = opencadcodec::Line::from_coords(0.0, 30.0, 0.0, 6.0, 30.0, 0.0);
    wall.common.layer = BOUND.to_string();
    doc.add_entity(EntityType::Line(wall)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_drawing(&doc, &dir.path().join("bound.dxf"));
    let mut s = Session::open(&path).unwrap();
    let h = handle_of(&s, |e| e.common().layer == BOUND);

    let one = |handle| Command::MeasureEntity {
        handle,
        chain: Vec::new(),
    };
    let m = measured(&mut s, one(h));
    assert_eq!(m.description, "Line on A-WALL");
    assert_eq!(m.layer, BOUND);
    assert!(m.auto_description);

    let palette = s.entity_info(h, &[]).unwrap();
    assert_eq!(
        (palette.layer.as_str(), palette.layer_short.as_str()),
        (BOUND, "A-WALL")
    );

    let (_, rows) = geometry(&mut s, h);
    assert_eq!(value(&rows, "layer"), "A-WALL");

    let divide = Command::DivideAlong {
        handle: h,
        chain: Vec::new(),
        count: 3,
    };
    assert_eq!(measured(&mut s, divide).description, "Divide 3 on A-WALL");
    let along = Command::MeasureAlong {
        handle: h,
        chain: Vec::new(),
        step: 2.5,
    };
    assert_eq!(
        measured(&mut s, along).description,
        "Measure every 2.5 on A-WALL"
    );
}

//! Parametric constraints through the command bus: AutoConstrain on the sample rectangle,
//! Review Focus 4 (a rectangle with 4 coincident, 2 horizontal, 2 vertical and a width
//! constraint stays a rectangle when the width changes), geometric constraints moving what they
//! hold, a move re-solving in the same step, an erase taking the constraints along, expressions
//! following each other (circles refused), and constraints surviving a DWG and DXF round trip.

use std::time::Instant;

use opencadcodec::{CadDocument, EntityType, Handle, Line};
use qiyas_core::fixtures::{sample_doc, write_drawing};
use qiyas_core::{
    AutoSettings, Command, ConstraintInfo, ConstraintKind, ModifySpec, Outcome, ParameterInfo,
    Session, Target, PARAM_APP,
};

const TOL: f64 = 1e-6;

fn open(doc: &CadDocument, name: &str) -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = write_drawing(doc, &dir.path().join(name));
    (dir, Session::open(&path).unwrap())
}

fn t(handle: u64) -> Target {
    Target {
        handle,
        chain: Vec::new(),
    }
}

/// The handle of the first entity of the sheet `pick` takes.
fn find(s: &Session, pick: impl Fn(&EntityType) -> bool) -> u64 {
    s.drawing
        .doc
        .entities()
        .find(|e| pick(e))
        .map(|e| e.common().handle.value())
        .expect("in the sample")
}

/// The sample's closed 4 x 3 rectangle (a polyline from (0, 0)).
fn rectangle(s: &Session) -> u64 {
    find(s, |e| matches!(e, EntityType::LwPolyline(_)))
}

/// The sample's 10 m line from (0, 0) to (10, 0).
fn wall_line(s: &Session) -> u64 {
    find(s, |e| matches!(e, EntityType::Line(l) if l.end.x == 10.0))
}

fn vertices(s: &Session, h: u64) -> Vec<[f64; 2]> {
    match s.drawing.doc.get_entity(Handle::new(h)) {
        Some(EntityType::LwPolyline(p)) => p
            .vertices
            .iter()
            .map(|v| [v.location.x, v.location.y])
            .collect(),
        other => panic!("{h:X} is no polyline: {other:?}"),
    }
}

fn line_ends(s: &Session, h: u64) -> [[f64; 2]; 2] {
    match s.drawing.doc.get_entity(Handle::new(h)) {
        Some(EntityType::Line(l)) => [[l.start.x, l.start.y], [l.end.x, l.end.y]],
        other => panic!("{h:X} is no line: {other:?}"),
    }
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < TOL && (a[1] - b[1]).abs() < TOL
}

fn assert_rect(v: &[[f64; 2]], corners: [[f64; 2]; 4]) {
    for (got, want) in v.iter().zip(corners) {
        assert!(near(*got, want), "{v:?} is not {corners:?}");
    }
}

fn constraints(s: &mut Session) -> Vec<ConstraintInfo> {
    match s.run(Command::ListConstraints).unwrap() {
        Outcome::Constraints { constraints } => constraints,
        other => panic!("{other:?}"),
    }
}

fn parameters(s: &mut Session) -> Vec<ParameterInfo> {
    match s.run(Command::ListParameters).unwrap() {
        Outcome::Parameters { parameters } => parameters,
        other => panic!("{other:?}"),
    }
}

fn ids(outcome: Outcome) -> Vec<u32> {
    match outcome {
        Outcome::Constrained { ids } => ids,
        other => panic!("{other:?}"),
    }
}

fn count(list: &[ConstraintInfo], kind: ConstraintKind) -> usize {
    list.iter().filter(|c| c.kind == kind).count()
}

fn auto(s: &mut Session, h: u64) -> Vec<u32> {
    ids(s
        .run(Command::AutoConstrain {
            targets: vec![t(h)],
            settings: AutoSettings::default(),
        })
        .unwrap())
}

/// The rectangle's width as a dimensional constraint on its bottom edge, its Dimension below.
fn width(s: &mut Session, rect: u64, expression: Option<&str>) -> u32 {
    ids(s
        .run(Command::AddDimConstraint {
            kind: ConstraintKind::Linear,
            targets: vec![t(rect)],
            points: vec![[2.0, 0.0]],
            line_point: [2.0, -1.0],
            expression: expression.map(str::to_string),
        })
        .unwrap())[0]
}

/// Its height on the left edge.
fn height(s: &mut Session, rect: u64) -> u32 {
    ids(s
        .run(Command::AddDimConstraint {
            kind: ConstraintKind::Linear,
            targets: vec![t(rect)],
            points: vec![[0.0, 1.5]],
            line_point: [-1.0, 1.5],
            expression: None,
        })
        .unwrap())[0]
}

#[test]
fn auto_constrain_on_the_sample_rectangle_infers_4_coincident_2_horizontal_2_vertical() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let rect = rectangle(&s);
    let added = auto(&mut s, rect);
    let list = constraints(&mut s);
    assert_eq!(added.len(), 8, "{list:?}");
    assert_eq!(count(&list, ConstraintKind::Coincident), 4);
    assert_eq!(count(&list, ConstraintKind::Horizontal), 2);
    assert_eq!(count(&list, ConstraintKind::Vertical), 2);
    assert!(list.iter().all(|c| c.handles == vec![rect]));
    // Run again, nothing new; and it is one undo step.
    assert!(auto(&mut s, rect).is_empty());
    assert!(s.undo());
    assert!(constraints(&mut s).is_empty());
}

#[test]
fn review_focus_4_the_rectangle_stays_a_rectangle_when_its_width_changes() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let rect = rectangle(&s);
    auto(&mut s, rect);
    width(&mut s, rect, None);
    let params = parameters(&mut s);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "d1");
    assert_eq!(params[0].expression, "4");
    assert!((params[0].value.unwrap() - 4.0).abs() < TOL);

    let t0 = Instant::now();
    s.run(Command::SetParameter {
        name: "d1".into(),
        expression: "6".into(),
    })
    .unwrap();
    let took = t0.elapsed();
    // The width's first point stays; the rectangle grows to the right.
    assert_rect(
        &vertices(&s, rect),
        [[0.0, 0.0], [6.0, 0.0], [6.0, 3.0], [0.0, 3.0]],
    );
    assert!(took.as_millis() < 50, "the width change took {took:?}");
    // The Dimension follows, shows its parameter in brackets (the lock mark), and reads 6.
    let dim = params[0].dimension;
    match s.drawing.doc.get_entity(Handle::new(dim)) {
        Some(EntityType::Dimension(d)) => {
            assert_eq!(d.base().user_text.as_deref(), Some("[d1=<>]"));
            assert!((d.measurement() - 6.0).abs() < TOL);
        }
        other => panic!("{other:?}"),
    }
    // One undo step puts the 4 m rectangle and d1 = 4 back.
    assert!(s.undo());
    assert_rect(
        &vertices(&s, rect),
        [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
    );
    assert_eq!(parameters(&mut s)[0].expression, "4");
}

#[test]
fn geometric_constraints_move_what_they_hold() {
    let mut doc = sample_doc();
    for (a, b) in [([5.0, 5.0], [9.0, 7.0]), ([5.0, 10.0], [9.0, 10.5])] {
        doc.add_entity(EntityType::Line(Line::from_coords(
            a[0], a[1], 0.0, b[0], b[1], 0.0,
        )))
        .unwrap();
    }
    let (_dir, mut s) = open(&doc, "plan.dxf");
    let raised: Vec<u64> = s
        .drawing
        .doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Line(l) if l.start.y > 4.0 => Some(l.common.handle.value()),
            _ => None,
        })
        .collect();
    let (slanted, upper) = (raised[0], raised[1]);
    let rect = rectangle(&s);
    // Horizontal: the line turns level, moving as little as it can.
    s.run(Command::AddConstraint {
        kind: ConstraintKind::Horizontal,
        targets: vec![t(slanted)],
        points: vec![[7.0, 6.0]],
    })
    .unwrap();
    let [a, b] = line_ends(&s, slanted);
    assert!(
        (a[1] - 6.0).abs() < TOL && (b[1] - 6.0).abs() < TOL,
        "{a:?} {b:?}"
    );
    // Perpendicular to the rectangle's right side, picked first: the side stays, the line turns.
    s.run(Command::AddConstraint {
        kind: ConstraintKind::Perpendicular,
        targets: vec![t(rect), t(upper)],
        points: vec![[4.0, 1.5], [7.0, 10.25]],
    })
    .unwrap();
    let [a, b] = line_ends(&s, upper);
    assert!((a[1] - b[1]).abs() < TOL, "{a:?} {b:?}");
    // Equal to the rectangle's bottom (4 m): the level line takes its length.
    s.run(Command::AddConstraint {
        kind: ConstraintKind::Equal,
        targets: vec![t(rect), t(slanted)],
        points: vec![[2.0, 0.0], [7.0, 6.0]],
    })
    .unwrap();
    let [a, b] = line_ends(&s, slanted);
    assert!(
        ((b[0] - a[0]).hypot(b[1] - a[1]) - 4.0).abs() < TOL,
        "{a:?} {b:?}"
    );
    assert!((a[1] - b[1]).abs() < TOL, "still level");
    assert_rect(
        &vertices(&s, rect),
        [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
    );
    // The same constraint twice is refused; Tangent needs something round.
    assert!(s
        .run(Command::AddConstraint {
            kind: ConstraintKind::Horizontal,
            targets: vec![t(slanted)],
            points: vec![[7.0, 6.0]],
        })
        .is_err());
    assert!(s
        .run(Command::AddConstraint {
            kind: ConstraintKind::Tangent,
            targets: vec![t(slanted), t(upper)],
            points: vec![[7.0, 6.0], [7.0, 10.25]],
        })
        .is_err());
}

#[test]
fn a_move_solves_again_in_the_same_step_and_an_erase_takes_the_constraints_along() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let (rect, line) = (rectangle(&s), wall_line(&s));
    // The line's start sits on the rectangle's first corner.
    s.run(Command::AddConstraint {
        kind: ConstraintKind::Coincident,
        targets: vec![t(rect), t(line)],
        points: vec![[0.0, 0.0], [0.0, 0.0]],
    })
    .unwrap();
    s.run(Command::Modify(ModifySpec::Move {
        targets: vec![t(rect)],
        delta: [1.0, 2.0],
    }))
    .unwrap();
    assert_rect(
        &vertices(&s, rect),
        [[1.0, 2.0], [5.0, 2.0], [5.0, 5.0], [1.0, 5.0]],
    );
    let [a, b] = line_ends(&s, line);
    assert!(near(a, [1.0, 2.0]), "the line's start followed: {a:?}");
    assert!(near(b, [10.0, 0.0]), "its end stays: {b:?}");
    // One step undoes both.
    assert!(s.undo());
    assert!(near(line_ends(&s, line)[0], [0.0, 0.0]));
    assert_rect(
        &vertices(&s, rect),
        [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
    );

    // Erasing the rectangle takes its constraints and its dimensional constraint's Dimension.
    width(&mut s, rect, None);
    assert_eq!(constraints(&mut s).len(), 2);
    let dim = parameters(&mut s)[0].dimension;
    s.run(Command::DeleteEntities {
        targets: vec![t(rect)],
    })
    .unwrap();
    assert!(constraints(&mut s).is_empty());
    assert!(parameters(&mut s).is_empty());
    assert!(s.drawing.doc.get_entity(Handle::new(dim)).is_none());
    // The line keeps no stale record.
    let e = s.drawing.doc.get_entity(Handle::new(line)).unwrap();
    assert!(e.common().extended_data.get_record(PARAM_APP).is_none());
    assert!(s.undo());
    assert_eq!(constraints(&mut s).len(), 2);
}

#[test]
fn a_fixed_point_refuses_a_move_that_breaks_it() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let rect = rectangle(&s);
    s.run(Command::AddConstraint {
        kind: ConstraintKind::Fixed,
        targets: vec![t(rect)],
        points: vec![[0.1, 0.1]],
    })
    .unwrap();
    let err = s
        .run(Command::Modify(ModifySpec::Move {
            targets: vec![t(rect)],
            delta: [1.0, 0.0],
        }))
        .unwrap_err();
    assert!(err.to_string().contains("constraints"), "{err}");
    assert_rect(
        &vertices(&s, rect),
        [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
    );
}

#[test]
fn a_parameter_follows_the_ones_it_uses_and_circles_are_refused() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let rect = rectangle(&s);
    auto(&mut s, rect);
    width(&mut s, rect, None);
    height(&mut s, rect);
    let names: Vec<String> = parameters(&mut s).into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["d1", "d2"]);

    s.run(Command::SetParameter {
        name: "d1".into(),
        expression: "2*d2 + 1.5".into(),
    })
    .unwrap();
    assert_rect(
        &vertices(&s, rect),
        [[0.0, 0.0], [7.5, 0.0], [7.5, 3.0], [0.0, 3.0]],
    );
    // d2 changes: d1 follows it.
    s.run(Command::SetParameter {
        name: "d2".into(),
        expression: "4".into(),
    })
    .unwrap();
    let v = vertices(&s, rect);
    let (w, h) = (v[1][0] - v[0][0], v[3][1] - v[0][1]);
    assert!((h - 4.0).abs() < TOL && (w - 9.5).abs() < TOL, "{v:?}");
    let d1 = parameters(&mut s)
        .into_iter()
        .find(|p| p.name == "d1")
        .unwrap();
    assert!((d1.value.unwrap() - 9.5).abs() < TOL);
    match s.drawing.doc.get_entity(Handle::new(d1.dimension)) {
        Some(EntityType::Dimension(d)) => {
            assert_eq!(d.base().user_text.as_deref(), Some("[fx: d1=<>]"))
        }
        other => panic!("{other:?}"),
    }

    // A circle is refused and nothing changes.
    let before = vertices(&s, rect);
    let err = s
        .run(Command::SetParameter {
            name: "d2".into(),
            expression: "d1 - 1".into(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("circle"), "{err}");
    assert_eq!(vertices(&s, rect), before);
    let err = s
        .run(Command::SetParameter {
            name: "d2".into(),
            expression: "d9 * 2".into(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("d9"), "{err}");

    // Deleting d2 leaves d1 its value as a number.
    let d2 = parameters(&mut s)
        .into_iter()
        .find(|p| p.name == "d2")
        .unwrap();
    s.run(Command::DeleteConstraint { id: d2.id }).unwrap();
    let left = parameters(&mut s);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].expression, "9.5");
}

#[test]
fn constraints_survive_a_dwg_and_a_dxf_round_trip() {
    for ext in ["dwg", "dxf"] {
        let (dir, mut s) = open(&sample_doc(), &format!("plan.{ext}"));
        let rect = rectangle(&s);
        auto(&mut s, rect);
        width(&mut s, rect, Some("5"));
        assert_rect(
            &vertices(&s, rect),
            [[0.0, 0.0], [5.0, 0.0], [5.0, 3.0], [0.0, 3.0]],
        );
        let before = constraints(&mut s);
        let path = dir.path().join(format!("again.{ext}"));
        s.save_as(&path).unwrap();
        let mut again = Session::open(&path).unwrap();
        let after = constraints(&mut again);
        assert_eq!(after.len(), before.len(), "{ext}: {after:?}");
        for (a, b) in after.iter().zip(&before) {
            assert_eq!((a.id, a.kind), (b.id, b.kind), "{ext}");
            assert_eq!(a.name, b.name, "{ext}");
            assert_eq!(a.expression, b.expression, "{ext}");
        }
        // And they still work there.
        let rect = rectangle(&again);
        again
            .run(Command::SetParameter {
                name: "d1".into(),
                expression: "3.5".into(),
            })
            .unwrap();
        assert_rect(
            &vertices(&again, rect),
            [[0.0, 0.0], [3.5, 0.0], [3.5, 3.0], [0.0, 3.0]],
        );
    }
}

#[test]
fn what_constraints_refuse() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let text = find(&s, |e| matches!(e, EntityType::Text(_)));
    let err = s
        .run(Command::AddConstraint {
            kind: ConstraintKind::Fixed,
            targets: vec![t(text)],
            points: vec![[0.0, -2.0]],
        })
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("lines, polylines, arcs and circles"),
        "{err}"
    );
    let rect = rectangle(&s);
    let inside = Target {
        handle: rect,
        chain: vec![rect],
    };
    assert!(s
        .run(Command::AddConstraint {
            kind: ConstraintKind::Horizontal,
            targets: vec![inside],
            points: vec![[2.0, 0.0]],
        })
        .is_err());
    assert!(s
        .run(Command::AddConstraint {
            kind: ConstraintKind::Linear,
            targets: vec![t(rect)],
            points: vec![[2.0, 0.0]],
        })
        .is_err());
    // A circle's radius, and an angle between two lines.
    let arc = find(&s, |e| matches!(e, EntityType::Arc(_)));
    let id = ids(s
        .run(Command::AddDimConstraint {
            kind: ConstraintKind::Radius,
            targets: vec![t(arc)],
            points: vec![[20.0, 2.0]],
            line_point: [21.0, 3.0],
            expression: Some("2.5".into()),
        })
        .unwrap())[0];
    let p = parameters(&mut s);
    assert_eq!((p[0].id, p[0].name.as_str()), (id, "rad1"));
    assert!((p[0].value.unwrap() - 2.5).abs() < TOL);
    assert!(constraints(&mut s).iter().all(|c| c.at[0].is_finite()));
}

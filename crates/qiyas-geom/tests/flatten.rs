use std::f64::consts::{FRAC_PI_2, PI};

use std::time::{Duration, Instant};

use opencadcodec::entities::{
    BoundaryEdge, BoundaryPath, CircularArcEdge, EntityType, Hatch, Insert, LineEdge, LwPolyline,
};
use opencadcodec::tables::{BlockRecord, TableEntry};
use opencadcodec::{CadDocument, Circle, Line, Vector2, Vector3};
use qiyas_geom::{flatten, Path, Xform};

fn flat(doc: &CadDocument, entity: EntityType, tol: f64) -> Vec<Path> {
    let mut out = Vec::new();
    flatten(doc, &entity, &Xform::identity(), tol, &mut out);
    out
}

fn closed_lwpolyline(points: &[(f64, f64, f64)]) -> EntityType {
    let mut pl = LwPolyline::new();
    for &(x, y, bulge) in points {
        pl.add_point_with_bulge(Vector2::new(x, y), bulge);
    }
    pl.is_closed = true;
    EntityType::LwPolyline(pl)
}

fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> EntityType {
    EntityType::Line(Line::from_points(
        Vector3::new(x1, y1, 0.0),
        Vector3::new(x2, y2, 0.0),
    ))
}

fn assert_points(actual: &[[f64; 2]], expected: &[[f64; 2]]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a[0] - e[0]).abs() < 1e-9 && (a[1] - e[1]).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }
}

/// Adds a block definition named `name` holding `entities`.
fn add_block(doc: &mut CadDocument, name: &str, entities: Vec<EntityType>) {
    add_block_at(doc, name, Vector3::new(0.0, 0.0, 0.0), entities);
}

/// Adds a block definition with the given base point.
fn add_block_at(doc: &mut CadDocument, name: &str, base: Vector3, entities: Vec<EntityType>) {
    let mut record = BlockRecord::new(name);
    record.set_handle(doc.allocate_handle());
    record.base_point = base;
    let owner = record.handle;
    doc.block_records.add(record).unwrap();
    for mut entity in entities {
        entity.common_mut().owner_handle = owner;
        doc.add_entity(entity).unwrap();
    }
}

#[test]
fn rectangle_area_and_perimeter() {
    let doc = CadDocument::new();
    let rect = closed_lwpolyline(&[
        (0.0, 0.0, 0.0),
        (4.0, 0.0, 0.0),
        (4.0, 3.0, 0.0),
        (0.0, 3.0, 0.0),
    ]);
    let paths = flat(&doc, rect, 1e-3);
    assert_eq!(paths.len(), 1);
    assert!(paths[0].closed);
    assert!((paths[0].area() - 12.0).abs() < 1e-9);
    assert!((paths[0].length() - 14.0).abs() < 1e-9);
}

#[test]
fn circle_area_converges() {
    let doc = CadDocument::new();
    let circle = Circle::from_center_radius(Vector3::new(0.0, 0.0, 0.0), 1.0);
    let paths = flat(&doc, EntityType::Circle(circle), 1e-4);
    assert_eq!(paths.len(), 1);
    assert!(paths[0].closed);
    assert!((paths[0].area() - PI).abs() < 1e-3);
    assert!((paths[0].length() - 2.0 * PI).abs() < 1e-3);
}

#[test]
fn bulge_semicircle() {
    let doc = CadDocument::new();
    let half = closed_lwpolyline(&[(0.0, 0.0, 1.0), (2.0, 0.0, 0.0)]);
    let paths = flat(&doc, half, 1e-4);
    assert_eq!(paths.len(), 1);
    assert!((paths[0].area() - FRAC_PI_2).abs() < 1e-3);
}

#[test]
fn nested_scaled_rotated_insert() {
    let mut doc = CadDocument::new();
    let line = Line::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
    add_block(&mut doc, "A", vec![EntityType::Line(line)]);
    let a = Insert::new("A", Vector3::new(0.0, 0.0, 0.0)).with_scale(2.0, 2.0, 2.0);
    add_block(&mut doc, "B", vec![EntityType::Insert(a)]);

    let mut b = Insert::new("B", Vector3::new(5.0, 5.0, 0.0)).with_scale(3.0, 3.0, 3.0);
    b.rotation = FRAC_PI_2;
    let paths = flat(&doc, EntityType::Insert(b), 1e-3);

    assert_eq!(paths.len(), 1);
    let p = &paths[0];
    assert!((p.length() - 6.0).abs() < 1e-9);
    let (start, end) = (p.points[0], p.points[p.points.len() - 1]);
    assert!((start[0] - 5.0).abs() < 1e-9 && (start[1] - 5.0).abs() < 1e-9);
    assert!((end[0] - 5.0).abs() < 1e-9 && (end[1] - 11.0).abs() < 1e-9);
}

#[test]
fn self_intersecting_bowtie() {
    let bowtie = Path {
        points: vec![[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]],
        closed: true,
    };
    assert!(bowtie.self_intersects());
    assert!((bowtie.area() - 2.0).abs() < 1e-9);
}

#[test]
fn open_path_has_zero_area() {
    let doc = CadDocument::new();
    let line = Line::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(3.0, 4.0, 0.0));
    let paths = flat(&doc, EntityType::Line(line), 1e-3);
    assert_eq!(paths.len(), 1);
    assert!(!paths[0].closed);
    assert_eq!(paths[0].area(), 0.0);
}

#[test]
fn negative_bulge_turns_clockwise() {
    let doc = CadDocument::new();
    let half = closed_lwpolyline(&[(0.0, 0.0, -1.0), (2.0, 0.0, 0.0)]);
    let paths = flat(&doc, half, 1e-4);
    assert!((paths[0].area() - FRAC_PI_2).abs() < 1e-3);
    // Going left to right clockwise, the arc rises above the chord.
    let [lo, hi] = paths[0].bounds().unwrap();
    assert!(lo[1].abs() < 1e-9 && (hi[1] - 1.0).abs() < 1e-3);
}

#[test]
fn near_zero_bulge_is_straight() {
    let doc = CadDocument::new();
    let mut pl = LwPolyline::new();
    pl.add_point_with_bulge(Vector2::new(0.0, 0.0), 1e-15);
    pl.add_point_with_bulge(Vector2::new(2.0, 0.0), 0.0);
    let paths = flat(&doc, EntityType::LwPolyline(pl), 1e-4);
    assert_points(&paths[0].points, &[[0.0, 0.0], [2.0, 0.0]]);
}

#[test]
fn block_base_point_is_subtracted() {
    let mut doc = CadDocument::new();
    add_block_at(
        &mut doc,
        "D",
        Vector3::new(10.0, 0.0, 0.0),
        vec![line(10.0, 0.0, 11.0, 0.0)],
    );
    let insert = Insert::new("D", Vector3::new(100.0, 0.0, 0.0)).with_scale(2.0, 2.0, 2.0);
    let paths = flat(&doc, EntityType::Insert(insert), 1e-3);
    assert_eq!(paths.len(), 1);
    assert_points(&paths[0].points, &[[100.0, 0.0], [102.0, 0.0]]);
}

#[test]
fn rotated_minsert_offsets_rotate_with_insert() {
    let mut doc = CadDocument::new();
    add_block(&mut doc, "D", vec![line(0.0, 0.0, 1.0, 0.0)]);
    let mut insert = Insert::new("D", Vector3::new(100.0, 0.0, 0.0));
    insert.rotation = FRAC_PI_2;
    insert.column_count = 2;
    insert.row_count = 2;
    insert.column_spacing = 5.0;
    insert.row_spacing = 7.0;
    let paths = flat(&doc, EntityType::Insert(insert), 1e-3);
    // Columns step along the rotated x axis (+y), rows along the rotated y axis (-x).
    let expected = [
        [[100.0, 0.0], [100.0, 1.0]],
        [[93.0, 0.0], [93.0, 1.0]],
        [[100.0, 5.0], [100.0, 6.0]],
        [[93.0, 5.0], [93.0, 6.0]],
    ];
    assert_eq!(paths.len(), 4);
    for (path, want) in paths.iter().zip(expected) {
        assert_points(&path.points, &want);
    }
}

#[test]
fn clockwise_hatch_arc_edge() {
    let doc = CadDocument::new();
    let mut boundary = BoundaryPath::new();
    boundary.add_edge(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(0.0, 0.0),
        radius: 1.0,
        start_angle: 0.0,
        end_angle: PI,
        counter_clockwise: false,
    }));
    boundary.add_edge(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(-1.0, 0.0),
        end: Vector2::new(1.0, 0.0),
    }));
    let mut hatch = Hatch::new();
    hatch.paths.push(boundary);
    let paths = flat(&doc, EntityType::Hatch(hatch), 1e-4);
    assert_eq!(paths.len(), 1);
    assert!(paths[0].closed);
    assert!((paths[0].area() - FRAC_PI_2).abs() < 1e-3);
    // Clockwise from angle 0 to PI runs through the lower half.
    let [lo, hi] = paths[0].bounds().unwrap();
    assert!((lo[1] + 1.0).abs() < 1e-3 && hi[1].abs() < 1e-9);
}

#[test]
fn duplicate_vertex_is_not_a_self_intersection() {
    let rect = Path {
        points: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
        closed: true,
    };
    assert!(!rect.self_intersects());
    assert!((rect.area() - 12.0).abs() < 1e-9);
}

#[test]
fn self_referencing_block_stops_at_depth_limit() {
    let mut doc = CadDocument::new();
    let inner = Insert::new("X", Vector3::new(1.0, 0.0, 0.0));
    add_block(
        &mut doc,
        "X",
        vec![line(0.0, 0.0, 1.0, 0.0), EntityType::Insert(inner)],
    );
    let start = Instant::now();
    let paths = flat(
        &doc,
        EntityType::Insert(Insert::new("X", Vector3::new(0.0, 0.0, 0.0))),
        1e-3,
    );
    // Inserts at depths 0 to 31 expand, each adding one line.
    assert_eq!(paths.len(), 32);
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn self_referencing_minsert_stops_at_work_budget() {
    let mut doc = CadDocument::new();
    let mut inner = Insert::new("X", Vector3::new(1.0, 0.0, 0.0));
    inner.column_count = 2;
    inner.row_count = 2;
    add_block(
        &mut doc,
        "X",
        vec![line(0.0, 0.0, 1.0, 0.0), EntityType::Insert(inner)],
    );
    // Without a budget this would expand 4^32 instances.
    let paths = flat(
        &doc,
        EntityType::Insert(Insert::new("X", Vector3::new(0.0, 0.0, 0.0))),
        1e-3,
    );
    assert!(!paths.is_empty() && paths.len() <= 1_000_000);
}

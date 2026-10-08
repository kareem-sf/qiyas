use std::f64::consts::{FRAC_PI_2, PI};

use opencadcodec::entities::{EntityType, Insert, LwPolyline};
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

/// Adds a block definition named `name` holding `entities`.
fn add_block(doc: &mut CadDocument, name: &str, entities: Vec<EntityType>) {
    let mut record = BlockRecord::new(name);
    record.set_handle(doc.allocate_handle());
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

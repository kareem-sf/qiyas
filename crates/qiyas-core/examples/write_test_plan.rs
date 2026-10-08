//! `cargo run -p qiyas-core --example write_test_plan -- <out.dwg>`: writes a small apartment plan
//! with known quantities, then reopens it through the core and prints what Qiyas measures.
//! Units are metres. Expected numbers are in `samples/test-plan.md`.

use std::f64::consts::PI;
use std::path::Path;

use opencadcodec::entities::Insert;
use opencadcodec::{
    Arc, BlockRecord, CadDocument, Circle, Color, DwgWriter, EntityType, Layer, Line, LwPolyline,
    Text, Vector2, Vector3,
};
use qiyas_core::{Command, Outcome, Session};

fn layer(doc: &mut CadDocument, name: &str, index: u8) {
    let mut layer = Layer::new(name);
    layer.handle = doc.allocate_handle();
    layer.color = Color::Index(index);
    doc.layers.add(layer).unwrap();
}

fn block(doc: &mut CadDocument, name: &str, entities: Vec<EntityType>) {
    let mut record = BlockRecord::new(name);
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let owner = record.handle;
    doc.block_records.add(record).unwrap();
    for mut entity in entities {
        entity.common_mut().owner_handle = owner;
        doc.add_entity(entity).unwrap();
    }
}

fn closed(points: &[(f64, f64)]) -> LwPolyline {
    let mut polyline = LwPolyline::new();
    for &(x, y) in points {
        polyline.add_point(Vector2::new(x, y));
    }
    polyline.is_closed = true;
    polyline
}

fn add(doc: &mut CadDocument, layer: &str, mut entity: EntityType) {
    entity.common_mut().layer = layer.to_string();
    doc.add_entity(entity).unwrap();
}

fn build() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = 6;
    for (name, index) in [
        ("A-WALL", 1),
        ("A-ROOM", 3),
        ("A-DOOR", 2),
        ("A-WINDOW", 4),
        ("S-COLUMN", 5),
        ("A-TEXT", 7),
    ] {
        layer(&mut doc, name, index);
    }

    // Blocks: a door leaf (0.9 m) with its swing, a window line (1.2 m), a column 0.4 x 0.4.
    block(
        &mut doc,
        "DOOR",
        vec![
            EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 0.9, 0.0, 0.0)),
            EntityType::Arc(Arc::from_center_radius_angles(
                Vector3::new(0.0, 0.0, 0.0),
                0.9,
                0.0,
                PI / 2.0,
            )),
        ],
    );
    block(
        &mut doc,
        "WINDOW",
        vec![EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 1.2, 0.0, 0.0,
        ))],
    );
    block(
        &mut doc,
        "COLUMN",
        vec![EntityType::LwPolyline(closed(&[
            (-0.2, -0.2),
            (0.2, -0.2),
            (0.2, 0.2),
            (-0.2, 0.2),
        ]))],
    );

    // Outer walls 12 x 9 m, one partition at x = 7, one at y = 5 in the right part.
    add(
        &mut doc,
        "A-WALL",
        EntityType::LwPolyline(closed(&[(0.0, 0.0), (12.0, 0.0), (12.0, 9.0), (0.0, 9.0)])),
    );
    add(
        &mut doc,
        "A-WALL",
        EntityType::Line(Line::from_coords(7.0, 0.0, 0.0, 7.0, 9.0, 0.0)),
    );
    add(
        &mut doc,
        "A-WALL",
        EntityType::Line(Line::from_coords(7.0, 5.0, 0.0, 12.0, 5.0, 0.0)),
    );
    // A bay window arc on the top wall: centre (3.5, 9), radius 2, half circle outwards.
    add(
        &mut doc,
        "A-WALL",
        EntityType::Arc(Arc::from_center_radius_angles(
            Vector3::new(3.5, 9.0, 0.0),
            2.0,
            0.0,
            PI,
        )),
    );
    // A round feature: circle radius 1.5 on the floor (fountain).
    add(
        &mut doc,
        "A-ROOM",
        EntityType::Circle(Circle::from_center_radius(Vector3::new(3.5, 4.5, 0.0), 1.5)),
    );

    // Rooms as closed polylines, inset 0.15 m from the wall lines so nothing overlaps.
    add(
        &mut doc,
        "A-ROOM",
        EntityType::LwPolyline(closed(&[
            (0.15, 0.15),
            (6.85, 0.15),
            (6.85, 8.85),
            (0.15, 8.85),
        ])),
    );
    add(
        &mut doc,
        "A-ROOM",
        EntityType::LwPolyline(closed(&[
            (7.15, 0.15),
            (11.85, 0.15),
            (11.85, 4.85),
            (7.15, 4.85),
        ])),
    );
    add(
        &mut doc,
        "A-ROOM",
        EntityType::LwPolyline(closed(&[
            (7.15, 5.15),
            (11.85, 5.15),
            (11.85, 8.85),
            (7.15, 8.85),
        ])),
    );

    // 3 doors, 5 windows, 6 columns.
    for (x, y, rot) in [(3.0, 0.0, 0.0), (7.0, 2.0, PI / 2.0), (7.0, 7.0, PI / 2.0)] {
        add(
            &mut doc,
            "A-DOOR",
            EntityType::Insert(Insert::new("DOOR", Vector3::new(x, y, 0.0)).with_rotation(rot)),
        );
    }
    for (x, y, rot) in [
        (1.0, 9.0, 0.0),
        (9.0, 9.0, 0.0),
        (12.0, 2.0, PI / 2.0),
        (12.0, 7.0, PI / 2.0),
        (0.0, 4.0, PI / 2.0),
    ] {
        add(
            &mut doc,
            "A-WINDOW",
            EntityType::Insert(Insert::new("WINDOW", Vector3::new(x, y, 0.0)).with_rotation(rot)),
        );
    }
    for (x, y) in [
        (0.0, 0.0),
        (7.0, 0.0),
        (12.0, 0.0),
        (0.0, 9.0),
        (7.0, 9.0),
        (12.0, 9.0),
    ] {
        add(
            &mut doc,
            "S-COLUMN",
            EntityType::Insert(Insert::new("COLUMN", Vector3::new(x, y, 0.0))),
        );
    }

    for (text, x, y) in [
        ("LIVING", 3.0, 1.5),
        ("BEDROOM", 9.0, 2.0),
        ("KITCHEN", 9.0, 6.5),
        ("QIYAS TEST PLAN  1:100  UNITS M", 0.0, -1.5),
    ] {
        add(
            &mut doc,
            "A-TEXT",
            EntityType::Text(Text::with_value(text, Vector3::new(x, y, 0.0)).with_height(0.4)),
        );
    }
    doc
}

fn measure(session: &mut Session, label: &str, at: [f64; 2]) {
    let handle = session.pick(at, 0.02).expect(label);
    match session.run(Command::MeasureEntity { handle }).unwrap() {
        Outcome::Measured { measurement } => println!(
            "{label:<22} {:>10.3} {}",
            measurement.shown, measurement.unit
        ),
        other => println!("{label:<22} {other:?}"),
    }
}

fn count(session: &mut Session, name: &str) {
    match session
        .run(Command::CountBlock {
            block_name: name.to_string(),
            layer: None,
        })
        .unwrap()
    {
        Outcome::Measured { measurement } => {
            println!("{name:<22} {:>10} {}", measurement.shown, measurement.unit)
        }
        other => println!("{name:<22} {other:?}"),
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: write_test_plan <out.dwg>");
    let path = Path::new(&out);
    DwgWriter::write_to_file(path, &build()).unwrap();
    println!("wrote {}", path.display());

    let mut session = Session::open(path).unwrap();
    println!("problems: {:?}", session.drawing.problems);
    println!("\nWhat the core measures (click these spots in Qiyas):");
    measure(&mut session, "outer wall ring", [12.0, 4.0]);
    measure(&mut session, "partition x=7", [7.0, 4.0]);
    measure(&mut session, "partition y=5", [10.0, 5.0]);
    measure(&mut session, "bay arc", [3.5, 11.0]);
    measure(&mut session, "fountain circle", [5.0, 4.5]);
    measure(&mut session, "living room", [0.15, 4.0]);
    measure(&mut session, "bedroom", [11.85, 2.5]);
    measure(&mut session, "kitchen", [11.85, 7.5]);
    count(&mut session, "DOOR");
    count(&mut session, "WINDOW");
    count(&mut session, "COLUMN");
    let _ = std::fs::remove_file(path.with_extension("qiyas"));
}

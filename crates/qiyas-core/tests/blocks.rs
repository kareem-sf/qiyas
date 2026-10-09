//! Picking and measuring inside blocks, exact curves, dynamic blocks and over-deep blocks.

use std::f64::consts::{PI, TAU};

use opencadcodec::entities::Insert;
use opencadcodec::objects::{Dictionary, ObjectType};
use opencadcodec::{BlockRecord, CadDocument, Circle, EntityType, Handle, Line, Vector3};
use qiyas_core::{project_path, Command, CoreError, Kind, Measurement, Outcome, Picked, Session};

/// Opens the sample drawing (DXF, metres) in a session. The temp dir must outlive the test body.
fn sample_session() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let session = Session::open(&path).unwrap();
    (dir, session)
}

fn measure(session: &mut Session, cmd: Command) -> Measurement {
    match session.run(cmd).unwrap() {
        Outcome::Measured { measurement } => *measurement,
        other => panic!("expected a measurement, got {other:?}"),
    }
}

/// The sample's DOOR references (plain, rotated, scaled 2) and the leaf line inside DOOR.
fn doors(session: &Session) -> ([u64; 3], u64) {
    let inserts: Vec<u64> = session
        .drawing
        .entities_in("*Model_Space")
        .filter(|e| matches!(e, EntityType::Insert(_)))
        .map(|e| e.common().handle.value())
        .collect();
    let leaf = session
        .drawing
        .entities_in("DOOR")
        .next()
        .unwrap()
        .common()
        .handle
        .value();
    (inserts.try_into().unwrap(), leaf)
}

/// Adds a block definition named `name` holding `entities`; returns the record's handle.
fn add_block(doc: &mut CadDocument, name: &str, entities: Vec<EntityType>) -> Handle {
    let mut record = BlockRecord::new(name);
    record.handle = doc.allocate_handle();
    let owner = record.handle;
    doc.block_records.add(record).unwrap();
    for mut entity in entities {
        entity.common_mut().owner_handle = owner;
        doc.add_entity(entity).unwrap();
    }
    owner
}

#[test]
fn pick_inside_the_door_returns_the_leaf_line() {
    let (_dir, session) = sample_session();
    let ([_, _, scaled], leaf) = doors(&session);
    // The scaled DOOR at (9, 1): its leaf runs to (10.8, 1).
    let picked = session.pick([10.0, 1.0], 0.05).unwrap();
    assert_eq!(
        picked,
        Picked {
            top: scaled,
            entity: leaf,
            chain: vec![scaled],
            xref: None,
            kind: Some("Insert".to_string()),
        }
    );

    let info = session.entity_info(leaf, &picked.chain).unwrap();
    assert_eq!(info.kind, "Line");
    // On layer 0 inside the block, so drawn on the insert's layer.
    assert_eq!(info.layer, "A-DOOR");
    assert!((info.length.unwrap() - 1.8).abs() < 1e-9);
    assert_eq!(info.inside_block.as_deref(), Some("DOOR"));
    assert_eq!(info.chain, [scaled]);
    assert_eq!(info.block_path, ["DOOR"]);
    assert_eq!(info.block_name, None);

    // The whole block, as before.
    let block = session.entity_info(scaled, &[]).unwrap();
    assert_eq!(block.block_name.as_deref(), Some("DOOR"));
    assert_eq!(block.inside_block, None);
    // A block reference is counted: no length or area from the lines inside it.
    assert_eq!((block.length, block.area), (None, None));
}

#[test]
fn measure_entity_with_a_chain_measures_in_world_units() {
    let (_dir, mut session) = sample_session();
    let ([plain, rotated, scaled], leaf) = doors(&session);
    for (door, scale) in [(plain, 1.0), (rotated, 1.0), (scaled, 2.0)] {
        let m = measure(
            &mut session,
            Command::MeasureEntity {
                handle: leaf,
                chain: vec![door],
            },
        );
        assert_eq!(m.kind, Kind::Length);
        assert!((m.value - 0.9 * scale).abs() < 1e-9, "{}", m.value);
        assert_eq!(m.description, "Line on A-DOOR");
        assert_eq!(m.layer, "A-DOOR");
        assert_eq!(m.handles, [leaf]);
        assert_eq!(m.chain, [door]);
        assert_eq!(m.block_path, ["DOOR"]);
        assert!(m.auto_description);
    }

    // A chain that does not lead to the entity is refused.
    let line = session
        .drawing
        .entities_in("*Model_Space")
        .find(|e| matches!(e, EntityType::Line(_)))
        .unwrap()
        .common()
        .handle
        .value();
    for (handle, chain) in [(line, vec![plain]), (leaf, vec![line])] {
        assert!(matches!(
            session.run(Command::MeasureEntity { handle, chain }),
            Err(CoreError::InvalidCommand(_))
        ));
    }
}

#[test]
fn arcs_and_circles_measure_exactly() {
    let (_dir, mut session) = sample_session();
    let circle = Circle::from_center_radius(Vector3::new(30.0, 0.0, 0.0), 1.0);
    let circle = session
        .drawing
        .doc
        .add_entity(EntityType::Circle(circle))
        .unwrap()
        .value();
    let arc = session
        .drawing
        .entities_in("*Model_Space")
        .find(|e| matches!(e, EntityType::Arc(_)))
        .unwrap()
        .common()
        .handle
        .value();

    // The half circle of radius 2: 2π, not the 6.282 of its chords.
    let m = measure(
        &mut session,
        Command::MeasureEntity {
            handle: arc,
            chain: Vec::new(),
        },
    );
    assert!((m.value - 2.0 * PI).abs() < 1e-9, "{}", m.value);

    let info = session.entity_info(circle, &[]).unwrap();
    assert!((info.length.unwrap() - TAU).abs() < 1e-9);
    assert!((info.area.unwrap() - PI).abs() < 1e-9);
}

#[test]
fn chain_survives_a_trip_through_the_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let source = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let mut session = Session::open(&source).unwrap();
    let ([_, _, scaled], leaf) = doors(&session);
    session
        .run(Command::MeasureEntity {
            handle: leaf,
            chain: vec![scaled],
        })
        .unwrap();
    let out = dir.path().join("out.dwg");
    session.save_as(&out).unwrap();
    std::fs::remove_file(project_path(&out)).unwrap();

    let items = Session::open(&out).unwrap().takeoff.items;
    assert_eq!(items.len(), 1);
    let m = &items[0];
    assert_eq!(m.chain, [scaled]);
    assert_eq!(m.block_path, ["DOOR"]);
    assert!((m.value - 1.8).abs() < 1e-9);
    // Drawn where the scaled DOOR places the leaf.
    assert_eq!(m.points, [[9.0, 1.0], [10.8, 1.0]]);
    // "Line on A-DOOR" is a default name.
    assert!(m.auto_description);
}

#[test]
fn dynamic_block_copies_count_as_their_block() {
    let (_dir, mut session) = sample_session();
    let doc = &mut session.drawing.doc;
    let definition = add_block(doc, "DOOR-DYN", Vec::new());
    let leaf = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    add_block(doc, "*U7", vec![leaf]);

    // The anonymous copy's insert links back to DOOR-DYN through its extension dictionary.
    let mut insert = Insert::new("*U7", Vector3::new(40.0, 0.0, 0.0));
    insert.common.layer = "A-DOOR".to_string();
    let xdict = doc.allocate_handle();
    insert.common.xdictionary_handle = Some(xdict);
    let handle = doc.add_entity(EntityType::Insert(insert)).unwrap();
    let representation = doc.allocate_handle();
    for (h, owner) in [(xdict, handle), (representation, xdict)] {
        let mut dictionary = Dictionary::new();
        dictionary.handle = h;
        dictionary.owner = owner;
        doc.objects.insert(h, ObjectType::Dictionary(dictionary));
    }
    doc.block_representations.insert(representation, definition);
    let handle = handle.value();

    let info = session.entity_info(handle, &[]).unwrap();
    assert_eq!(info.block_name.as_deref(), Some("DOOR-DYN"));
    let summary = session.block_summary(None);
    assert!(
        summary
            .iter()
            .any(|b| b.block_name == "DOOR-DYN" && b.count == 1),
        "{summary:?}"
    );
    let m = measure(
        &mut session,
        Command::CountBlock {
            block_name: "DOOR-DYN".to_string(),
            layer: None,
        },
    );
    assert_eq!(m.handles, [handle]);
}

#[test]
fn too_deep_blocks_are_reported() {
    let (_dir, session) = sample_session();
    assert!(session.info().problems.is_empty());

    let (_dir, mut session) = sample_session();
    let doc = &mut session.drawing.doc;
    let inner = Insert::new("LOOP", Vector3::new(1.0, 0.0, 0.0));
    let line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    add_block(doc, "LOOP", vec![line, EntityType::Insert(inner)]);
    let top = Insert::new("LOOP", Vector3::new(0.0, 50.0, 0.0));
    doc.add_entity(EntityType::Insert(top)).unwrap();
    assert_eq!(
        session.info().problems,
        ["Drawing too complex, some block content not drawn"]
    );
}

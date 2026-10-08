use std::path::Path;

use opencadcodec::{Color, EntityType};
use qiyas_core::{project_path, Command, CoreError, Kind, Session};

/// Measures a distance, an area and the three DOOR references in `session`.
fn measure_sample(session: &mut Session) {
    for cmd in [
        Command::MeasureDistance {
            points: vec![[0.0, 0.0], [3.0, 0.0], [3.0, 4.0]],
        },
        Command::MeasureArea {
            points: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
        },
        Command::CountBlock {
            block_name: "DOOR".to_string(),
            layer: None,
        },
    ] {
        session.run(cmd).unwrap();
    }
}

/// Saves the sample (written as `source_ext`) with its measurements to `out_name`, deletes the
/// project file, reopens the saved drawing and checks the take-off was rebuilt from it.
fn round_trip(source_ext: &str, out_name: &str) {
    let dir = tempfile::tempdir().unwrap();
    let source = qiyas_core::fixtures::write_sample(dir.path(), source_ext);
    let mut session = Session::open(&source).unwrap();
    measure_sample(&mut session);
    let out = dir.path().join(out_name);
    session.save_as(&out).unwrap();
    assert_eq!(session.drawing.path, out);
    assert!(project_path(&out).exists());

    std::fs::remove_file(project_path(&out)).unwrap();
    let reopened = Session::open(&out).unwrap();
    let before = &session.takeoff.items;
    let after = &reopened.takeoff.items;
    assert_eq!(after.len(), 3, "{after:?}");
    for (b, a) in before.iter().zip(after) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.kind, b.kind);
        assert!(
            (a.value - b.value).abs() < 1e-6,
            "{} vs {}",
            a.value,
            b.value
        );
        assert!((a.shown - b.shown).abs() < 1e-6);
        assert_eq!(a.unit, b.unit);
        assert_eq!(a.description, b.description);
        assert_eq!(a.layer, b.layer);
        assert_eq!(a.sheet, b.sheet);
        assert_eq!(a.block_name, b.block_name);
        assert_eq!(a.source, "user");
        assert!(a.handles.is_empty());
    }
    assert_eq!(after[0].points, before[0].points);
    assert_eq!(after[1].points, before[1].points);
    assert_eq!(after[2].kind, Kind::Count);
    assert_eq!(after[2].value, 3.0);

    let layers = &reopened.drawing.doc.layers;
    for (name, index) in [("QIYAS-LENGTH", 6), ("QIYAS-AREA", 5), ("QIYAS-COUNT", 4)] {
        let layer = layers.get(name).unwrap_or_else(|| panic!("{name} missing"));
        assert_eq!(layer.color, Color::Index(index), "{name}");
    }
}

#[test]
fn save_dwg_reopen_without_project_rebuilds_measurements() {
    round_trip("dxf", "out.dwg");
}

#[test]
fn save_dxf_from_dwg_too() {
    round_trip("dwg", "out.dxf");
}

/// Entities in the saved file that carry a QIYAS xdata record.
fn embedded_count(path: &Path) -> usize {
    let session = Session::open(path).unwrap();
    session
        .drawing
        .doc
        .entities()
        .filter(|e| e.common().extended_data.get_record("QIYAS").is_some())
        .count()
}

#[test]
fn saving_twice_does_not_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let source = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let mut session = Session::open(&source).unwrap();
    measure_sample(&mut session);
    let out = dir.path().join("out.dwg");
    session.save_as(&out).unwrap();
    session.save().unwrap();
    // A distance polyline, an area polyline and one point per door.
    assert_eq!(embedded_count(&out), 5);
    let kinds: Vec<_> = Session::open(&out)
        .unwrap()
        .drawing
        .doc
        .entities()
        .filter(|e| e.common().extended_data.get_record("QIYAS").is_some())
        .map(|e| match e {
            EntityType::LwPolyline(p) => (e.common().layer.clone(), p.is_closed),
            _ => (e.common().layer.clone(), false),
        })
        .collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|k| *k == &("QIYAS-COUNT".to_string(), false))
            .count(),
        3
    );
    assert!(kinds.contains(&("QIYAS-LENGTH".to_string(), false)));
    assert!(kinds.contains(&("QIYAS-AREA".to_string(), true)));

    // Reopened and saved again (the project file wins): still no duplicates.
    let mut reopened = Session::open(&out).unwrap();
    assert_eq!(reopened.takeoff.items.len(), 3);
    reopened.save().unwrap();
    assert_eq!(embedded_count(&out), 5);
}

#[test]
fn project_file_wins_over_embedded_measurements() {
    let dir = tempfile::tempdir().unwrap();
    let source = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let mut session = Session::open(&source).unwrap();
    measure_sample(&mut session);
    let out = dir.path().join("out.dxf");
    session.save_as(&out).unwrap();
    let id = session.takeoff.items[0].id.clone();
    session.run(Command::RemoveMeasurement { id }).unwrap();
    session.save_project().unwrap();
    assert_eq!(Session::open(&out).unwrap().takeoff.items.len(), 2);
}

#[test]
fn save_as_other_formats_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let source = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let mut session = Session::open(&source).unwrap();
    let out = dir.path().join("out.pdf");
    assert!(matches!(
        session.save_as(&out),
        Err(CoreError::InvalidCommand(_))
    ));
    assert!(!out.exists());
    assert_eq!(session.drawing.path, source);
}

#[test]
fn measured_entities_are_embedded_from_their_shape() {
    let dir = tempfile::tempdir().unwrap();
    let source = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let mut session = Session::open(&source).unwrap();
    let handle_of = |session: &Session, is: fn(&EntityType) -> bool| {
        session
            .drawing
            .entities_in("*Model_Space")
            .find(|e| is(e))
            .unwrap()
            .common()
            .handle
            .value()
    };
    let rect = handle_of(&session, |e| matches!(e, EntityType::LwPolyline(_)));
    let line = handle_of(&session, |e| matches!(e, EntityType::Line(_)));
    let door = handle_of(&session, |e| matches!(e, EntityType::Insert(_)));
    for handle in [rect, line, door] {
        session.run(Command::MeasureEntity { handle }).unwrap();
    }
    let out = dir.path().join("out.dwg");
    session.save_as(&out).unwrap();
    std::fs::remove_file(project_path(&out)).unwrap();

    let items = Session::open(&out).unwrap().takeoff.items;
    let summary: Vec<_> = items.iter().map(|m| (m.kind, m.value)).collect();
    assert_eq!(
        summary,
        [(Kind::Area, 12.0), (Kind::Length, 10.0), (Kind::Count, 1.0)]
    );
    assert_eq!(
        items[0].points,
        [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]
    );
    assert_eq!(items[1].points, [[0.0, 0.0], [10.0, 0.0]]);
    assert_eq!(items[0].layer, "A-WALL");
    assert_eq!(items[2].block_name.as_deref(), Some("DOOR"));
}

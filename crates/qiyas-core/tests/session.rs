use qiyas_core::{CoreError, Session};

/// Opens the sample drawing (DXF) in a session. The temp dir must outlive the test body.
fn sample_session() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let session = Session::open(&path).unwrap();
    (dir, session)
}

#[test]
fn session_reports_info_and_switches_sheet() {
    let (_dir, mut session) = sample_session();
    let info = session.info();
    assert!(info.path.ends_with("sample.dxf"), "{}", info.path);
    assert_eq!(info.units, "metres");
    assert_eq!(info.sheets[0].block, "*Model_Space");
    let layers: Vec<_> = info.layers.iter().map(|l| l.name.as_str()).collect();
    assert!(
        layers.contains(&"A-WALL") && layers.contains(&"A-DOOR"),
        "{layers:?}"
    );
    assert_eq!(session.sheet, "*Model_Space");

    session.set_sheet("*Model_Space").unwrap();
    assert!(
        matches!(session.set_sheet("NOPE"), Err(CoreError::NoSuchSheet(name)) if name == "NOPE")
    );
    assert_eq!(session.sheet, "*Model_Space");
}

#[test]
fn hiding_a_layer_changes_the_batch() {
    let (_dir, mut session) = sample_session();
    assert_eq!(session.batch().ranges.len(), 7);

    session.set_layer_hidden("A-WALL", true);
    assert_eq!(session.batch().ranges.len(), 4);
    assert_eq!(session.hidden_layers(), ["A-WALL"]);
    // The rectangle is gone, so nothing is picked on its edge.
    assert_eq!(session.pick([4.0, 1.5], 0.05), None);

    // Layer names match case-insensitively, as in DXF.
    session.set_layer_hidden("a-wall", false);
    assert_eq!(session.batch().ranges.len(), 7);
    assert!(session.hidden_layers().is_empty());
}

#[test]
fn entity_info_for_rectangle() {
    let (_dir, session) = sample_session();
    // (2, 0) lies on both the line and the rectangle; the rectangle's right edge is its own.
    let picked = session
        .pick([4.0, 1.5], 0.05)
        .expect("rectangle edge is picked");
    // On the sheet itself: no chain, and the entity is the top-level one.
    assert!(picked.chain.is_empty());
    assert_eq!(picked.top, picked.entity);
    let handle = picked.entity;
    let info = session.entity_info(handle, &[]).unwrap();
    assert_eq!(info.handle, handle);
    assert_eq!(info.inside_block, None);
    assert_eq!(info.kind, "LwPolyline");
    assert_eq!(info.layer, "A-WALL");
    assert_eq!(info.area, Some(12.0));
    assert_eq!(info.length, Some(14.0));
    assert_eq!(info.block_name, None);
    assert_eq!(info.text, None);

    assert!(matches!(
        session.entity_info(0xDEAD_BEEF, &[]),
        Err(CoreError::NoSuchEntity(0xDEAD_BEEF))
    ));
}

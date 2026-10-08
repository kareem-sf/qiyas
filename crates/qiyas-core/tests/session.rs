use qiyas_core::{Command, CoreError, Session};

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

#[test]
fn batch_version_ignores_the_take_off_and_follows_the_drawing_view() {
    let (_dir, mut session) = sample_session();
    let v0 = session.batch_version();
    assert_eq!(session.batch_version(), v0);
    // The window holds it as a JavaScript number, which is exact up to 2^53.
    assert!(v0 < 1 << 53);

    // A measurement and its undo and redo do not change what is drawn.
    session
        .run(Command::MeasureDistance {
            points: vec![[0.0, 0.0], [3.0, 4.0]],
        })
        .unwrap();
    assert_eq!(session.batch_version(), v0);
    assert!(session.undo());
    assert_eq!(session.batch_version(), v0);
    assert!(session.redo());
    assert_eq!(session.batch_version(), v0);

    // Hiding a layer does, and showing it again gives the first version back.
    session.set_layer_hidden("A-WALL", true);
    let hidden = session.batch_version();
    assert_ne!(hidden, v0);
    session.set_layer_hidden("a-wall", false);
    assert_eq!(session.batch_version(), v0);

    // So does another sheet.
    let paper = session
        .info()
        .sheets
        .into_iter()
        .find(|s| !s.is_model)
        .expect("the sample has a paper layout");
    session.set_sheet(&paper.block).unwrap();
    assert_ne!(session.batch_version(), v0);
    assert_ne!(session.batch_version(), hidden);
    session.set_sheet("*Model_Space").unwrap();
    assert_eq!(session.batch_version(), v0);
}

#[test]
fn saving_changes_the_batch_version_and_another_session_never_shares_it() {
    let (dir, mut session) = sample_session();
    let v0 = session.batch_version();
    // The save embeds the (empty) take-off, but still counts as a change to the drawing.
    session.save().unwrap();
    assert_ne!(session.batch_version(), v0);

    let again = Session::open(&dir.path().join("sample.dxf")).unwrap();
    let other = Session::open(&dir.path().join("sample.dxf")).unwrap();
    assert_ne!(again.batch_version(), other.batch_version());
}

#[test]
fn batch_bytes_are_sent_only_when_the_version_differs() {
    let (_dir, mut session) = sample_session();
    let v0 = session.batch_version();
    let full = session.batch().to_bytes();
    assert_eq!(session.batch_bytes_if(None), Some(full.clone()));
    assert_eq!(session.batch_bytes_if(Some(v0 ^ 1)), Some(full));
    assert_eq!(session.batch_bytes_if(Some(v0)), None);

    // A layer click changes the version, so the old one gets the new batch.
    session.set_layer_hidden("A-WALL", true);
    let bytes = session.batch_bytes_if(Some(v0)).expect("batch changed");
    assert_eq!(bytes, session.batch().to_bytes());
    assert_eq!(session.batch_bytes_if(Some(session.batch_version())), None);
}

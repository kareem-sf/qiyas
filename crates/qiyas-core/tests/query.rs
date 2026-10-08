use qiyas_core::{EntityFilter, Session};

/// Opens a fixture drawing (DXF) in a session. The temp dir must outlive the test body.
fn open(write: fn(&std::path::Path, &str) -> std::path::PathBuf) -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "dxf");
    let session = Session::open(&path).unwrap();
    (dir, session)
}

#[test]
fn find_entities_filters_and_pages() {
    let (_dir, session) = open(qiyas_core::fixtures::write_sample);

    let page = session.find_entities(&EntityFilter {
        limit: 2,
        ..Default::default()
    });
    assert_eq!(page.total, 7);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.next_offset, Some(2));

    let last = session.find_entities(&EntityFilter {
        offset: 6,
        limit: 2,
        ..Default::default()
    });
    assert_eq!((last.rows.len(), last.next_offset), (1, None));

    // Layer, kind and block names match case-insensitively.
    let doors = session.find_entities(&EntityFilter {
        layer: Some("a-door".to_string()),
        ..Default::default()
    });
    assert_eq!(doors.total, 3);
    assert!(doors
        .rows
        .iter()
        .all(|r| r.kind == "Insert" && r.block_name.as_deref() == Some("DOOR")));
    let by_kind = session.find_entities(&EntityFilter {
        kind: Some("insert".to_string()),
        block_name: Some("door".to_string()),
        ..Default::default()
    });
    assert_eq!(by_kind.total, 3);
    // The plain door's leaf runs from (1, 1) to (1.9, 1) in world coordinates.
    let plain = doors
        .rows
        .iter()
        .find(|r| r.bounds[0] == [1.0, 1.0])
        .unwrap();
    assert!(
        (plain.bounds[1][0] - 1.9).abs() < 1e-9,
        "{:?}",
        plain.bounds
    );

    let title = session.find_entities(&EntityFilter {
        text_contains: Some("ground".to_string()),
        ..Default::default()
    });
    assert_eq!(title.total, 1);
    assert_eq!(title.rows[0].kind, "Text");
    assert_eq!(title.rows[0].text.as_deref(), Some("GROUND FLOOR PLAN"));

    // Only the rectangle has a point (its corner at (4, 3)) in this box.
    let boxed = session.find_entities(&EntityFilter {
        within: Some([[3.5, 2.5], [4.5, 3.5]]),
        ..Default::default()
    });
    assert_eq!(boxed.total, 1);
    let rect = &boxed.rows[0];
    assert_eq!(rect.kind, "LwPolyline");
    assert_eq!((rect.length, rect.area), (Some(14.0), Some(12.0)));
    assert_eq!(rect.bounds, [[0.0, 0.0], [4.0, 3.0]]);
}

#[test]
fn read_text_strips_mtext_codes() {
    let (_dir, session) = open(qiyas_core::fixtures::write_schedule);
    let texts = session.read_text(None, None);
    let note = texts.iter().find(|t| t.text.contains("FIRE")).unwrap();
    assert_eq!(note.text, "FIRE RATED\nDOORS");
    assert_eq!(note.at, [0.0, -8.0]);

    // An insert's attribute, and the text inside its block, placed by the insert.
    let mark = texts.iter().find(|t| t.text == "D9").unwrap();
    assert_eq!(
        (mark.at, mark.inside_block.as_deref()),
        ([30.5, 10.0], None)
    );
    let nested = texts.iter().find(|t| t.text == "T1").unwrap();
    assert_eq!(nested.at, [30.0, 10.0]);
    assert_eq!(nested.inside_block.as_deref(), Some("TAG"));
    assert!((nested.height - 0.2).abs() < 1e-9);

    let boxed = session.read_text(None, Some([[-1.0, -9.0], [1.0, -7.0]]));
    assert_eq!(boxed.len(), 1);
    assert_eq!(boxed[0].text, note.text);
}

#[test]
fn read_tables_finds_the_text_grid_and_the_table_entity() {
    let (_dir, session) = open(qiyas_core::fixtures::write_schedule);
    let schedules = session.read_tables(None);
    assert_eq!(schedules.len(), 2, "{schedules:?}");

    let grid = schedules.iter().find(|s| s.source == 0).unwrap();
    assert_eq!(grid.name, "DOOR SCHEDULE");
    assert_eq!(grid.sheet, "*Model_Space");
    assert_eq!(grid.columns, ["MARK", "WIDTH", "QTY"]);
    assert_eq!(grid.rows.len(), 3);
    assert_eq!(grid.rows[0], ["D1", "0.9", "2"]);
    assert_eq!(grid.rows[2], ["D3", "1.2", "4"]);

    let table = schedules.iter().find(|s| s.source != 0).unwrap();
    assert_eq!(table.name, "Table at 20.0,0.0");
    assert_eq!(table.columns, ["A", "B"]);
    assert_eq!(table.rows, [["C", "D"]]);
}

#[test]
fn read_dimensions_reports_linear_value() {
    let (_dir, session) = open(qiyas_core::fixtures::write_schedule);
    let dims = session.read_dimensions(None);
    assert_eq!(dims.len(), 1);
    assert_eq!(dims[0].kind, "Linear");
    assert_eq!(dims[0].measurement, Some(4.0));
    assert_eq!(dims[0].text_override, None);
}

#[test]
fn block_summary_counts_per_block() {
    let (_dir, mut session) = open(qiyas_core::fixtures::write_sample);
    let summary = session.block_summary(None);
    assert_eq!(summary.len(), 1);
    assert_eq!(
        (
            summary[0].block_name.as_str(),
            summary[0].layer.as_str(),
            summary[0].count
        ),
        ("DOOR", "A-DOOR", 3)
    );

    session.set_layer_hidden("A-DOOR", true);
    assert!(session.block_summary(None).is_empty());
}

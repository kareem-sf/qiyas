use opencadcodec::{CadDocument, DxfWriter, EntityType, Line, Text, Vector3};
use qiyas_ai::{catalogue, execute, ToolError, ToolOutput};
use qiyas_core::{Boq, BoqItem, Command, EntityFilter, Kind, Session, Source};
use serde_json::{json, Value};

/// Opens the sample drawing (DXF) in a session. The temp dir must outlive the test body.
fn sample() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let session = Session::open(&path).unwrap();
    (dir, session)
}

/// The handles of the sample's entities of `kind` (e.g. "Insert"), in drawing order.
fn handles_of(session: &Session, kind: &str) -> Vec<u64> {
    session
        .find_entities(&EntityFilter {
            kind: Some(kind.to_string()),
            ..Default::default()
        })
        .rows
        .iter()
        .map(|r| r.handle)
        .collect()
}

fn json_of(output: ToolOutput) -> Value {
    match output {
        ToolOutput::Json(v) => v,
        ToolOutput::Image { .. } => panic!("expected JSON, got an image"),
    }
}

fn run(session: &mut Session, name: &str, input: Value) -> Result<Value, ToolError> {
    execute(session, name, &input, Source::Ai).map(json_of)
}

/// Schema keywords the API's strict tool mode does not accept.
const UNSUPPORTED: [&str; 7] = [
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "oneOf",
];

fn assert_strict_keywords(name: &str, v: &Value) {
    match v {
        Value::Object(map) => {
            for key in UNSUPPORTED {
                assert!(!map.contains_key(key), "{name} uses {key}");
            }
            if map.get("type") == Some(&json!("object")) {
                assert_eq!(
                    map.get("additionalProperties"),
                    Some(&json!(false)),
                    "{name}"
                );
            }
            map.values().for_each(|v| assert_strict_keywords(name, v));
        }
        Value::Array(items) => items.iter().for_each(|v| assert_strict_keywords(name, v)),
        _ => {}
    }
}

#[test]
fn catalogue_schemas_are_strict_objects() {
    let tools = catalogue();
    let names: Vec<&str> = tools.iter().map(|t| t.name).collect();
    assert_eq!(
        names,
        [
            "list_sheets",
            "set_sheet",
            "list_layers",
            "show_layers",
            "find_entities",
            "get_entity",
            "read_text",
            "read_tables",
            "read_dimensions",
            "block_summary",
            "measure_length",
            "measure_area",
            "count_blocks",
            "add_measurement",
            "set_height",
            "list_takeoff",
            "remove_measurement",
            "set_description",
            "import_boq",
            "list_boq",
            "link_boq",
            "compare_boq",
            "list_conflicts",
            "resolve_conflict",
            "add_markup",
            "zoom_to",
            "view_image",
        ]
    );
    for tool in &tools {
        let schema = &tool.input_schema;
        assert_eq!(schema["type"], "object", "{}", tool.name);
        assert_eq!(
            schema["additionalProperties"],
            json!(false),
            "{}",
            tool.name
        );
        let properties = schema["properties"].as_object().unwrap();
        let required = schema["required"].as_array().unwrap();
        for r in required {
            assert!(
                properties.contains_key(r.as_str().unwrap()),
                "{}: {r} is required but not a property",
                tool.name
            );
        }
        assert!(!tool.description.is_empty(), "{}", tool.name);
        assert_strict_keywords(tool.name, schema);
    }
}

#[test]
fn execute_measure_area_by_handle_marks_ai_source() {
    let (_dir, mut session) = sample();
    let rectangle = handles_of(&session, "LwPolyline")[0];
    let out = run(&mut session, "measure_area", json!({ "handle": rectangle })).unwrap();
    let m = &out["measurement"];
    assert_eq!(m["source"], "ai");
    assert_eq!(m["kind"], "area");
    assert_eq!(m["unit"], "m2");
    assert!((m["shown"].as_f64().unwrap() - 12.0).abs() < 1e-9, "{m}");
    assert_eq!(session.takeoff.items.len(), 1);
    assert_eq!(session.takeoff.items[0].source, "ai");
    assert_eq!(session.takeoff.items[0].handles, [rectangle]);

    // By points, from MCP.
    execute(
        &mut session,
        "measure_length",
        &json!({ "points": [[0, 0], [3, 0], [3, 4]] }),
        Source::Mcp,
    )
    .unwrap();
    let length = &session.takeoff.items[1];
    assert_eq!((length.kind, length.value), (Kind::Length, 7.0));
    assert_eq!(length.source, "mcp");
}

#[test]
fn execute_find_entities_truncates_at_40kb() {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = 6;
    for i in 0..2000 {
        let y = i as f64;
        let line = Line::from_coords(0.0, y, 0.0, 10.0, y, 0.0);
        doc.add_entity(EntityType::Line(line)).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lines.dxf");
    DxfWriter::new(&doc).write_to_file(&path).unwrap();
    let mut session = Session::open(&path).unwrap();

    let out = run(&mut session, "find_entities", json!({ "limit": 500 })).unwrap();
    assert_eq!(out["truncated"], true);
    assert_eq!(out["total"], 2000);
    let rows = out["rows"].as_array().unwrap().len();
    assert!(rows > 0 && rows < 500, "{rows}");
    assert_eq!(out["next_offset"], rows);
    assert!(serde_json::to_vec(&out).unwrap().len() <= 40_960);

    // The next page starts where this one stopped.
    let next = run(
        &mut session,
        "find_entities",
        json!({ "limit": 500, "offset": rows }),
    )
    .unwrap();
    assert_eq!(
        next["rows"][0]["handle"],
        handles_at(&session, rows),
        "{}",
        next["rows"][0]
    );
}

/// The handle of the `offset`-th entity of the current sheet.
fn handles_at(session: &Session, offset: usize) -> Value {
    json!(
        session
            .find_entities(&EntityFilter {
                offset,
                limit: 1,
                ..Default::default()
            })
            .rows[0]
            .handle
    )
}

#[test]
fn view_image_is_a_png_of_the_right_size() {
    let (_dir, mut session) = sample();
    let out = execute(
        &mut session,
        "view_image",
        &json!({ "width": 640 }),
        Source::Ai,
    )
    .unwrap();
    let ToolOutput::Image { png, caption } = out else {
        panic!("expected an image");
    };
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    assert_eq!(width, 640);
    assert!((1..=1600).contains(&height), "{height}");
    assert!(caption.contains("bounds"), "{caption}");

    // Bounds given: a 2:1 box gives a 2:1 picture.
    let out = execute(
        &mut session,
        "view_image",
        &json!({ "width": 400, "bounds": [[0, 0], [10, 5]] }),
        Source::Ai,
    )
    .unwrap();
    let ToolOutput::Image { png, .. } = out else {
        panic!("expected an image");
    };
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    assert_eq!(height, 200);
}

#[test]
fn unknown_tool_is_an_error() {
    let (_dir, mut session) = sample();
    let err = execute(&mut session, "delete_everything", &json!({}), Source::Ai).unwrap_err();
    assert!(matches!(err, ToolError::UnknownTool(ref n) if n == "delete_everything"));
    // Every tool in the catalogue is known to `execute`.
    for tool in catalogue() {
        let result = execute(&mut session, tool.name, &json!({}), Source::Ai);
        assert!(
            !matches!(result, Err(ToolError::UnknownTool(_))),
            "{} is not dispatched",
            tool.name
        );
    }
}

#[test]
fn add_markup_lands_on_the_markup_layer() {
    let (dir, mut session) = sample();
    let out = run(
        &mut session,
        "add_markup",
        json!({
            "points": [[0, 0], [4, 0], [4, 3]],
            "text": "Check this wall",
            "kind": "cloud"
        }),
    )
    .unwrap();
    assert_eq!(out["ui"]["action"], "show_markup");
    let m = &session.takeoff.items[0];
    assert_eq!(
        (m.kind, m.value, m.unit.as_str()),
        (Kind::Item, 0.0, "markup")
    );
    assert_eq!(
        (m.description.as_str(), m.note.as_str()),
        ("Check this wall", "cloud")
    );

    let out = dir.path().join("out.dxf");
    session.save_as(&out).unwrap();
    std::fs::remove_file(qiyas_core::project_path(&out)).unwrap();
    let reopened = Session::open(&out).unwrap();
    let markup_layer: Vec<&str> = reopened
        .drawing
        .doc
        .entities()
        .filter(|e| e.common().extended_data.get_record("QIYAS").is_some())
        .map(|e| e.common().layer.as_str())
        .collect();
    assert_eq!(markup_layer, ["QIYAS-MARKUP"]);
    let back = &reopened.takeoff.items[0];
    assert_eq!(
        (back.unit.as_str(), back.description.as_str()),
        ("markup", "Check this wall")
    );
}

#[test]
fn count_with_non_insert_handle_is_bad_input() {
    let (_dir, mut session) = sample();
    let line = handles_of(&session, "Line")[0];
    let doors = handles_of(&session, "Insert");
    let count =
        |handles: Vec<u64>| json!({ "kind": "count", "handles": handles, "description": "Doors" });

    let err = run(&mut session, "add_measurement", count(vec![doors[0], line])).unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
    let err = run(
        &mut session,
        "add_measurement",
        count(vec![doors[0], doors[0]]),
    )
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
    assert!(session.takeoff.items.is_empty());

    let out = run(&mut session, "add_measurement", count(doors)).unwrap();
    assert_eq!(out["measurement"]["value"], 3.0);
    assert_eq!(out["measurement"]["source"], "ai");
}

#[test]
fn missing_handles_blocks_and_ids_are_bad_input() {
    let (_dir, mut session) = sample();
    for (name, input) in [
        ("measure_length", json!({ "handle": 999_999 })),
        ("measure_area", json!({ "handle": 999_999 })),
        ("measure_area", json!({})),
        ("count_blocks", json!({ "block_name": "WINDOW" })),
        ("get_entity", json!({ "handle": 999_999 })),
        ("set_description", json!({ "id": "nope", "text": "x" })),
        ("remove_measurement", json!({ "id": "nope" })),
        ("set_sheet", json!({ "sheet": "Layout 9" })),
        (
            "show_layers",
            json!({ "names": ["NO-SUCH"], "visible": false }),
        ),
        ("find_entities", json!({ "colour": "red" })),
    ] {
        let err = run(&mut session, name, input).unwrap_err();
        assert!(matches!(err, ToolError::BadInput(_)), "{name}: {err:?}");
    }
    assert!(session.takeoff.items.is_empty());

    // Block names match case-insensitively.
    let out = run(
        &mut session,
        "count_blocks",
        json!({ "block_name": "door" }),
    )
    .unwrap();
    assert_eq!(out["measurement"]["value"], 3.0);
}

#[test]
fn compare_and_conflict_tools_work() {
    let (_dir, mut session) = sample();
    let item = |code: &str, description: &str, unit: &str, quantity: f64| BoqItem {
        code: code.to_string(),
        description: description.to_string(),
        unit: unit.to_string(),
        quantity,
        rate: None,
        section: String::new(),
        tolerance: None,
        row: 2,
    };
    let bill = Boq::new(
        "boq.xlsx".to_string(),
        vec![
            item("A.1", "Walls", "m", 100.0),
            item("A.2", "Roof waterproofing", "m2", 50.0),
        ],
        0,
    );
    session.run(Command::SetBoq { boq: bill }).unwrap();
    let out = run(
        &mut session,
        "measure_length",
        json!({ "points": [[0, 0], [10, 0]] }),
    )
    .unwrap();
    let id = out["measurement"]["id"].as_str().unwrap().to_string();
    run(&mut session, "link_boq", json!({ "id": id, "code": "A.1" })).unwrap();
    assert_eq!(session.takeoff.items[0].boq_code.as_deref(), Some("A.1"));

    let out = run(&mut session, "compare_boq", json!({})).unwrap();
    assert_eq!(out["total"], 2, "{out}");
    assert_eq!(out["by_kind"]["quantity_mismatch"], 1, "{out}");
    assert_eq!(out["by_kind"]["not_in_drawings"], 1, "{out}");
    assert_eq!(out["by_status"]["open"], 2, "{out}");
    assert_eq!(session.conflicts.len(), 2);
    assert_eq!(session.top_step_source(), Some(Source::Ai));
    let mismatch = out["rows"][0].clone();
    assert_eq!(mismatch["kind"], "quantity_mismatch");
    assert_eq!(mismatch["boq_code"], "A.1");
    assert_eq!(mismatch["expected"], 100.0);
    assert_eq!(mismatch["found"], 10.0);
    let conflict_id = mismatch["id"].as_str().unwrap().to_string();

    let out = run(
        &mut session,
        "resolve_conflict",
        json!({ "id": conflict_id, "status": "ignored", "note": "Walls are on sheet 2" }),
    )
    .unwrap();
    assert_eq!(out["conflict"]["status"], "ignored");
    for input in [
        json!({ "id": "nope", "status": "resolved", "note": "" }),
        json!({ "id": conflict_id, "status": "done", "note": "" }),
    ] {
        let err = run(&mut session, "resolve_conflict", input).unwrap_err();
        assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
    }

    // A new comparison keeps the status and note; a wider tolerance does not change the ids.
    let out = run(&mut session, "compare_boq", json!({ "tolerance": 10 })).unwrap();
    assert_eq!(out["rows"][0]["id"], conflict_id.as_str());
    assert_eq!(out["rows"][0]["status"], "ignored");
    assert_eq!(out["rows"][0]["note"], "Walls are on sheet 2");
    let list = run(&mut session, "list_conflicts", json!({ "offset": 1 })).unwrap();
    assert_eq!(list["total"], 2);
    assert_eq!(list["rows"].as_array().unwrap().len(), 1);
    assert_eq!(list["by_status"]["ignored"], 1);

    let err = run(&mut session, "compare_boq", json!({ "tolerance": -1 })).unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
}

#[test]
fn compare_counts_blocks_on_layers_the_user_hides() {
    // The sample (three DOOR references on A-DOOR) with a schedule saying 2 doors.
    let mut doc = qiyas_core::fixtures::sample_doc();
    let grid = [["MARK", "QTY"], ["DOOR", "2"], ["WINDOW", "1"]];
    for (r, row) in grid.iter().enumerate() {
        for (c, value) in row.iter().enumerate() {
            let at = Vector3::new(100.0 + 2.0 * c as f64, 100.0 - 0.5 * r as f64, 0.0);
            let text = Text::with_value(*value, at).with_height(0.25);
            doc.add_entity(EntityType::Text(text)).unwrap();
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doors.dxf");
    DxfWriter::new(&doc).write_to_file(&path).unwrap();
    let mut session = Session::open(&path).unwrap();
    session.set_layer_hidden("A-DOOR", true);

    let out = run(&mut session, "compare_boq", json!({})).unwrap();
    assert_eq!(out["total"], 1, "{out}");
    assert_eq!(out["rows"][0]["kind"], "schedule_vs_plan");
    assert_eq!(out["rows"][0]["expected"], 2.0);
    assert_eq!(out["rows"][0]["found"], 3.0, "hidden doors still count");
}

#[test]
fn heights_are_metres_and_zoom_returns_bounds() {
    let (_dir, mut session) = sample();
    let out = run(
        &mut session,
        "add_measurement",
        json!({
            "kind": "volume",
            "points": [[0, 0], [4, 0], [4, 3], [0, 3]],
            "height": 0.2,
            "description": "Slab"
        }),
    )
    .unwrap();
    assert!((out["measurement"]["shown"].as_f64().unwrap() - 2.4).abs() < 1e-9);

    let doors = handles_of(&session, "Insert");
    let out = run(&mut session, "zoom_to", json!({ "handles": [doors[0]] })).unwrap();
    assert_eq!(out["ui"]["action"], "zoom");
    // The plain door's leaf runs from (1, 1) to (1.9, 1).
    let b = &out["ui"]["bounds"];
    assert!((b[0][0].as_f64().unwrap() - 1.0).abs() < 1e-6, "{b}");
    assert!((b[1][0].as_f64().unwrap() - 1.9).abs() < 1e-6, "{b}");

    let takeoff = run(&mut session, "list_takeoff", json!({})).unwrap();
    assert_eq!(takeoff["rows"].as_array().unwrap().len(), 1);
    assert_eq!(takeoff["totals"][0]["unit"], "m3");
}

#[test]
fn import_boq_and_list_boq() {
    let (dir, mut session) = sample();
    let err = run(&mut session, "list_boq", json!({})).unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    let path = dir.path().join("boq.xlsx");
    let mut book = rust_xlsxwriter::Workbook::new();
    let sheet = book.add_worksheet();
    sheet
        .write_row(0, 0, ["Item", "Description", "Unit", "Quantity"])
        .unwrap();
    sheet.write_string(1, 1, "Finishes").unwrap();
    sheet
        .write_row(2, 0, ["F.1", "Floor tiles", "sqm"])
        .unwrap();
    sheet.write_number(2, 3, 48.0).unwrap();
    book.save(&path).unwrap();

    let missing = json!({ "path": dir.path().join("nope.xlsx") });
    let err = run(&mut session, "import_boq", missing).unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    let out = run(&mut session, "import_boq", json!({ "path": path })).unwrap();
    assert_eq!(out["items"], 1);
    assert_eq!(out["sections"], json!(["Finishes"]));
    assert_eq!(session.top_step_source(), Some(Source::Ai));

    let out = run(&mut session, "list_boq", json!({})).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["rows"][0]["code"], "F.1");
    assert_eq!(out["rows"][0]["unit"], "m2");
    assert_eq!(out["rows"][0]["section"], "Finishes");

    assert!(session.undo_if(Source::Ai));
    assert_eq!(session.boq, None);
}

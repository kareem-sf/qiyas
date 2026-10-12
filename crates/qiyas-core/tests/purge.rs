//! PURGE: what nothing uses (layers, blocks, text styles, linetypes, dimension styles, groups)
//! listed by a dry run, then taken out in one undo step.

use opencadcodec::entities::{CellContent, Insert, MultiLeader, Table, TableRow};
use opencadcodec::objects::ObjectType;
use opencadcodec::{
    BlockRecord, CadDocument, DimStyle, EntityType, Handle, Layer, Line, LineType, TextStyle,
    Vector3,
};
use qiyas_core::fixtures::{sample_doc, write_drawing, write_sample};
use qiyas_core::purge::{PurgeKind, PurgeReport};
use qiyas_core::{Command, DimSpec, Outcome, Session, Target};

fn open(doc: &CadDocument) -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = write_drawing(doc, &dir.path().join("plan.dxf"));
    let s = Session::open(&path).unwrap();
    (dir, s)
}

fn t(handle: u64) -> Target {
    Target {
        handle,
        chain: Vec::new(),
    }
}

fn purge(s: &mut Session, kinds: &[PurgeKind], dry_run: bool) -> PurgeReport {
    match s
        .run(Command::Purge {
            kinds: kinds.to_vec(),
            dry_run,
        })
        .unwrap()
    {
        Outcome::Purged { report } => report,
        other => panic!("expected a purge report, got {other:?}"),
    }
}

fn sorted(names: &[String]) -> Vec<String> {
    let mut names = names.to_vec();
    names.sort();
    names
}

/// A block `name` holding `entities`; its record's handle.
fn add_block(doc: &mut CadDocument, name: &str, entities: Vec<EntityType>) -> Handle {
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
    owner
}

fn line() -> EntityType {
    EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0))
}

/// The sample with one of each kind nothing uses: layer OLD, text style OLDTEXT, linetype
/// DASHED2, dimension style OLDDIM, block SPARE, and block OUTER holding the only reference to
/// INNER. Its own block DOOR and layers stay in use.
fn cluttered() -> CadDocument {
    let mut doc = sample_doc();
    let mut layer = opencadcodec::Layer::new("OLD");
    layer.handle = doc.allocate_handle();
    doc.layers.add(layer).unwrap();
    let mut style = TextStyle::new("OLDTEXT");
    style.handle = doc.allocate_handle();
    doc.text_styles.add(style).unwrap();
    let mut linetype = LineType::new("DASHED2");
    linetype.handle = doc.allocate_handle();
    doc.line_types.add(linetype).unwrap();
    let mut dimstyle = DimStyle::new("OLDDIM");
    dimstyle.handle = doc.allocate_handle();
    doc.dim_styles.add(dimstyle).unwrap();
    add_block(&mut doc, "SPARE", vec![line()]);
    add_block(&mut doc, "INNER", vec![line()]);
    let inner = opencadcodec::entities::Insert::new("INNER", opencadcodec::Vector3::ZERO);
    add_block(&mut doc, "OUTER", vec![EntityType::Insert(inner)]);
    doc
}

#[test]
fn a_dry_run_lists_exactly_the_unused_layer() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    let mut s = Session::open(&path).unwrap();
    s.run(Command::NewLayer {
        name: "UNUSED".into(),
    })
    .unwrap();
    let report = purge(&mut s, &[PurgeKind::Layers], true);
    assert!(report.dry_run);
    assert_eq!(report.layers, vec!["UNUSED".to_string()]);
    assert_eq!(report.total, 1);
    assert!(report.blocks.is_empty() && report.styles.is_empty());
    // Nothing was taken out and no step was made: the last step is still the new layer.
    assert!(s.drawing.doc.layers.get("UNUSED").is_some());
    assert!(s.undo());
    assert!(s.drawing.doc.layers.get("UNUSED").is_none());
}

#[test]
fn a_dry_run_lists_every_kind_and_the_purge_takes_them_out_in_one_step() {
    let (_dir, mut s) = open(&cluttered());
    let before = purge(&mut s, &[], true);
    assert_eq!(before.layers, vec!["OLD".to_string()]);
    assert_eq!(
        sorted(&before.blocks),
        vec!["INNER".to_string(), "OUTER".into(), "SPARE".into()],
        "INNER is used only inside OUTER, which nothing uses"
    );
    assert_eq!(before.styles, vec!["OLDTEXT".to_string()]);
    assert_eq!(before.linetypes, vec!["DASHED2".to_string()]);
    assert_eq!(before.dimstyles, vec!["OLDDIM".to_string()]);
    assert_eq!(before.total, 7);

    // Only the kinds asked for.
    let layers_only = purge(&mut s, &[PurgeKind::Layers], true);
    assert_eq!(layers_only.total, 1);
    assert!(layers_only.blocks.is_empty());

    let entities = s.drawing.doc.entities().count();
    let can_undo = s.can_undo();
    let done = purge(&mut s, &[], false);
    assert!(!done.dry_run);
    assert_eq!(done.total, 7);
    let doc = &s.drawing.doc;
    assert!(doc.layers.get("OLD").is_none());
    assert!(doc.text_styles.get("OLDTEXT").is_none());
    assert!(doc.line_types.get("DASHED2").is_none());
    assert!(doc.dim_styles.get("OLDDIM").is_none());
    for name in ["SPARE", "INNER", "OUTER"] {
        assert!(doc.block_records.get(name).is_none(), "{name} went");
    }
    assert_eq!(
        doc.entities().count(),
        entities - 3,
        "the three blocks' entities went too"
    );
    // What is used stays.
    assert!(doc.block_records.get("DOOR").is_some());
    assert!(doc.layers.get("A-WALL").is_some() && doc.layers.get("0").is_some());
    assert!(doc.text_styles.get("Standard").is_some());
    assert!(doc.line_types.get("Continuous").is_some());
    assert!(purge(&mut s, &[], true).total == 0, "nothing left to purge");

    // One step: a single undo puts every table back as it was.
    assert!(s.undo());
    let doc = &s.drawing.doc;
    assert!(doc.layers.get("OLD").is_some());
    assert!(doc.text_styles.get("OLDTEXT").is_some());
    assert!(doc.line_types.get("DASHED2").is_some());
    assert!(doc.dim_styles.get("OLDDIM").is_some());
    assert!(doc.block_records.get("OUTER").is_some());
    assert_eq!(doc.entities().count(), entities);
    assert_eq!(doc.entities_in_block("SPARE").count(), 1);
    assert_eq!(s.can_undo(), can_undo, "the purge was one step");
    assert!(s.redo());
    assert!(s.drawing.doc.block_records.get("SPARE").is_none());
}

#[test]
fn a_purge_with_nothing_to_purge_is_no_step() {
    let (_dir, mut s) = open(&sample_doc());
    assert!(!s.can_undo());
    let report = purge(&mut s, &[], false);
    assert_eq!(report.total, 0);
    assert!(!s.can_undo());
}

#[test]
fn purge_never_removes_a_block_a_dimension_references() {
    let (_dir, mut s) = open(&sample_doc());
    let Outcome::Added { handles } = s
        .run(Command::Dimension(DimSpec::Linear {
            p1: [0.0, 0.0],
            p2: [10.0, 0.0],
            line_point: [5.0, 2.0],
            rotation: None,
        }))
        .unwrap()
    else {
        panic!("a dimension adds an entity");
    };
    let block = match s
        .drawing
        .doc
        .get_entity(opencadcodec::Handle::new(handles[0]))
    {
        Some(EntityType::Dimension(d)) => d.base().block_name.clone(),
        other => panic!("expected the dimension, got {other:?}"),
    };
    assert!(block.starts_with("*D"));
    assert!(!purge(&mut s, &[PurgeKind::Blocks], true)
        .blocks
        .contains(&block));
    purge(&mut s, &[], false);
    assert!(s.drawing.doc.block_records.get(&block).is_some());

    // Erased, its block is the save-time purge's orphan, and PURGE takes it out too.
    s.run(Command::DeleteEntities {
        targets: vec![t(handles[0])],
    })
    .unwrap();
    assert_eq!(
        purge(&mut s, &[PurgeKind::Blocks], true).blocks,
        vec![block.clone()]
    );
    purge(&mut s, &[PurgeKind::Blocks], false);
    assert!(s.drawing.doc.block_records.get(&block).is_none());
    assert!(s.undo());
    assert!(s.drawing.doc.block_records.get(&block).is_some());
    assert!(s.drawing.doc.entities_in_block(&block).count() > 0);
}

#[test]
fn groups_lose_their_erased_members_and_empty_groups_go() {
    let (_dir, mut s) = open(&sample_doc());
    let walls: Vec<u64> = s
        .drawing
        .doc
        .entities_in_block("*Model_Space")
        .filter(|e| e.common().layer == "A-WALL")
        .map(|e| e.common().handle.value())
        .take(2)
        .collect();
    s.run(Command::CreateGroup {
        name: "PAIR".into(),
        targets: walls.iter().map(|&h| t(h)).collect(),
    })
    .unwrap();
    s.run(Command::DeleteEntities {
        targets: vec![t(walls[0])],
    })
    .unwrap();
    let report = purge(&mut s, &[PurgeKind::Groups], true);
    assert!(report.groups.is_empty());
    assert_eq!(report.group_members, 1);
    assert_eq!(report.total, 1);
    purge(&mut s, &[PurgeKind::Groups], false);
    assert_eq!(s.info().groups[0].handles, vec![walls[1]]);

    s.run(Command::DeleteEntities {
        targets: vec![t(walls[1])],
    })
    .unwrap();
    let report = purge(&mut s, &[PurgeKind::Groups], false);
    assert_eq!(report.groups, vec!["PAIR".to_string()]);
    assert!(s.info().groups.is_empty());
    assert!(s.undo());
    assert_eq!(s.info().groups.len(), 1);
}

#[test]
fn the_current_layer_is_never_purged() {
    let (_dir, mut s) = open(&sample_doc());
    s.run(Command::NewLayer {
        name: "WORK".into(),
    })
    .unwrap();
    s.run(Command::SetCurrent {
        current: qiyas_core::Current {
            layer: "WORK".into(),
            ..s.current.clone()
        },
    })
    .unwrap();
    assert!(purge(&mut s, &[PurgeKind::Layers], true).layers.is_empty());
}

#[test]
fn the_wire_form_reads_and_the_report_writes() {
    let cmd: Command =
        serde_json::from_str(r#"{"type":"purge","kinds":["layers","dimstyles"],"dry_run":true}"#)
            .unwrap();
    assert!(matches!(cmd, Command::Purge { ref kinds, dry_run: true } if kinds.len() == 2));
    let every: Command = serde_json::from_str(r#"{"type":"purge","dry_run":false}"#).unwrap();
    assert!(matches!(every, Command::Purge { ref kinds, .. } if kinds.is_empty()));
    let (_dir, mut s) = open(&cluttered());
    let outcome = s
        .run(Command::Purge {
            kinds: vec![PurgeKind::Linetypes],
            dry_run: true,
        })
        .unwrap();
    let json = serde_json::to_value(&outcome).unwrap();
    assert_eq!(json["type"], "purged");
    assert_eq!(json["report"]["linetypes"][0], "DASHED2");
    assert_eq!(json["report"]["total"], 1);
}

#[test]
fn the_takeoff_layers_every_save_writes_stay() {
    let (dir, mut s) = open(&sample_doc());
    s.save_as(&dir.path().join("saved.dwg")).unwrap();
    assert!(s.drawing.doc.layers.get("QIYAS-LENGTH").is_some());
    assert!(purge(&mut s, &[PurgeKind::Layers], true).layers.is_empty());
}

fn layer(doc: &mut CadDocument, name: &str) -> Handle {
    let mut layer = Layer::new(name);
    layer.handle = doc.allocate_handle();
    let h = layer.handle;
    doc.layers.add(layer).unwrap();
    h
}

fn text_style(doc: &mut CadDocument, name: &str) -> Handle {
    let mut style = TextStyle::new(name);
    style.handle = doc.allocate_handle();
    let h = style.handle;
    doc.text_styles.add(style).unwrap();
    h
}

fn linetype(doc: &mut CadDocument, name: &str) -> Handle {
    let mut lt = LineType::new(name);
    lt.handle = doc.allocate_handle();
    let h = lt.handle;
    doc.line_types.add(lt).unwrap();
    h
}

fn dimstyle(doc: &mut CadDocument, name: &str) -> DimStyle {
    let mut d = DimStyle::new(name);
    d.handle = doc.allocate_handle();
    d
}

fn every_list(report: &PurgeReport) -> Vec<String> {
    [
        &report.layers,
        &report.blocks,
        &report.styles,
        &report.linetypes,
        &report.dimstyles,
    ]
    .into_iter()
    .flatten()
    .cloned()
    .collect()
}

#[test]
fn what_only_a_less_plain_reference_uses_is_never_listed() {
    let (_dir, mut s) = open(&sample_doc());
    let doc = &mut s.drawing.doc;
    // A block only an MLEADER shows, one only a table cell shows, one only a dimension style's
    // DIMBLK is; a text style only that dimension style names.
    let mleader_block = add_block(doc, "MLBLOCK", vec![line()]);
    let mut leader = MultiLeader::new();
    leader.block_content_handle = Some(mleader_block);
    doc.add_entity(EntityType::MultiLeader(leader)).unwrap();
    let cell_block = add_block(doc, "CELLBLOCK", vec![line()]);
    let mut table = Table::new(Vector3::new(50.0, 0.0, 0.0), 0, 0);
    table.rows = vec![TableRow::from_texts(&["A"])];
    table.rows[0].cells[0].contents.push(CellContent {
        block_handle: Some(cell_block),
        ..CellContent::default()
    });
    // A `*T` block its table uses.
    add_block(doc, "*T7", vec![line()]);
    table.block_name = "*T7".into();
    doc.add_entity(EntityType::Table(table)).unwrap();
    let dimblk = add_block(doc, "ARRBLK", vec![line()]);
    let dimtext = text_style(doc, "DIMTEXT");
    let mut arrows = dimstyle(doc, "ARROWS");
    arrows.dimblk = dimblk;
    arrows.dimblk_name = "ARRBLK".into();
    arrows.dimtxsty = "DIMTEXT".into();
    arrows.dimtxsty_handle = dimtext;
    doc.dim_styles.add(arrows).unwrap();
    // A linetype only a layer in use has.
    linetype(doc, "LAYLT");
    layer(doc, "LTLAYER");
    doc.layers.get_mut("LTLAYER").unwrap().line_type = "LAYLT".into();
    let mut on_it = line();
    on_it.common_mut().layer = "LTLAYER".into();
    doc.add_entity(on_it).unwrap();
    // A layer used only inside a block definition that a reference keeps.
    layer(doc, "INBLOCK");
    let mut inside = line();
    inside.common_mut().layer = "INBLOCK".into();
    add_block(doc, "KEPT", vec![inside]);
    doc.add_entity(EntityType::Insert(Insert::new("KEPT", Vector3::ZERO)))
        .unwrap();
    // The current text style and dimension style.
    let current_text = text_style(doc, "CURTEXT");
    doc.header.current_text_style_name = "CURTEXT".into();
    doc.header.current_text_style_handle = current_text;
    let current_dim = dimstyle(doc, "CURDIM");
    doc.header.current_dimstyle_name = "CURDIM".into();
    doc.header.current_dimstyle_handle = current_dim.handle;
    doc.dim_styles.add(current_dim).unwrap();
    // An xref's names.
    layer(doc, "XR|WALL");
    text_style(doc, "XR|STYLE");
    linetype(doc, "XR|DASH");
    add_block(doc, "XR|BLOCK", vec![line()]);
    let xr_dim = dimstyle(doc, "XR|DIM");
    doc.dim_styles.add(xr_dim).unwrap();
    // And one that nothing uses, which is listed.
    add_block(doc, "SPARE", vec![line()]);

    let report = purge(&mut s, &[], true);
    assert_eq!(report.limited_reason, None, "every kind was looked at");
    assert_eq!(report.blocks, vec!["SPARE".to_string()]);
    let listed = every_list(&report);
    for name in [
        "MLBLOCK",
        "CELLBLOCK",
        "*T7",
        "ARRBLK",
        "DIMTEXT",
        "LAYLT",
        "LTLAYER",
        "INBLOCK",
        "KEPT",
        "CURTEXT",
        "CURDIM",
        "XR|WALL",
        "XR|STYLE",
        "XR|DASH",
        "XR|BLOCK",
        "XR|DIM",
    ] {
        assert!(
            !listed.contains(&name.to_string()),
            "{name} is in use: {listed:?}"
        );
    }
}

#[test]
fn a_drawing_with_unread_data_purges_layers_only_and_says_why() {
    let (_dir, mut s) = open(&cluttered());
    let h = s.drawing.doc.allocate_handle();
    s.drawing.doc.objects.insert(
        h,
        ObjectType::Unknown {
            type_name: "AECC_THING".into(),
            handle: h,
            owner: Handle::NULL,
            raw_dxf_codes: Some(vec![(340, "1F".into())]),
            raw_dwg_data: None,
            raw_dwg_handle_bits: 0,
            raw_dwg_version: None,
        },
    );
    let report = purge(&mut s, &[], true);
    assert_eq!(report.layers, vec!["OLD".to_string()]);
    assert!(report.blocks.is_empty() && report.styles.is_empty());
    assert!(report.linetypes.is_empty() && report.dimstyles.is_empty());
    assert!(report
        .limited_reason
        .as_deref()
        .unwrap()
        .contains("1 object"));
    // Layers alone are not limited.
    assert_eq!(
        purge(&mut s, &[PurgeKind::Layers], true).limited_reason,
        None
    );
    // The real run takes the layer only.
    purge(&mut s, &[], false);
    assert!(s.drawing.doc.layers.get("OLD").is_none());
    assert!(s.drawing.doc.block_records.get("SPARE").is_some());
}

#[test]
fn a_non_ascii_separator_and_an_arabic_layer_name_scan_safely() {
    let (_dir, mut s) = open(&cluttered());
    s.drawing.doc.header.dim_decimal_separator = 'Ä';
    layer(&mut s.drawing.doc, "جدار");
    let report = purge(&mut s, &[], true);
    assert!(report.layers.contains(&"جدار".to_string()));
    assert!(report.blocks.contains(&"SPARE".to_string()));
}

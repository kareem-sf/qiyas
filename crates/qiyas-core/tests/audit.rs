//! AUDIT: what in the drawing points at something it does not have (dangling handles, missing
//! layers, styles and blocks, gone group members), empty blocks and unreferenced layers; with
//! `fix` the safe repairs in one undo step.

use std::path::Path;

use opencadcodec::{CadDocument, EntityType, Handle};
use qiyas_core::audit::{AuditKind, AuditReport};
use qiyas_core::fixtures::{sample_doc, schedule_doc, write_drawing};
use qiyas_core::{Command, Outcome, Session, Target};

fn open(doc: &CadDocument, name: &str) -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = write_drawing(doc, &dir.path().join(name));
    let s = Session::open(&path).unwrap();
    (dir, s)
}

fn t(handle: u64) -> Target {
    Target {
        handle,
        chain: Vec::new(),
    }
}

fn audit(s: &mut Session, fix: bool) -> AuditReport {
    match s.run(Command::Audit { fix }).unwrap() {
        Outcome::Audited { report } => report,
        other => panic!("expected an audit report, got {other:?}"),
    }
}

fn count(report: &AuditReport, kind: AuditKind) -> usize {
    report.counts.get(&kind).copied().unwrap_or(0)
}

/// The kinds that are problems (not the empty blocks and unused layers it only lists).
const PROBLEMS: [AuditKind; 7] = [
    AuditKind::OwnerGone,
    AuditKind::MissingBlock,
    AuditKind::DimensionBlockGone,
    AuditKind::TableBlockGone,
    AuditKind::GroupMemberGone,
    AuditKind::MissingLayer,
    AuditKind::MissingTextStyle,
];

/// The handles of the model space entities on `layer`.
fn on_layer(s: &Session, layer: &str) -> Vec<u64> {
    s.drawing
        .doc
        .entities_in_block("*Model_Space")
        .filter(|e| e.common().layer == layer)
        .map(|e| e.common().handle.value())
        .collect()
}

fn layer_of(s: &Session, h: u64) -> String {
    s.drawing
        .doc
        .get_entity(Handle::new(h))
        .unwrap()
        .common()
        .layer
        .clone()
}

#[test]
fn audit_finds_an_entity_on_a_missing_layer() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let line = on_layer(&s, "A-WALL")[0];
    s.drawing
        .doc
        .get_entity_mut(Handle::new(line))
        .unwrap()
        .common_mut()
        .layer = "GHOST".into();
    let report = audit(&mut s, false);
    assert_eq!(count(&report, AuditKind::MissingLayer), 1);
    let item = report
        .items
        .iter()
        .find(|i| i.kind == AuditKind::MissingLayer)
        .unwrap();
    assert_eq!(item.name, "GHOST");
    assert_eq!(item.handles, vec![line]);
    assert!(!item.fixed);
    assert!(item.line.contains("GHOST"), "{}", item.line);
    assert_eq!(report.repaired, 0);
    // Only a report: nothing changed, no step.
    assert_eq!(layer_of(&s, line), "GHOST");
    assert!(!s.can_undo());
}

#[test]
fn a_clean_drawing_has_no_problems() {
    let dir = tempfile::tempdir().unwrap();
    let mut drawings = vec![
        write_drawing(&sample_doc(), &dir.path().join("sample.dxf")),
        write_drawing(&sample_doc(), &dir.path().join("sample.dwg")),
        write_drawing(&schedule_doc(), &dir.path().join("schedule.dxf")),
        write_drawing(&schedule_doc(), &dir.path().join("schedule.dwg")),
    ];
    let plan = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/test-plan.dwg");
    if plan.exists() {
        drawings.push(plan);
    }
    for path in drawings {
        let mut s = Session::open(&path).unwrap();
        let report = audit(&mut s, false);
        for kind in PROBLEMS {
            assert_eq!(
                count(&report, kind),
                0,
                "{} reports {kind:?}: {:?}",
                path.display(),
                report.items
            );
        }
    }
}

#[test]
fn audit_fix_is_one_step_and_lists_what_it_leaves() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let walls = on_layer(&s, "A-WALL");
    let title = on_layer(&s, "0")
        .into_iter()
        .find(|&h| {
            matches!(
                s.drawing.doc.get_entity(Handle::new(h)),
                Some(EntityType::Text(_))
            )
        })
        .unwrap();
    // A group, one of whose members is then erased.
    s.run(Command::CreateGroup {
        name: "PAIR".into(),
        targets: vec![t(walls[0]), t(walls[1])],
    })
    .unwrap();
    s.run(Command::DeleteEntities {
        targets: vec![t(walls[0])],
    })
    .unwrap();
    // A wall on a layer the drawing has no entry for, the title in a style it has none for, and
    // the DOOR block gone from under its three references and its leaf line.
    let doc = &mut s.drawing.doc;
    doc.get_entity_mut(Handle::new(walls[1]))
        .unwrap()
        .common_mut()
        .layer = "GHOST".into();
    if let Some(EntityType::Text(text)) = doc.get_entity_mut(Handle::new(title)) {
        text.style = "GONE".into();
    }
    doc.block_records.remove("DOOR").unwrap();

    let report = audit(&mut s, false);
    assert_eq!(count(&report, AuditKind::GroupMemberGone), 1);
    assert_eq!(count(&report, AuditKind::MissingLayer), 1);
    assert_eq!(count(&report, AuditKind::MissingTextStyle), 1);
    assert_eq!(count(&report, AuditKind::MissingBlock), 3);
    assert_eq!(count(&report, AuditKind::OwnerGone), 1, "DOOR's leaf line");

    let report = audit(&mut s, true);
    assert_eq!(report.repaired, 3, "{:?}", report.items);
    assert!(report
        .items
        .iter()
        .filter(|i| i.kind == AuditKind::MissingBlock)
        .all(|i| !i.fixed && i.name == "DOOR"));
    assert_eq!(layer_of(&s, walls[1]), "0");
    match s.drawing.doc.get_entity(Handle::new(title)) {
        Some(EntityType::Text(text)) => assert!(text.style.eq_ignore_ascii_case("Standard")),
        other => panic!("{other:?}"),
    }
    assert_eq!(s.info().groups[0].handles, vec![walls[1]]);
    let inserts = s
        .drawing
        .doc
        .entities_in_block("*Model_Space")
        .filter(|e| matches!(e, EntityType::Insert(_)))
        .count();
    assert_eq!(
        inserts, 3,
        "references to a missing block are listed, never deleted"
    );
    let after = audit(&mut s, false);
    for kind in [
        AuditKind::GroupMemberGone,
        AuditKind::MissingLayer,
        AuditKind::MissingTextStyle,
    ] {
        assert_eq!(count(&after, kind), 0, "{kind:?} repaired");
    }

    // One undo takes back every repair at once.
    assert!(s.undo());
    assert_eq!(layer_of(&s, walls[1]), "GHOST");
    match s.drawing.doc.get_entity(Handle::new(title)) {
        Some(EntityType::Text(text)) => assert_eq!(text.style, "GONE"),
        other => panic!("{other:?}"),
    }
    let group = s
        .drawing
        .doc
        .objects
        .values()
        .find_map(|o| match o {
            opencadcodec::objects::ObjectType::Group(g) => Some(g.entities.len()),
            _ => None,
        })
        .unwrap();
    assert_eq!(group, 2, "the gone member is listed again");
    assert!(s.redo());
    assert_eq!(layer_of(&s, walls[1]), "0");
}

#[test]
fn a_dimension_whose_block_is_gone_is_reported() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    let Outcome::Added { handles } = s
        .run(Command::Dimension(qiyas_core::DimSpec::Linear {
            p1: [0.0, 0.0],
            p2: [10.0, 0.0],
            line_point: [5.0, 2.0],
            rotation: None,
        }))
        .unwrap()
    else {
        panic!("a dimension adds an entity");
    };
    let block = match s.drawing.doc.get_entity(Handle::new(handles[0])) {
        Some(EntityType::Dimension(d)) => d.base().block_name.clone(),
        other => panic!("{other:?}"),
    };
    s.drawing.doc.block_records.remove(&block).unwrap();
    let report = audit(&mut s, false);
    assert_eq!(count(&report, AuditKind::DimensionBlockGone), 1);
    let item = report
        .items
        .iter()
        .find(|i| i.kind == AuditKind::DimensionBlockGone)
        .unwrap();
    assert_eq!(
        (item.name.as_str(), item.handles.as_slice()),
        (block.as_str(), &handles[..])
    );
}

#[test]
fn empty_blocks_and_unused_layers_are_listed() {
    let (_dir, mut s) = open(&sample_doc(), "plan.dxf");
    s.run(Command::NewLayer {
        name: "SPARE".into(),
    })
    .unwrap();
    let mut record = opencadcodec::BlockRecord::new("HOLLOW");
    record.handle = s.drawing.doc.allocate_handle();
    s.drawing.doc.block_records.add(record).unwrap();
    let report = audit(&mut s, false);
    let names = |kind| {
        report
            .items
            .iter()
            .filter(|i| i.kind == kind)
            .map(|i| i.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(AuditKind::EmptyBlock), vec!["HOLLOW".to_string()]);
    assert_eq!(names(AuditKind::UnusedLayer), vec!["SPARE".to_string()]);
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["counts"]["empty_block"], 1);
    assert_eq!(json["items"][0]["kind"], "empty_block");
}

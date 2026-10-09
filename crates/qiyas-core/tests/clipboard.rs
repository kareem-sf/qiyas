//! The clipboard: entities copied as a small DXF drawing (with the layers, blocks and styles they
//! use) and pasted into the same or another drawing with new handles, as one undo step.

use std::collections::HashSet;

use opencadcodec::{CadDocument, Color, EntityType, Handle, Line};
use qiyas_core::fixtures::{sample_doc, write_drawing, write_xref_pair};
use qiyas_core::{Command, DimSpec, Outcome, Session, Target, CLIPBOARD_MARK};

const MODEL: &str = "*Model_Space";

fn open(doc: &CadDocument, dir: &std::path::Path, name: &str) -> Session {
    Session::open(&write_drawing(doc, &dir.join(name))).unwrap()
}

fn t(handle: u64) -> Target {
    Target {
        handle,
        chain: Vec::new(),
    }
}

fn handles(s: &Session) -> HashSet<u64> {
    s.drawing
        .doc
        .entities_in_block(MODEL)
        .map(|e| e.common().handle.value())
        .collect()
}

fn on_layer(s: &Session, layer: &str) -> Vec<u64> {
    s.drawing
        .doc
        .entities_in_block(MODEL)
        .filter(|e| e.common().layer == layer)
        .map(|e| e.common().handle.value())
        .collect()
}

fn copy(s: &mut Session, targets: Vec<Target>, base: Option<[f64; 2]>) -> String {
    match s.run(Command::CopyToClipboard { targets, base }).unwrap() {
        Outcome::Clipboard { text, count } => {
            assert!(count > 0);
            text
        }
        other => panic!("{other:?}"),
    }
}

fn paste(s: &mut Session, at: [f64; 2], text: &str, as_block: bool) -> Vec<u64> {
    let cmd = Command::PasteFromClipboard {
        at,
        text: Some(text.to_string()),
        as_block,
        height: None,
    };
    match s.run(cmd).unwrap() {
        Outcome::Added { handles } => handles,
        other => panic!("{other:?}"),
    }
}

/// A drawing with layer 0 only and one line on it.
fn plain_doc() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = 6;
    doc.add_entity(EntityType::Line(Line::from_coords(
        50.0, 50.0, 0.0, 51.0, 50.0, 0.0,
    )))
    .unwrap();
    doc
}

#[test]
fn copy_from_one_drawing_and_paste_into_another_creates_the_layer_with_new_handles() {
    // Review Focus 5.
    let dir = tempfile::tempdir().unwrap();
    let mut a = open(&sample_doc(), dir.path(), "a.dwg");
    let mut b = open(&plain_doc(), dir.path(), "b.dwg");
    assert!(!b.drawing.doc.layers.contains("A-WALL"));
    let red = a.drawing.doc.layers.get("A-WALL").unwrap().color;
    // The sample's 10 m line on A-WALL, from (0, 0).
    let line = on_layer(&a, "A-WALL")[0];

    let text = copy(&mut a, vec![t(line)], Some([0.0, 0.0]));
    assert!(text.starts_with(CLIPBOARD_MARK));
    // Copying changes nothing.
    assert!(!a.can_undo());
    assert!(!a.is_dirty());

    let before = handles(&b);
    let pasted = paste(&mut b, [100.0, 200.0], &text, false);
    assert_eq!(pasted.len(), 1);
    // New handles: none the drawing had, and the copy is a line of B's own.
    assert!(!before.contains(&pasted[0]));
    let layer = b
        .drawing
        .doc
        .layers
        .get("A-WALL")
        .expect("the layer is created");
    assert_eq!(layer.color, red);
    assert_ne!(layer.color, Color::ByLayer);
    match b.drawing.doc.get_entity(Handle::new(pasted[0])) {
        Some(EntityType::Line(l)) => {
            assert_eq!(l.common.layer, "A-WALL");
            assert_eq!((l.start.x, l.start.y), (100.0, 200.0));
            assert_eq!((l.end.x, l.end.y), (110.0, 200.0));
        }
        other => panic!("{other:?}"),
    }
    // One undo step takes the entity and the layer back.
    assert!(b.is_dirty());
    assert!(b.undo());
    assert_eq!(handles(&b), before);
    assert!(!b.drawing.doc.layers.contains("A-WALL"));
    assert!(!b.can_undo());
}

#[test]
fn a_pasted_block_reference_brings_its_block_and_the_copy_saves() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = open(&sample_doc(), dir.path(), "a.dwg");
    let mut b = open(&plain_doc(), dir.path(), "b.dwg");
    let door = on_layer(&a, "A-DOOR")[0];
    // Without a base: the lower left of what is copied.
    let text = copy(&mut a, vec![t(door)], None);
    let pasted = paste(&mut b, [0.0, 0.0], &text, false);
    match b.drawing.doc.get_entity(Handle::new(pasted[0])) {
        Some(EntityType::Insert(i)) => {
            assert_eq!(i.block_name, "DOOR");
            // The door's leaf runs from its insertion point, so that is the lower left.
            assert!((i.insert_point.x).abs() < 1e-9 && (i.insert_point.y).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    assert!(b.drawing.doc.block_records.contains("DOOR"));
    assert!(b.drawing.doc.layers.contains("A-DOOR"));
    b.save().unwrap();
    let again = Session::open(&dir.path().join("b.dwg")).unwrap();
    assert!(again.drawing.doc.block_records.contains("DOOR"));
    assert_eq!(on_layer(&again, "A-DOOR").len(), 1);
}

#[test]
fn copy_and_paste_in_the_same_drawing_keeps_the_original_and_moves_a_dimension_with_its_block() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(&sample_doc(), dir.path(), "s.dwg");
    let dim = match s
        .run(Command::Dimension(DimSpec::Linear {
            p1: [0.0, -5.0],
            p2: [4.0, -5.0],
            line_point: [2.0, -6.0],
            rotation: None,
        }))
        .unwrap()
    {
        Outcome::Added { handles } => handles[0],
        other => panic!("{other:?}"),
    };
    let text = copy(&mut s, vec![t(dim)], Some([0.0, -5.0]));
    let pasted = paste(&mut s, [0.0, 95.0], &text, false);
    assert_eq!(pasted.len(), 1);
    assert_ne!(pasted[0], dim);
    assert!(s.drawing.doc.get_entity(Handle::new(dim)).is_some());
    let Some(EntityType::Dimension(d)) = s.drawing.doc.get_entity(Handle::new(pasted[0])) else {
        panic!("a dimension");
    };
    let block = d.base().block_name.clone();
    // Drawn 100 up, with its own block (not the original's).
    let EntityType::Dimension(original) = s.drawing.doc.get_entity(Handle::new(dim)).unwrap()
    else {
        unreachable!()
    };
    assert_ne!(block, original.base().block_name);
    let bounds = s.batch().bounds;
    assert!(bounds[1][1] > 94.0, "{bounds:?}");
}

#[test]
fn paste_as_a_block_makes_one_reference() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = open(&sample_doc(), dir.path(), "a.dwg");
    let walls = on_layer(&a, "A-WALL");
    let text = copy(
        &mut a,
        walls.iter().map(|&h| t(h)).collect(),
        Some([0.0, 0.0]),
    );
    let mut b = open(&plain_doc(), dir.path(), "b.dwg");
    let pasted = paste(&mut b, [10.0, 10.0], &text, true);
    assert_eq!(pasted.len(), 1);
    let Some(EntityType::Insert(i)) = b.drawing.doc.get_entity(Handle::new(pasted[0])) else {
        panic!("a block reference");
    };
    assert!(i.block_name.starts_with("A$C"), "{}", i.block_name);
    assert_eq!(b.drawing.doc.entities_in_block(&i.block_name).count(), 3);
    assert!(b.drawing.doc.layers.contains("A-WALL"));
    assert!(b.undo());
    assert!(b.drawing.doc.get_entity(Handle::new(pasted[0])).is_none());
}

#[test]
fn plain_text_pastes_as_a_text_at_the_point() {
    let dir = tempfile::tempdir().unwrap();
    let mut b = open(&plain_doc(), dir.path(), "b.dwg");
    let pasted = paste(&mut b, [3.0, 4.0], "Room 101\n", false);
    match b.drawing.doc.get_entity(Handle::new(pasted[0])) {
        Some(EntityType::Text(text)) => {
            assert_eq!(text.value, "Room 101");
            assert_eq!((text.insertion_point.x, text.insertion_point.y), (3.0, 4.0));
            // 2.5 mm in a metre drawing at 1:1.
            assert!((text.height - 0.0025).abs() < 1e-12);
        }
        other => panic!("{other:?}"),
    }
    // Several lines: an MText.
    let pasted = paste(&mut b, [3.0, 4.0], "Line one\nLine two", false);
    assert!(matches!(
        b.drawing.doc.get_entity(Handle::new(pasted[0])),
        Some(EntityType::MText(_))
    ));
    // Nothing on the clipboard.
    let empty = Command::PasteFromClipboard {
        at: [0.0, 0.0],
        text: None,
        as_block: false,
        height: None,
    };
    assert!(b.run(empty).is_err());
}

#[test]
fn an_xref_reference_stays_out_of_the_clipboard() {
    let dir = tempfile::tempdir().unwrap();
    let host = write_xref_pair(dir.path());
    let mut s = Session::open(&host).unwrap();
    let doc = &s.drawing.doc;
    let xref = doc
        .entities_in_block(MODEL)
        .find(|e| match e {
            EntityType::Insert(i) => i.block_name == "plan-arch",
            _ => false,
        })
        .map(|e| e.common().handle.value())
        .expect("the host references an xref");
    let others: Vec<u64> = doc
        .entities_in_block(MODEL)
        .map(|e| e.common().handle.value())
        .filter(|&h| h != xref)
        .collect();
    assert!(s
        .run(Command::CopyToClipboard {
            targets: vec![t(xref)],
            base: None,
        })
        .is_err());
    let other = others[0];
    {
        match s
            .run(Command::CopyToClipboard {
                targets: vec![t(xref), t(other)],
                base: None,
            })
            .unwrap()
        {
            Outcome::Clipboard { count, .. } => assert_eq!(count, 1),
            o => panic!("{o:?}"),
        }
    }
}

/// A drawing whose blocks D0, D1, ... D{depth - 1} each hold a reference to the next (the last
/// a line), with a reference to D0 in model space.
fn deep_doc(depth: usize) -> CadDocument {
    use opencadcodec::entities::Insert;
    use opencadcodec::{BlockRecord, Vector3};
    let mut doc = plain_doc();
    for i in 0..depth {
        let mut block = BlockRecord::new(format!("D{i}"));
        block.handle = doc.allocate_handle();
        let owner = block.handle;
        doc.block_records.add(block).unwrap();
        let mut e = if i + 1 < depth {
            EntityType::Insert(Insert::new(format!("D{}", i + 1), Vector3::ZERO))
        } else {
            EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0))
        };
        e.common_mut().owner_handle = owner;
        doc.add_entity(e).unwrap();
    }
    doc.add_entity(EntityType::Insert(Insert::new("D0", Vector3::ZERO)))
        .unwrap();
    doc
}

#[test]
fn blocks_nested_too_deep_and_oversized_pastes_are_refused_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(&plain_doc(), dir.path(), "target.dxf");
    let before = (handles(&s), s.drawing.doc.block_records.len());
    let refused = |s: &mut Session, cmd: Command| {
        let why = match s.run(cmd) {
            Err(qiyas_core::CoreError::InvalidCommand(why)) => why,
            other => panic!("{other:?}"),
        };
        assert_eq!((handles(s), s.drawing.doc.block_records.len()), before);
        why
    };
    // Design Center: a block 40 deep from another drawing.
    let deep = write_drawing(&deep_doc(40), &dir.path().join("deep.dxf"));
    let why = refused(
        &mut s,
        Command::InsertBlock {
            name: "D0".into(),
            at: [0.0, 0.0],
            scale: 1.0,
            rotation: 0.0,
            attribute_values: Default::default(),
            from_path: Some(deep.display().to_string()),
        },
    );
    assert!(why.contains("32 deep"), "{why}");
    // Copying it is refused the same way; a clipboard drawing holding it, pasted, too.
    let mut other = Session::open(&deep).unwrap();
    let top: Vec<Target> = other
        .drawing
        .doc
        .entities_in_block(MODEL)
        .filter(|e| matches!(e, EntityType::Insert(_)))
        .map(|e| t(e.common().handle.value()))
        .collect();
    let copying = other.run(Command::CopyToClipboard {
        targets: top,
        base: None,
    });
    assert!(copying.is_err(), "{copying:?}");
    let bytes = opencadcodec::DxfWriter::new(&deep_doc(40))
        .write_to_vec()
        .unwrap();
    let text = format!("{CLIPBOARD_MARK}{}", String::from_utf8_lossy(&bytes));
    let paste = |text: String| Command::PasteFromClipboard {
        at: [0.0, 0.0],
        text: Some(text),
        as_block: false,
        height: None,
    };
    let why = refused(&mut s, paste(text));
    assert!(why.contains("32 deep"), "{why}");
    // 32 deep is fine.
    let ok = write_drawing(&deep_doc(32), &dir.path().join("ok.dxf"));
    let mut fine = Session::open(&ok).unwrap();
    let top: Vec<Target> = fine
        .drawing
        .doc
        .entities_in_block(MODEL)
        .map(|e| t(e.common().handle.value()))
        .collect();
    let text = copy(&mut fine, top, Some([0.0, 0.0]));
    assert!(!paste_ok(&mut s, text).is_empty());
    let before = (handles(&s), s.drawing.doc.block_records.len());
    let refused = |s: &mut Session, cmd: Command| {
        let why = match s.run(cmd) {
            Err(qiyas_core::CoreError::InvalidCommand(why)) => why,
            other => panic!("{other:?}"),
        };
        assert_eq!((handles(s), s.drawing.doc.block_records.len()), before);
        why
    };
    // Over 32 MB of clipboard drawing, over 64 KB of plain text.
    let huge = format!("{CLIPBOARD_MARK}{}", " ".repeat(33 << 20));
    assert!(refused(&mut s, paste(huge)).contains("32 MB"));
    let long = "x".repeat(65 << 10);
    assert!(refused(&mut s, paste(long)).contains("64 KB"));
}

fn paste_ok(s: &mut Session, text: String) -> Vec<u64> {
    paste(s, [0.0, 0.0], &text, false)
}

#[test]
fn a_paste_from_a_millimetre_drawing_into_a_metre_one_is_scaled() {
    let dir = tempfile::tempdir().unwrap();
    let mut mm = CadDocument::new();
    mm.header.insertion_units = 4;
    mm.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 1000.0, 0.0, 0.0,
    )))
    .unwrap();
    let mut from = open(&mm, dir.path(), "mm.dxf");
    let line: Vec<Target> = handles(&from).into_iter().map(t).collect();
    let text = copy(&mut from, line, Some([0.0, 0.0]));
    let mut to = open(&plain_doc(), dir.path(), "m.dxf");
    let pasted = paste(&mut to, [5.0, 5.0], &text, false);
    let EntityType::Line(l) = to.drawing.doc.get_entity(Handle::new(pasted[0])).unwrap() else {
        panic!("a line");
    };
    let ends = [l.start.x, l.start.y, l.end.x, l.end.y];
    for (got, want) in ends.iter().zip([5.0, 5.0, 6.0, 5.0]) {
        assert!((got - want).abs() < 1e-9, "{ends:?}");
    }
    // As a block: one reference at a thousandth.
    let block = paste(&mut to, [0.0, 0.0], &text, true);
    let EntityType::Insert(i) = to.drawing.doc.get_entity(Handle::new(block[0])).unwrap() else {
        panic!("a reference");
    };
    assert!((i.x_scale() - 0.001).abs() < 1e-12, "{}", i.x_scale());
}

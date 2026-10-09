//! The clipboard: entities copied as a small DXF drawing (with the layers, blocks and styles they
//! use) and pasted into the same or another drawing with new handles, as one undo step.

use std::collections::HashSet;

use opencadcodec::{CadDocument, Color, EntityType, Handle, Line};
use qiyas_core::fixtures::{sample_doc, write_drawing};
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

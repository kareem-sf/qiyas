//! Drawing edits through the command bus: each is one undo step made of the touched entities'
//! before images, and moves the document revision on (so the batch is drawn again).

use opencadcodec::entities::Insert;
use opencadcodec::{BlockRecord, CadDocument, EntityType, Handle, Line, Vector3};
use qiyas_core::fixtures::{write_drawing, write_png, write_sample, write_xref_pair};
use qiyas_core::{Command, CoreError, Outcome, Session, Source};

const MODEL: &str = "*Model_Space";

/// The sample drawing (DXF) in a session; the temp dir must outlive the test body.
fn sample_session() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let session = Session::open(&write_sample(dir.path(), "dxf")).unwrap();
    (dir, session)
}

/// Handles of the sample's 10 m line and its arc, both on A-WALL.
fn line_and_arc(session: &Session) -> (u64, u64) {
    let find = |want: fn(&EntityType) -> bool| {
        session
            .drawing
            .doc
            .entities_in_block(MODEL)
            .find(|e| want(e))
            .map(|e| e.common().handle.value())
            .unwrap()
    };
    (
        find(|e| matches!(e, EntityType::Line(_))),
        find(|e| matches!(e, EntityType::Arc(_))),
    )
}

/// The entity `handle` is in the drawing under that handle, and the batch draws it.
fn present(session: &Session, handle: u64) -> bool {
    let in_doc = session
        .drawing
        .doc
        .get_entity(Handle::new(handle))
        .is_some_and(|e| e.common().handle.value() == handle);
    let in_sheet = session
        .drawing
        .doc
        .entities_in_block(MODEL)
        .any(|e| e.common().handle.value() == handle);
    let drawn = session.batch().ranges.iter().any(|r| r.entity == handle);
    assert_eq!(
        in_doc, in_sheet,
        "the sheet and the store agree on {handle}"
    );
    assert_eq!(in_doc, drawn, "the batch follows the drawing for {handle}");
    in_doc
}

fn delete(handles: &[u64]) -> Command {
    Command::DeleteEntities {
        handles: handles.to_vec(),
    }
}

#[test]
fn deleting_two_entities_is_one_step_and_undo_brings_them_back_with_their_handles() {
    let (_dir, mut s) = sample_session();
    let (line, arc) = line_and_arc(&s);
    let drawn = s.batch().ranges.len();
    assert!(present(&s, line) && present(&s, arc));

    let outcome = s.run(delete(&[line, arc])).unwrap();
    assert!(matches!(outcome, Outcome::Changed), "{outcome:?}");
    assert!(!present(&s, line) && !present(&s, arc));
    assert_eq!(s.batch().ranges.len(), drawn - 2);

    // One step takes both back, under the same handles.
    assert!(s.undo());
    assert!(present(&s, line) && present(&s, arc));
    assert_eq!(s.batch().ranges.len(), drawn);
    assert!(!s.can_undo());

    // Redo removes both again; undo once more and they are back again, same handles.
    assert!(s.redo());
    assert!(!present(&s, line) && !present(&s, arc));
    assert!(s.undo());
    assert!(present(&s, line) && present(&s, arc));
    // Restored, not duplicated: the sheet lists each handle once.
    let listed = s
        .drawing
        .doc
        .entities_in_block(MODEL)
        .filter(|e| e.common().handle.value() == line)
        .count();
    assert_eq!(listed, 1);
}

#[test]
fn every_document_step_moves_the_revision_on_and_marks_the_drawing_unsaved() {
    let (_dir, mut s) = sample_session();
    let (line, _) = line_and_arc(&s);
    assert!(!s.is_dirty());
    let v0 = s.batch_version();

    s.run(delete(&[line])).unwrap();
    let v1 = s.batch_version();
    assert_ne!(v1, v0);
    assert!(s.is_dirty());

    assert!(s.undo());
    let v2 = s.batch_version();
    assert!(v2 != v1 && v2 != v0, "undo is a new revision too");
    assert!(s.redo());
    assert!(s.batch_version() != v2 && s.batch_version() != v1);

    s.save().unwrap();
    assert!(!s.is_dirty());
    assert!(!present(&s, line), "saved without the deleted line");
}

#[test]
fn a_saved_deletion_stays_deleted_on_reopen() {
    for ext in ["dxf", "dwg"] {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path(), ext);
        let mut s = Session::open(&path).unwrap();
        let (line, arc) = line_and_arc(&s);
        s.run(delete(&[line])).unwrap();
        s.save().unwrap();
        let reopened = Session::open(&path).unwrap();
        assert!(!present(&reopened, line), "{ext}");
        assert!(present(&reopened, arc), "{ext}");
    }
}

#[test]
fn unknown_or_xref_handles_are_refused_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::open(&write_xref_pair(dir.path())).unwrap();
    let own = s
        .drawing
        .doc
        .entities_in_block(MODEL)
        .find(|e| matches!(e, EntityType::Line(_)))
        .unwrap()
        .common()
        .handle
        .value();
    // A handle of the xref drawing that the host does not have.
    let xref_only = {
        let xdoc = &s.xrefs.entries[0].drawing.as_ref().unwrap().doc;
        xdoc.entities_in_block(MODEL)
            .map(|e| e.common().handle.value())
            .find(|h| s.drawing.doc.get_entity(Handle::new(*h)).is_none())
            .expect("the xref has a handle the host lacks")
    };
    let version = s.batch_version();

    let err = s.run(delete(&[own, xref_only])).unwrap_err();
    assert!(
        matches!(&err, CoreError::InvalidCommand(m) if m.contains("plan-arch")),
        "{err:?}"
    );
    let err = s.run(delete(&[own, 0xDEAD_BEEF])).unwrap_err();
    assert!(
        matches!(err, CoreError::NoSuchEntity(0xDEAD_BEEF)),
        "{err:?}"
    );
    let err = s.run(delete(&[])).unwrap_err();
    assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");

    // All or nothing: the host's own line is still there, and nothing is an undo step.
    assert!(present(&s, own));
    assert!(!s.can_undo());
    assert_eq!(s.batch_version(), version);
    assert!(!s.is_dirty());
}

#[test]
fn entities_inside_a_block_or_on_another_sheet_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = CadDocument::new();
    let mut block = BlockRecord::new("BOX");
    block.handle = doc.allocate_handle();
    block.block_entity_handle = doc.allocate_handle();
    block.block_end_handle = doc.allocate_handle();
    doc.block_records.add(block).unwrap();
    let mut inner = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    inner.common_mut().owner_handle = doc.block_records.get("BOX").unwrap().handle;
    let inner = doc.add_entity(inner).unwrap().value();
    doc.add_entity(EntityType::Insert(Insert::new(
        "BOX",
        Vector3::new(0.0, 0.0, 0.0),
    )))
    .unwrap();
    let paper = doc
        .add_paper_space_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 2.0, 0.0, 0.0,
        )))
        .unwrap()
        .value();
    let mut s = Session::open(&write_drawing(&doc, &dir.path().join("blocks.dxf"))).unwrap();

    for handle in [inner, paper] {
        let err = s.run(delete(&[handle])).unwrap_err();
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert!(s.drawing.doc.get_entity(Handle::new(handle)).is_some());
    }
    assert!(!s.can_undo());
}

#[test]
fn take_off_and_drawing_steps_undo_and_redo_in_order() {
    let (_dir, mut s) = sample_session();
    let (line, arc) = line_and_arc(&s);
    let measured = |s: &Session| s.takeoff.items.len();

    s.run(Command::MeasureDistance {
        points: vec![[0.0, 0.0], [3.0, 4.0]],
    })
    .unwrap();
    s.run(delete(&[line])).unwrap();
    s.run(Command::MeasureArea {
        points: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    })
    .unwrap();
    s.run(delete(&[arc])).unwrap();
    assert_eq!(measured(&s), 2);

    assert!(s.undo());
    assert!(present(&s, arc) && !present(&s, line));
    assert_eq!(measured(&s), 2);
    assert!(s.undo());
    assert_eq!(measured(&s), 1);
    assert!(!present(&s, line));
    assert!(s.undo());
    assert!(present(&s, line));
    assert_eq!(measured(&s), 1);
    assert!(s.undo());
    assert_eq!(measured(&s), 0);
    assert!(!s.undo());

    assert!(s.redo() && s.redo());
    assert_eq!(measured(&s), 1);
    assert!(!present(&s, line) && present(&s, arc));
    assert!(s.redo() && s.redo());
    assert_eq!(measured(&s), 2);
    assert!(!present(&s, arc));
    assert!(!s.redo());
}

#[test]
fn the_ai_undoes_only_its_own_deletion() {
    let (_dir, mut s) = sample_session();
    let (line, _) = line_and_arc(&s);
    s.run_as(delete(&[line]), Source::Ai).unwrap();
    assert_eq!(s.top_step_source(), Some(Source::Ai));
    s.run(Command::MeasureDistance {
        points: vec![[0.0, 0.0], [1.0, 0.0]],
    })
    .unwrap();
    assert!(!s.undo_if(Source::Ai), "the user's step is on top");
    assert!(!present(&s, line));
    assert!(s.undo());
    assert!(s.undo_if(Source::Ai));
    assert!(present(&s, line));
}

#[test]
fn at_most_200_drawing_steps_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = CadDocument::new();
    let handles: Vec<u64> = (0..205)
        .map(|i| {
            let y = f64::from(i);
            doc.add_entity(EntityType::Line(Line::from_coords(
                0.0, y, 0.0, 1.0, y, 0.0,
            )))
            .unwrap()
            .value()
        })
        .collect();
    let mut s = Session::open(&write_drawing(&doc, &dir.path().join("lines.dxf"))).unwrap();
    for &h in &handles {
        s.run(delete(&[h])).unwrap();
    }
    let undone = std::iter::from_fn(|| s.undo().then_some(())).count();
    assert_eq!(undone, 200);
    // The five oldest deletions can no longer be undone; the rest are back.
    assert!(handles[..5].iter().all(|&h| !present(&s, h)));
    assert!(handles[5..].iter().all(|&h| present(&s, h)));
}

#[test]
fn an_attached_image_undoes_with_its_definition_and_redoes_with_its_handle() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::open(&write_sample(dir.path(), "dxf")).unwrap();
    let objects = s.drawing.doc.objects.len();
    let png = write_png(dir.path(), "pic.png");
    let Outcome::Added { handles } = s
        .run(Command::AttachImage {
            path: png.display().to_string(),
            insert_at: [0.0, 0.0],
            width_units: 10.0,
        })
        .unwrap()
    else {
        panic!("an image is added");
    };
    let image = handles[0];
    assert!(s.drawing.doc.objects.len() > objects);

    assert!(s.undo());
    assert!(s.drawing.doc.get_entity(Handle::new(image)).is_none());
    assert_eq!(
        s.drawing.doc.objects.len(),
        objects,
        "definition and reactor gone"
    );
    assert!(s.redo());
    let back = s.drawing.doc.get_entity(Handle::new(image)).unwrap();
    assert!(matches!(back, EntityType::RasterImage(_)));
    assert!(s.batch().ranges.iter().any(|r| r.entity == image));
}

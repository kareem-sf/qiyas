//! The user coordinate system: view state of the session (not an undo step), kept in the project
//! file and written to the drawing's header and UCS table on save so AutoCAD agrees.

use opencadcodec::{EntityType, Handle};
use qiyas_core::fixtures::write_sample;
use qiyas_core::{Command, Session, Target, Ucs};

fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

#[test]
fn a_ucs_is_kept_in_the_project_and_written_to_the_drawing_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    let mut s = Session::open(&path).unwrap();
    assert!(s.ucs.is_world());

    s.run(Command::Ucs {
        origin: [10.0, 5.0],
        x_axis: [0.0, 2.0],
    })
    .unwrap();
    assert_eq!(s.info().ucs, s.ucs);
    // A turn of a quarter, the axis made a unit vector.
    assert!(close(s.ucs.x_axis, [0.0, 1.0]));
    assert!(close(s.ucs.to_ucs([10.0, 6.0]), [1.0, 0.0]));
    assert!(close(s.ucs.to_world([1.0, 2.0]), [8.0, 6.0]));
    // View state: no undo step, the drawing unchanged.
    assert!(!s.can_undo());
    assert!(!s.is_dirty());

    s.save_project().unwrap();
    let again = Session::open(&path).unwrap();
    assert_eq!(again.ucs, s.ucs);

    s.save().unwrap();
    let doc = &Session::open(&path).unwrap().drawing.doc;
    let h = &doc.header;
    assert_eq!(
        (h.model_space_ucs_origin.x, h.model_space_ucs_origin.y),
        (10.0, 5.0)
    );
    assert!((h.model_space_ucs_x_axis.y - 1.0).abs() < 1e-9);
    assert!((h.model_space_ucs_y_axis.x + 1.0).abs() < 1e-9);
    let named = doc.ucss.get("QIYAS").expect("the UCS table entry");
    assert_eq!((named.origin.x, named.origin.y), (10.0, 5.0));

    // A zero axis is refused.
    assert!(s
        .run(Command::Ucs {
            origin: [0.0, 0.0],
            x_axis: [0.0, 0.0],
        })
        .is_err());
}

#[test]
fn a_drawing_without_a_project_takes_the_ucs_its_header_keeps() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dwg");
    let mut s = Session::open(&path).unwrap();
    s.run(Command::Ucs {
        origin: [1.0, 2.0],
        x_axis: [1.0, 1.0],
    })
    .unwrap();
    s.save().unwrap();
    std::fs::remove_file(qiyas_core::project_path(&path)).unwrap();
    let again = Session::open(&path).unwrap();
    assert!(close(again.ucs.origin, [1.0, 2.0]));
    let d = std::f64::consts::FRAC_1_SQRT_2;
    assert!(close(again.ucs.x_axis, [d, d]));
}

#[test]
fn ucs_object_follows_a_line_and_world_goes_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_sample(dir.path(), "dxf");
    let mut s = Session::open(&path).unwrap();
    // The DOOR turned a quarter at (5, 1).
    let turned = s
        .drawing
        .doc
        .entities_in_block("*Model_Space")
        .filter(|e| matches!(e, EntityType::Insert(_)))
        .nth(1)
        .unwrap()
        .common()
        .handle
        .value();
    s.run(Command::UcsObject {
        target: Target {
            handle: turned,
            chain: Vec::new(),
        },
    })
    .unwrap();
    assert!(close(s.ucs.origin, [5.0, 1.0]));
    assert!(close(s.ucs.x_axis, [0.0, 1.0]));
    assert!(!s.can_undo());

    s.run(Command::Ucs {
        origin: [0.0, 0.0],
        x_axis: [1.0, 0.0],
    })
    .unwrap();
    assert_eq!(s.ucs, Ucs::default());
    assert!(s.drawing.doc.get_entity(Handle::new(turned)).is_some());
}

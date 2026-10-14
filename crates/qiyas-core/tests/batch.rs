//! `Command::Batch`: drawing commands as one undo step, with the guards `run_as` gives each.

use opencadcodec::EntityType;
use qiyas_core::{Command, Session, Target};

fn session() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    (dir, Session::open(&path).unwrap())
}

fn insert_door() -> Command {
    Command::InsertBlock {
        name: "DOOR".into(),
        at: [0.0, 0.0],
        scale: 1.0,
        rotation: 0.0,
        attribute_values: Default::default(),
        from_path: None,
    }
}

#[test]
fn a_batch_obeys_the_block_edit_guards_for_each_command() {
    let (_dir, mut s) = session();
    let insert = s
        .drawing
        .entities_in("*Model_Space")
        .find(|e| matches!(e, EntityType::Insert(_)))
        .unwrap()
        .common()
        .handle
        .value();
    s.run(Command::EditBlockInPlace {
        target: Target {
            handle: insert,
            chain: vec![],
        },
    })
    .unwrap();
    let before = s.drawing.entities_in("*Model_Space").count();
    // The block being edited can't be put into itself, alone or in a batch.
    assert!(s.run(insert_door()).is_err());
    let err = s
        .run(Command::Batch {
            commands: vec![insert_door()],
        })
        .unwrap_err();
    assert!(err.to_string().contains("DOOR"), "{err}");
    assert_eq!(s.drawing.entities_in("*Model_Space").count(), before);
    s.run(Command::SaveBlockEdits).unwrap();
    assert!(
        !s.drawing
            .entities_in("DOOR")
            .any(|e| matches!(e, EntityType::Insert(i) if i.block_name == "DOOR")),
        "DOOR must not hold a reference to DOOR"
    );
}

#[test]
fn a_failed_batch_puts_back_the_layer_and_the_current_state_and_undo_restores_them() {
    let (_dir, mut s) = session();
    let current = s.current.clone();
    let mut other = current.clone();
    other.layer = "ZZ".into();
    let failed = s.run(Command::Batch {
        commands: vec![
            Command::NewLayer { name: "ZZ".into() },
            Command::SetCurrent { current: other },
            Command::InsertBlock {
                name: "NOPE".into(),
                at: [0.0, 0.0],
                scale: 1.0,
                rotation: 0.0,
                attribute_values: Default::default(),
                from_path: None,
            },
        ],
    });
    assert!(failed.is_err());
    assert_eq!(s.current, current);
    assert!(s.drawing.doc.layers.get("ZZ").is_none());

    let mut other = current.clone();
    other.layer = "ZY".into();
    s.run(Command::Batch {
        commands: vec![
            Command::NewLayer { name: "ZY".into() },
            Command::SetCurrent { current: other },
        ],
    })
    .unwrap();
    assert_eq!(s.current.layer, "ZY");
    assert!(s.undo());
    assert_eq!(s.current, current);
    assert!(s.drawing.doc.layers.get("ZY").is_none());
    assert!(s.redo());
    assert_eq!(s.current.layer, "ZY");
}

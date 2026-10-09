use opencadcodec::{CadDocument, EntityType, Line};
use qiyas_core::{Command, Outcome, Session, EMPTY_MODEL};

#[test]
fn views_round_trip_through_dxf() {
    for ext in ["dxf", "dwg"] {
        let dir = tempfile::tempdir().unwrap();
        let path = qiyas_core::fixtures::write_sample(dir.path(), ext);
        let mut session = Session::open(&path).unwrap();
        assert!(session.views().is_empty());
        let bounds = [[1.0, 2.0], [11.0, 7.5]];
        let save = Command::SaveView {
            name: "Kitchen".into(),
            bounds,
        };
        assert!(matches!(session.run(save).unwrap(), Outcome::Changed));
        assert!(session.is_dirty(), "a view is a drawing change");
        // The same name in another case replaces it.
        let wider = [[0.0, 0.0], [20.0, 10.0]];
        let again = Command::SaveView {
            name: "KITCHEN".into(),
            bounds: wider,
        };
        session.run(again).unwrap();
        assert_eq!(session.views().len(), 1);
        assert!(session.undo());
        assert_eq!(session.views()[0].bounds, bounds);

        let out = dir.path().join(format!("out.{ext}"));
        session.save_as(&out).unwrap();
        let reopened = Session::open(&out).unwrap();
        let views = reopened.views();
        assert_eq!(views.len(), 1, "{ext}: {views:?}");
        assert_eq!(views[0].name, "Kitchen");
        for (a, b) in views[0]
            .bounds
            .iter()
            .flatten()
            .zip(bounds.iter().flatten())
        {
            assert!((a - b).abs() < 1e-9, "{ext}: {:?}", views[0].bounds);
        }
    }
}

#[test]
fn views_are_deleted_and_checked() {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let mut session = Session::open(&path).unwrap();
    let save = |name: &str, bounds| Command::SaveView {
        name: name.into(),
        bounds,
    };
    assert!(session.run(save(" ", [[0.0, 0.0], [1.0, 1.0]])).is_err());
    assert!(session.run(save("Flat", [[0.0, 0.0], [1.0, 0.0]])).is_err());
    session.run(save("A", [[0.0, 0.0], [1.0, 1.0]])).unwrap();
    let delete = |name: &str| Command::DeleteView { name: name.into() };
    assert!(matches!(
        session.run(delete("none")).unwrap(),
        Outcome::Nothing
    ));
    assert!(matches!(
        session.run(delete("a")).unwrap(),
        Outcome::Changed
    ));
    assert!(session.views().is_empty());
    assert!(session.undo());
    assert_eq!(session.views().len(), 1);
}

#[test]
fn empty_drawing_warns() {
    let dir = tempfile::tempdir().unwrap();
    // Nothing in model space.
    let doc = CadDocument::new();
    let path = qiyas_core::fixtures::write_drawing(&doc, &dir.path().join("empty.dxf"));
    let session = Session::open(&path).unwrap();
    assert!(session.info().problems.contains(&EMPTY_MODEL.to_string()));

    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
    )))
    .unwrap();
    let path = qiyas_core::fixtures::write_drawing(&doc, &dir.path().join("one.dxf"));
    let session = Session::open(&path).unwrap();
    assert!(!session.info().problems.contains(&EMPTY_MODEL.to_string()));
}

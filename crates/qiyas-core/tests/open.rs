use qiyas_core::{CoreError, Drawing, Units};

#[test]
fn opens_sample_dxf_and_dwg() {
    for ext in ["dxf", "dwg"] {
        let dir = tempfile::tempdir().unwrap();
        let path = qiyas_core::fixtures::write_sample(dir.path(), ext);
        let d = Drawing::open(&path).unwrap();
        assert_eq!(d.units, Units::Metres);
        assert_eq!(d.sheets()[0].name, "Model");
        let names: Vec<_> = d.layers().into_iter().map(|l| l.name).collect();
        assert!(names.contains(&"A-WALL".to_string()) && names.contains(&"A-DOOR".to_string()));
        assert_eq!(d.entities_in("*Model_Space").count(), 7);
    }
}

#[test]
fn unreadable_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("junk.dwg");
    std::fs::write(&p, b"not a drawing").unwrap();
    assert!(matches!(Drawing::open(&p), Err(CoreError::Unreadable(_))));
}

#[test]
fn truncated_dwg_opens_with_problems() {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dwg");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() * 7 / 10]).unwrap();
    match Drawing::open(&path) {
        Ok(d) => assert!(!d.problems.is_empty(), "a cut file must report problems"),
        Err(CoreError::Unreadable(_)) => {}
        Err(other) => panic!("expected Unreadable, got {other:?}"),
    }
}

#[test]
fn units_table() {
    assert_eq!(Units::from_header(4), Units::Millimetres);
    assert_eq!(Units::from_header(0), Units::Unknown);
    assert!((Units::Feet.to_metres() - 0.3048).abs() < 1e-9);
}

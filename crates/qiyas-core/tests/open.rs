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
    // Cut each sample at several points. Every cut must open with problems or be Unreadable, never panic,
    // and at least one cut must recover (open with problems).
    let mut recovered = Vec::new();
    for ext in ["dwg", "dxf"] {
        for percent in [95, 90, 85, 80, 70] {
            let dir = tempfile::tempdir().unwrap();
            let path = qiyas_core::fixtures::write_sample(dir.path(), ext);
            let bytes = std::fs::read(&path).unwrap();
            std::fs::write(&path, &bytes[..bytes.len() * percent / 100]).unwrap();
            match Drawing::open(&path) {
                Ok(d) => {
                    assert!(
                        !d.problems.is_empty(),
                        "{ext} cut to {percent}% opened with no problems"
                    );
                    recovered.push(format!("{ext} {percent}%"));
                }
                Err(CoreError::Unreadable(_)) => {}
                Err(other) => panic!("{ext} cut to {percent}%: expected Unreadable, got {other:?}"),
            }
        }
    }
    assert!(!recovered.is_empty(), "no cut file opened with problems");
}

#[test]
fn units_table() {
    assert_eq!(Units::from_header(4), Units::Millimetres);
    assert_eq!(Units::from_header(0), Units::Unknown);
    assert!((Units::Feet.to_metres() - 0.3048).abs() < 1e-9);
}

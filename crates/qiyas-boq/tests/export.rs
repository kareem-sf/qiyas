use calamine::{open_workbook, Data, Reader, Xlsx};
use qiyas_core::{Kind, Measurement, Takeoff, Units};

fn item(kind: Kind, value: f64, description: &str) -> Measurement {
    Measurement::new(
        kind,
        value,
        Units::Metres,
        description.to_string(),
        "A-WALL".to_string(),
        "*Model_Space".to_string(),
    )
}

#[test]
fn export_is_readable_by_calamine() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("villa-takeoff.xlsx");
    let mut takeoff = Takeoff::default();
    takeoff.add(item(Kind::Area, 12.0, "Slab"));
    takeoff.add(item(Kind::Length, 7.0, "Wall"));
    takeoff.add(item(Kind::Count, 3.0, "Doors"));

    qiyas_boq::export_takeoff(
        &path,
        &takeoff.items,
        &takeoff.totals(),
        "villa.dwg take-off",
    )
    .unwrap();

    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let sheet = book.worksheet_range("Take-off").unwrap();
    let header: Vec<_> = (0..8)
        .map(|c| sheet.get((0, c)).unwrap().to_string())
        .collect();
    assert_eq!(
        header,
        [
            "Description",
            "Kind",
            "Quantity",
            "Unit",
            "Layer",
            "Sheet",
            "Note",
            "Id"
        ]
    );
    assert_eq!(sheet.get((1, 0)), Some(&Data::String("Slab".into())));
    assert_eq!(sheet.get((1, 1)), Some(&Data::String("area".into())));
    assert_eq!(sheet.get((1, 2)), Some(&Data::Float(12.0)));
    assert_eq!(
        sheet.get((1, 7)),
        Some(&Data::String(takeoff.items[0].id.clone()))
    );
    assert_eq!(sheet.get((2, 2)), Some(&Data::Float(7.0)));
    assert_eq!(sheet.get((3, 1)), Some(&Data::String("count".into())));
    assert_eq!(sheet.get((3, 2)), Some(&Data::Float(3.0)));
    // A blank row, then the totals block.
    assert!(sheet.get((4, 0)).is_none_or(|d| *d == Data::Empty));
    assert_eq!(sheet.get((5, 0)), Some(&Data::String("Kind".into())));
    assert_eq!(sheet.get((6, 3)), Some(&Data::Float(12.0)));
    assert_eq!(sheet.get((8, 3)), Some(&Data::Float(3.0)));

    let totals = book.worksheet_range("Totals").unwrap();
    let header: Vec<_> = (0..4)
        .map(|c| totals.get((0, c)).unwrap().to_string())
        .collect();
    assert_eq!(header, ["Kind", "Unit", "Count", "Sum"]);
    assert_eq!(totals.get((1, 0)), Some(&Data::String("area".into())));
    assert_eq!(totals.get((1, 2)), Some(&Data::Float(1.0)));
    assert_eq!(totals.get((2, 3)), Some(&Data::Float(7.0)));
    assert_eq!(totals.get((3, 0)), Some(&Data::String("count".into())));
    assert_eq!(totals.get((3, 3)), Some(&Data::Float(3.0)));
}

#[test]
fn export_table_writes_one_data_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doors.xlsx");
    let columns = ["Handle", "Block", "MARK", "Length (m)"].map(String::from);
    let long = "x".repeat(100);
    let rows = vec![
        ["42", "DOOR", "D1", ""].map(String::from).to_vec(),
        vec!["43".into(), "DOOR".into(), long.clone(), String::new()],
    ];
    qiyas_boq::export_table(&path, &columns, &rows).unwrap();

    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(book.sheet_names(), ["Data"]);
    let sheet = book.worksheet_range("Data").unwrap();
    let header: Vec<String> = (0..4)
        .map(|c| sheet.get((0, c)).unwrap().to_string())
        .collect();
    assert_eq!(header, columns);
    assert_eq!(sheet.get((1, 2)), Some(&Data::String("D1".into())));
    assert_eq!(sheet.get((2, 2)), Some(&Data::String(long)));
    assert_eq!(sheet.height(), 3);
}

//! Tables on screen: a table draws its `*T` block when it has one with something in it, else the
//! grid and cell texts Qiyas generates from its rows, columns and cells (merged cells drawn as
//! one); every path belongs to the table itself.

use opencadcodec::entities::{CellRange, EntityType, Table, TableColumn, TableRow};
use opencadcodec::tables::{BlockRecord, TableEntry};
use opencadcodec::xdata::{ExtendedDataRecord, XDataValue};
use opencadcodec::{CadDocument, Color, Layer, Line, Vector3};
use qiyas_geom::{flatten, table_cell_at, FlatPath, Paint, TextLayout, Xform, LOGICAL_TEXT_APP};

fn v(x: f64, y: f64) -> Vector3 {
    Vector3::new(x, y, 0.0)
}

fn doc() -> CadDocument {
    let mut doc = CadDocument::new();
    let mut layer = Layer::new("TABLE");
    layer.color = Color::Index(3);
    doc.layers.add(layer).unwrap();
    doc
}

/// A table at `at` with `texts` row by row, columns `width` wide and rows `height` tall, on
/// layer TABLE.
fn table(at: [f64; 2], texts: &[&[&str]], width: f64, height: f64) -> Table {
    let mut t = Table::new(v(at[0], at[1]), 0, 0);
    t.rows = texts.iter().map(|r| TableRow::from_texts(r)).collect();
    t.columns = (0..texts[0].len())
        .map(|_| TableColumn::with_width(width))
        .collect();
    t.set_uniform_row_height(height);
    t.common.layer = "TABLE".to_string();
    t
}

fn add(doc: &mut CadDocument, t: Table) -> (u64, EntityType) {
    let h = doc.add_entity(EntityType::Table(t)).unwrap();
    (h.value(), doc.get_entity(h).unwrap().clone())
}

fn lines(paths: &[FlatPath]) -> Vec<[[f64; 2]; 2]> {
    let mut out: Vec<[[f64; 2]; 2]> = paths
        .iter()
        .filter(|p| p.paint == Paint::Outline && p.text.is_none())
        .map(|p| {
            assert_eq!(p.path.points.len(), 2, "{p:?}");
            let [a, b] = [p.path.points[0], p.path.points[1]];
            let r = |p: [f64; 2]| p.map(|c| (c * 1e6).round() / 1e6);
            let (a, b) = (r(a), r(b));
            if (a[0], a[1]) <= (b[0], b[1]) {
                [a, b]
            } else {
                [b, a]
            }
        })
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

fn texts(paths: &[FlatPath]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| p.text.as_ref().map(|t| t.value.clone()))
        .collect()
}

#[test]
fn a_table_without_a_block_draws_its_grid_and_cell_texts_as_itself() {
    let mut doc = doc();
    let (h, t) = add(
        &mut doc,
        table([20.0, 0.0], &[&["A", "B"], &["C", "D"]], 2.0, 0.25),
    );
    let out = flatten(&doc, &t, &Xform::identity(), 1e-3);
    assert!(out
        .paths
        .iter()
        .all(|p| p.entity == h && p.chain.is_empty()));
    assert!(out.paths.iter().all(|p| p.layer == "TABLE"));
    assert!(out.paths.iter().all(|p| p.color == Color::Index(3)));
    // Rows go down from the insertion point, columns to the right.
    assert_eq!(
        lines(&out.paths),
        vec![
            [[20.0, -0.5], [20.0, 0.0]],
            [[20.0, -0.5], [24.0, -0.5]],
            [[20.0, -0.25], [24.0, -0.25]],
            [[20.0, 0.0], [24.0, 0.0]],
            [[22.0, -0.5], [22.0, 0.0]],
            [[24.0, -0.5], [24.0, 0.0]],
        ]
    );
    assert_eq!(texts(&out.paths), ["A", "B", "C", "D"]);
}

#[test]
fn merged_cells_draw_as_one_cell() {
    let mut doc = doc();
    let mut t = table(
        [0.0, 0.0],
        &[&["TITLE", "", ""], &["a", "b", "c"], &["", "e", "f"]],
        1.0,
        1.0,
    );
    t.merge_cells(CellRange::new(0, 0, 0, 2));
    t.merge_cells(CellRange::new(1, 0, 2, 0));
    let (_, t) = add(&mut doc, t);
    let out = flatten(&doc, &t, &Xform::identity(), 1e-3);
    assert_eq!(
        lines(&out.paths),
        vec![
            [[0.0, -3.0], [0.0, 0.0]],
            [[0.0, -3.0], [3.0, -3.0]],
            [[0.0, -1.0], [3.0, -1.0]],
            [[0.0, 0.0], [3.0, 0.0]],
            // No line inside the title row, none across the merged first column.
            [[1.0, -3.0], [1.0, -1.0]],
            [[1.0, -2.0], [3.0, -2.0]],
            [[2.0, -3.0], [2.0, -1.0]],
            [[3.0, -3.0], [3.0, 0.0]],
        ]
    );
    // The title sits in the middle of the whole row.
    let title = out
        .paths
        .iter()
        .find_map(|p| p.text.as_ref().filter(|t| t.value == "TITLE"))
        .unwrap();
    match title.layout {
        TextLayout::Block {
            insertion,
            attachment,
            ..
        } => {
            assert_eq!(attachment, 5, "middle centre");
            assert!((insertion[0] - 1.5).abs() < 1e-9 && (insertion[1] + 0.5).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(texts(&out.paths), ["TITLE", "a", "b", "c", "e", "f"]);
    // The cell under a point, a merged one by its first cell.
    let EntityType::Table(t) = &t else {
        unreachable!()
    };
    assert_eq!(table_cell_at(t, [2.5, -0.5]), Some((0, 0)));
    assert_eq!(table_cell_at(t, [0.5, -2.5]), Some((1, 0)));
    assert_eq!(table_cell_at(t, [2.5, -2.5]), Some((2, 2)));
    assert_eq!(table_cell_at(t, [3.5, -2.5]), None);
}

#[test]
fn a_turned_table_turns_with_its_direction() {
    let mut doc = doc();
    let mut t = table([10.0, 0.0], &[&["A", "B"]], 2.0, 1.0);
    t.horizontal_direction = v(0.0, 1.0);
    let (_, e) = add(&mut doc, t);
    let out = flatten(&doc, &e, &Xform::identity(), 1e-3);
    // Columns go up, rows to the right (x then y of the table turned a quarter).
    assert!(lines(&out.paths).contains(&[[10.0, 0.0], [10.0, 4.0]]));
    assert!(lines(&out.paths).contains(&[[11.0, 0.0], [11.0, 4.0]]));
    let EntityType::Table(t) = &e else {
        unreachable!()
    };
    assert_eq!(table_cell_at(t, [10.5, 3.0]), Some((0, 1)));
}

#[test]
fn a_table_with_a_block_draws_its_block_where_the_table_is() {
    let mut doc = doc();
    let mut record = BlockRecord::new("*T1");
    record.set_handle(doc.allocate_handle());
    record.flags.anonymous = true;
    let owner = record.handle;
    doc.block_records.add(record).unwrap();
    let mut line = EntityType::Line(Line::from_points(v(0.0, 0.0), v(4.0, 0.0)));
    line.common_mut().owner_handle = owner;
    line.common_mut().color = Color::ByBlock;
    doc.add_entity(line).unwrap();
    let mut t = table([20.0, 5.0], &[&["A", "B"], &["C", "D"]], 2.0, 0.25);
    t.block_name = "*T1".to_string();
    let mut by_handle = t.clone();
    let (h, t) = add(&mut doc, t);
    let out = flatten(&doc, &t, &Xform::identity(), 1e-3);
    assert_eq!(out.paths.len(), 1, "the block's line, nothing generated");
    let p = &out.paths[0];
    assert_eq!(p.entity, h);
    assert_eq!(p.color, Color::Index(3), "ByBlock takes the table's");
    assert_eq!(p.path.points, vec![[20.0, 5.0], [24.0, 5.0]]);

    // A DWG reader names the block by its record only; a stale name falls back to the record.
    for name in ["", "*T9"] {
        by_handle.block_name = name.to_string();
        by_handle.block_record_handle = Some(owner);
        let (_, e) = add(&mut doc, by_handle.clone());
        let out = flatten(&doc, &e, &Xform::identity(), 1e-3);
        assert_eq!(out.paths.len(), 1, "{name:?}");
    }
}

#[test]
fn arabic_cell_text_of_a_qiyas_table_is_in_reading_order() {
    let mut doc = doc();
    let mut t = table([0.0, 0.0], &[&["غرفة", "مطبخ"]], 2.0, 1.0);
    let mut record = ExtendedDataRecord::new(LOGICAL_TEXT_APP);
    record.add_value(XDataValue::Integer16(1));
    t.common.extended_data.upsert_record(record);
    let (_, t) = add(&mut doc, t);
    let out = flatten(&doc, &t, &Xform::identity(), 1e-3);
    let cells: Vec<_> = out.paths.iter().filter_map(|p| p.text.as_ref()).collect();
    assert_eq!(cells.len(), 2);
    assert!(cells.iter().all(|t| t.logical));
    assert_eq!(cells[0].value, "غرفة");
    // An AutoCAD table's text is the drawing's own: "Arabic text is reversed" applies.
    let (_, plain) = add(&mut doc, table([0.0, 9.0], &[&["غرفة"]], 2.0, 1.0));
    let out = flatten(&doc, &plain, &Xform::identity(), 1e-3);
    assert!(out
        .paths
        .iter()
        .filter_map(|p| p.text.as_ref())
        .all(|t| !t.logical));
}

//! Writes the take-off to an Excel workbook.

use std::path::Path;

use qiyas_core::{Kind, Measurement, Total};
use rust_xlsxwriter::{DocProperties, Format, RowNum, Workbook, Worksheet, XlsxError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoqError {
    #[error("The workbook could not be written: {0}")]
    Xlsx(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<XlsxError> for BoqError {
    fn from(e: XlsxError) -> BoqError {
        match e {
            XlsxError::IoError(e) => BoqError::Io(e),
            other => BoqError::Xlsx(other.to_string()),
        }
    }
}

const ITEM_HEADER: [&str; 8] = [
    "Description",
    "Kind",
    "Quantity",
    "Unit",
    "Layer",
    "Sheet",
    "Note",
    "Id",
];
const TOTAL_HEADER: [&str; 4] = ["Kind", "Unit", "Count", "Sum"];

/// Writes `items` and `totals` to the workbook at `path`. Sheet "Take-off" has one row per item,
/// a blank row, then the totals; sheet "Totals" has the totals alone. `title` becomes the
/// workbook's document title.
pub fn export_takeoff(
    path: &Path,
    items: &[Measurement],
    totals: &[Total],
    title: &str,
) -> Result<(), BoqError> {
    let mut book = Workbook::new();
    book.set_properties(&DocProperties::new().set_title(title));
    let bold = Format::new().set_bold();

    let sheet = book.add_worksheet().set_name("Take-off")?;
    sheet.write_row_with_format(0, 0, ITEM_HEADER, &bold)?;
    sheet.set_column_width(0, 40)?;
    for col in 1..ITEM_HEADER.len() as u16 {
        sheet.set_column_width(col, 12)?;
    }
    let mut row: RowNum = 1;
    for m in items {
        sheet.write_string(row, 0, &m.description)?;
        sheet.write_string(row, 1, kind_name(m.kind))?;
        sheet.write_number_with_format(row, 2, m.shown, &qty(m.kind))?;
        sheet.write_string(row, 3, &m.unit)?;
        sheet.write_string(row, 4, &m.layer)?;
        sheet.write_string(row, 5, &m.sheet)?;
        sheet.write_string(row, 6, &m.note)?;
        sheet.write_string(row, 7, &m.id)?;
        row += 1;
    }
    write_totals(sheet, row + 1, totals, &bold)?;

    let sheet = book.add_worksheet().set_name("Totals")?;
    for col in 0..TOTAL_HEADER.len() as u16 {
        sheet.set_column_width(col, 12)?;
    }
    write_totals(sheet, 0, totals, &bold)?;

    book.save(path)?;
    Ok(())
}

/// Writes the totals header at `row` and one row per total below it.
fn write_totals(
    sheet: &mut Worksheet,
    row: RowNum,
    totals: &[Total],
    bold: &Format,
) -> Result<(), XlsxError> {
    sheet.write_row_with_format(row, 0, TOTAL_HEADER, bold)?;
    for (t, row) in totals.iter().zip(row + 1..) {
        sheet.write_string(row, 0, kind_name(t.kind))?;
        sheet.write_string(row, 1, &t.unit)?;
        sheet.write_number(row, 2, t.count as f64)?;
        sheet.write_number_with_format(row, 3, t.sum, &qty(t.kind))?;
    }
    Ok(())
}

/// Counts are whole numbers; everything else keeps three decimals.
fn num_format(kind: Kind) -> &'static str {
    match kind {
        Kind::Count => "0",
        Kind::Length | Kind::Area | Kind::Volume | Kind::Item => "0.000",
    }
}

fn qty(kind: Kind) -> Format {
    Format::new().set_num_format(num_format(kind))
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Length => "length",
        Kind::Area => "area",
        Kind::Count => "count",
        Kind::Volume => "volume",
        Kind::Item => "item",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_whole_numbers() {
        assert_eq!(num_format(Kind::Count), "0");
        assert_eq!(num_format(Kind::Length), "0.000");
        assert_eq!(num_format(Kind::Area), "0.000");
    }
}

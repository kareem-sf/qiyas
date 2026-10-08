//! `cargo run -p qiyas-boq --example write_test_boq -- <out.xlsx>`: writes a six-item BOQ for
//! `samples/test-plan.dwg`. Two quantities are wrong on purpose (A.4 and B.2), so comparing it
//! with the drawing must give exactly those two conflicts. See `samples/test-plan.md`.

use rust_xlsxwriter::{Format, Workbook, XlsxError};

/// (code, description, unit, quantity, rate); a row with no unit is a section heading.
const ROWS: [(&str, &str, &str, f64, f64); 8] = [
    ("A", "Walls and floors", "", 0.0, 0.0),
    ("A.1", "Block walls 200 mm", "m", 42.0, 310.0),
    ("A.2", "Partition walls", "m", 14.0, 240.0),
    ("A.3", "Floor screed, living room", "m2", 58.29, 95.0),
    // Wrong on purpose: the bedroom is 22.090 m2.
    ("A.4", "Floor screed, bedroom", "m2", 30.0, 95.0),
    ("B", "Doors and windows", "", 0.0, 0.0),
    ("B.1", "Doors 900 mm", "pcs", 3.0, 4500.0),
    // Wrong on purpose: the drawing has 5 windows.
    ("B.2", "Windows 1200 mm", "pcs", 4.0, 3800.0),
];

fn main() -> Result<(), XlsxError> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "samples/test-boq.xlsx".to_string());
    let mut book = Workbook::new();
    let sheet = book.add_worksheet().set_name("BOQ")?;
    let bold = Format::new().set_bold();
    for (c, header) in ["Code", "Description", "Unit", "Qty", "Rate (EGP)"]
        .iter()
        .enumerate()
    {
        sheet.write_string_with_format(0, c as u16, *header, &bold)?;
    }
    for (r, (code, description, unit, qty, rate)) in ROWS.iter().enumerate() {
        let r = r as u32 + 1;
        sheet.write_string(r, 0, *code)?;
        sheet.write_string(r, 1, *description)?;
        if !unit.is_empty() {
            sheet.write_string(r, 2, *unit)?;
            sheet.write_number(r, 3, *qty)?;
            sheet.write_number(r, 4, *rate)?;
        }
    }
    sheet.set_column_width(1, 30)?;
    book.save(&out)?;
    println!("wrote {out}");
    Ok(())
}

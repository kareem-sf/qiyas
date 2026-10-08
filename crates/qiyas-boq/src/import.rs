//! Reads a bill of quantities from an Excel workbook with English or Arabic headers.
//!
//! The header row is found by matching its cells against known column names; rows above it
//! (titles, merged or not) are skipped. Below it, a row with a description but no unit and no
//! quantity is a section heading for the items that follow.

use std::path::Path;

use calamine::{open_workbook_auto, Data, Range, Reader};
use qiyas_core::{Boq, BoqItem};

use crate::BoqError;

/// Rows of each sheet searched for the header.
const HEADER_ROWS: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Field {
    Code,
    Description,
    Unit,
    Qty,
    Rate,
}

/// Column names per field, English and Arabic. A cell matches a name when it is that name or
/// starts with it and a space (e.g. "Rate (AED)"); the longest name matched wins, so
/// "Item description" is a description, not a code.
const NAMES: [(Field, &[&str]); 5] = [
    (
        Field::Code,
        &["code", "item", "ref", "no", "بند", "رقم", "كود"],
    ),
    (
        Field::Description,
        &[
            "description",
            "desc",
            "item description",
            "الوصف",
            "البيان",
            "الوصف البند",
        ],
    ),
    (Field::Unit, &["unit", "uom", "الوحدة"]),
    (Field::Qty, &["qty", "quantity", "الكمية"]),
    (
        Field::Rate,
        &["rate", "unit rate", "price", "السعر", "سعر الوحدة", "الفئة"],
    ),
];

/// Units as written in BOQs, and what each one is called in Qiyas.
const UNITS: [(&str, &[&str]); 6] = [
    ("m2", &["m²", "sqm", "sq.m", "m2", "م2", "م²", "متر مربع"]),
    ("m3", &["m³", "cum", "cu.m", "m3", "م3", "م³", "متر مكعب"]),
    (
        "pcs",
        &["no", "nr", "nos", "pcs", "pc", "each", "ea", "عدد", "قطعة"],
    ),
    ("m", &["lm", "m", "rm", "mt", "م", "م.ط", "متر", "متر طولي"]),
    ("kg", &["kg", "كجم", "كغ"]),
    ("t", &["ton", "t", "tonne", "طن"]),
];

/// Where the header row is and which column holds each field.
struct Header {
    /// Index of the header row within the sheet's range.
    row: usize,
    score: usize,
    code: Option<usize>,
    description: usize,
    unit: usize,
    qty: usize,
    rate: Option<usize>,
}

/// Reads the BOQ in the workbook at `path` (.xlsx, .xlsm, .xls or .ods). The sheet whose header
/// matches the most fields wins; the first such sheet on a tie.
pub fn import_boq(path: &Path) -> Result<Boq, BoqError> {
    let mut book = open_workbook_auto(path).map_err(|e| BoqError::Read(e.to_string()))?;
    let names = book.sheet_names();
    let mut best: Option<(Range<Data>, Header)> = None;
    for name in &names {
        // A sheet that can't be read (a chart sheet, say) has no BOQ in it.
        let Ok(range) = book.worksheet_range(name) else {
            continue;
        };
        if let Some(header) = find_header(&range) {
            if best.as_ref().is_none_or(|(_, b)| header.score > b.score) {
                best = Some((range, header));
            }
        }
    }
    let Some((range, header)) = best else {
        return Err(BoqError::NoHeader {
            sheets_tried: names,
        });
    };
    let (items, skipped_rows) = read_items(&range, &header);
    Ok(Boq::new(path.display().to_string(), items, skipped_rows))
}

/// The first row among the sheet's first `HEADER_ROWS` that names a description, a unit and a
/// quantity column.
fn find_header(range: &Range<Data>) -> Option<Header> {
    let first = range.start()?.0;
    range
        .rows()
        .enumerate()
        .take_while(|(i, _)| first + (*i as u32) < HEADER_ROWS)
        .find_map(|(i, cells)| {
            let mut columns: [Option<usize>; 5] = [None; 5];
            for (col, cell) in cells.iter().enumerate() {
                if let Some(field) = field_of(&text(cell)) {
                    let slot = &mut columns[field as usize];
                    slot.get_or_insert(col);
                }
            }
            let [code, description, unit, qty, rate] = columns;
            Some(Header {
                row: i,
                score: columns.iter().flatten().count(),
                code,
                description: description?,
                unit: unit?,
                qty: qty?,
                rate,
            })
        })
}

/// The field a header cell names, if any.
fn field_of(cell: &str) -> Option<Field> {
    let cell = header_key(cell);
    NAMES
        .iter()
        .flat_map(|(field, names)| names.iter().map(move |n| (*field, header_key(n))))
        .filter(|(_, name)| {
            !name.is_empty()
                && (cell == *name
                    || cell
                        .strip_prefix(name.as_str())
                        .is_some_and(|rest| rest.starts_with(' ')))
        })
        .max_by_key(|(_, name)| name.chars().count())
        .map(|(field, _)| field)
}

/// The items below the header, and how many item rows were left out for a quantity that is not
/// a number.
fn read_items(range: &Range<Data>, h: &Header) -> (Vec<BoqItem>, u32) {
    let first = range.start().map_or(0, |(row, _)| row);
    let mut items = Vec::new();
    let mut skipped = 0;
    let mut section = String::new();
    for (i, cells) in range.rows().enumerate().skip(h.row + 1) {
        let cell = |col: usize| cells.get(col).unwrap_or(&Data::Empty);
        let description = text(cell(h.description));
        let unit = text(cell(h.unit));
        let qty = cell(h.qty);
        let qty_empty = text(qty).is_empty();
        if description.is_empty() && qty_empty {
            continue;
        }
        if unit.is_empty() && qty_empty {
            section = description;
            continue;
        }
        let Some(quantity) = number(qty) else {
            skipped += 1;
            continue;
        };
        items.push(BoqItem {
            code: h.code.map(|c| text(cell(c))).unwrap_or_default(),
            description,
            unit: normalise_unit(&unit),
            quantity,
            rate: h.rate.and_then(|c| number(cell(c))),
            section: section.clone(),
            tolerance: None,
            row: first + i as u32 + 1,
        });
    }
    (items, skipped)
}

/// A cell as trimmed text; numbers as Excel would show them without formatting.
fn text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        other => other.to_string().trim().to_string(),
    }
}

/// A cell's number: a number cell, or text such as "1,250.5" or "١٢٥٠" (Arabic-Indic digits).
fn number(cell: &Data) -> Option<f64> {
    match cell {
        Data::Int(n) => Some(*n as f64),
        Data::Float(n) => Some(*n),
        Data::String(s) => parse_number(s),
        _ => None,
    }
    .filter(|n| n.is_finite())
}

/// Parses text with Western or Arabic-Indic digits, `.` or `٫` as the decimal point, and `,` or
/// `٬` between thousands. Thousands groups must be 3 digits, so "1,5" is refused rather than
/// read as 15.
fn parse_number(s: &str) -> Option<f64> {
    let mut plain = String::new();
    for c in s.trim().chars() {
        plain.push(match c {
            '٠'..='٩' => char::from(b'0' + (c as u32 - '٠' as u32) as u8),
            '۰'..='۹' => char::from(b'0' + (c as u32 - '۰' as u32) as u8),
            '٫' => '.',
            '٬' => ',',
            ' ' | '\u{a0}' | '\u{202f}' => continue,
            c => c,
        });
    }
    let (int, frac) = match plain.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (plain.as_str(), None),
    };
    let (sign, digits) = match int.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", int),
    };
    let mut groups = digits.split(',');
    let lead = groups.next().unwrap_or_default();
    let rest: Vec<&str> = groups.collect();
    let grouped = |g: &str| g.len() == 3 && g.bytes().all(|b| b.is_ascii_digit());
    if !rest.is_empty() && !(matches!(lead.len(), 1..=3) && rest.iter().all(|g| grouped(g))) {
        return None;
    }
    let joined = format!("{sign}{lead}{}", rest.concat());
    let joined = match frac {
        Some(frac) => format!("{joined}.{frac}"),
        None => joined,
    };
    joined.parse().ok()
}

/// The Qiyas name of a BOQ unit ("m2", "m3", "pcs", "m", "kg", "t"), or the text as written
/// (trimmed) when it is none of those.
fn normalise_unit(unit: &str) -> String {
    let key = unit_key(unit);
    UNITS
        .iter()
        .find(|(_, names)| names.iter().any(|n| unit_key(n) == key))
        .map_or_else(|| unit.trim().to_string(), |(name, _)| name.to_string())
}

/// A unit for comparing: lower case, no spaces, no trailing dots, Arabic without diacritics.
fn unit_key(unit: &str) -> String {
    let key: String = plain_arabic(&unit.to_lowercase())
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    key.trim_end_matches('.').to_string()
}

/// A header cell for comparing: lower case, punctuation as spaces, single spaces, Arabic
/// without diacritics and with one form of alef, yeh and teh marbuta.
fn header_key(cell: &str) -> String {
    let spaced: String = plain_arabic(&cell.to_lowercase())
        .chars()
        .map(|c| match c {
            'أ' | 'إ' | 'آ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s` without Arabic diacritics (harakat, superscript alef) and tatweel.
fn plain_arabic(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\u{064B}'..='\u{065F}' | '\u{0670}' | '\u{0640}'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_parse_with_separators_and_arabic_digits() {
        assert_eq!(parse_number("1,250.5"), Some(1250.5));
        assert_eq!(parse_number("١٢٣٫٥"), Some(123.5));
        assert_eq!(parse_number("١٬٢٥٠"), Some(1250.0));
        assert_eq!(parse_number("-12"), Some(-12.0));
        assert_eq!(parse_number("1,5"), None);
        assert_eq!(parse_number("Rate only"), None);
        assert_eq!(parse_number(""), None);
    }

    #[test]
    fn header_cells_match_the_longest_name() {
        assert_eq!(field_of("Item Description"), Some(Field::Description));
        assert_eq!(field_of("Item"), Some(Field::Code));
        assert_eq!(field_of("No."), Some(Field::Code));
        assert_eq!(field_of("Unit Rate (EGP)"), Some(Field::Rate));
        assert_eq!(field_of("الكمـــيّة"), Some(Field::Qty));
        assert_eq!(field_of("سعر الوحدة"), Some(Field::Rate));
        assert_eq!(field_of("Amount"), None);
        assert_eq!(field_of("Notes"), None);
    }

    #[test]
    fn units_normalise() {
        for (raw, want) in [
            ("M2", "m2"),
            ("sq. m", "m2"),
            ("مَتر مُربّع", "m2"),
            ("م³", "m3"),
            ("Nos.", "pcs"),
            ("عدد", "pcs"),
            ("م.ط", "m"),
            ("LM", "m"),
            ("كجم", "kg"),
            ("Tonne", "t"),
            (" L.S ", "L.S"),
        ] {
            assert_eq!(normalise_unit(raw), want, "{raw}");
        }
    }
}

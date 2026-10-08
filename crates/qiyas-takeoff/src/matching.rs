//! Text matching for the comparison: description tokens in English and Arabic, description
//! groups, floor tokens in sheet names, units, numbers in schedule cells.

use std::collections::BTreeSet;

/// Words that say nothing about the work. Compared after `fold`.
const STOP_WORDS: [&str; 10] = [
    "the", "of", "and", "in", "to", "for", "و", "في", "من", "إلى",
];
/// Arabic article and the prepositions fused with it, longest first.
const ARTICLES: [&str; 6] = ["وال", "بال", "كال", "فال", "لل", "ال"];
/// Two descriptions match when this share of the shorter one's tokens is in the other.
const MATCH_SHARE: f64 = 0.5;
/// The units the BOQ import knows; any other is the workbook's own text.
pub(crate) const STANDARD_UNITS: [&str; 6] = ["m", "m2", "m3", "pcs", "kg", "t"];

/// Lower case, Arabic diacritics and tatweel removed, alef forms, ى and ة folded (like the BOQ
/// panel's search), so "الأعمدة" and "الاعمده" are the same.
pub(crate) fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '\u{064B}'..='\u{0670}' | '\u{0640}'))
        .map(|c| match c {
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            c => c,
        })
        .collect()
}

/// The words of a description: folded, split at anything not a letter or digit, digits removed,
/// stop words and one-letter words dropped, the Arabic article (also fused with و ب ك ف ل)
/// taken off.
pub(crate) fn tokens(text: &str) -> BTreeSet<String> {
    let stop: Vec<String> = STOP_WORDS.iter().map(|w| fold(w)).collect();
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .map(|word| word.chars().filter(|c| !c.is_numeric()).collect::<String>())
        .filter(|word| !stop.contains(word))
        .map(|word| {
            ARTICLES
                .iter()
                .find_map(|a| {
                    word.strip_prefix(a)
                        .filter(|rest| rest.chars().count() >= 2)
                })
                .map_or(word.clone(), str::to_string)
        })
        .filter(|word| word.chars().count() >= 2)
        .collect()
}

/// True when at least half the tokens of the shorter description are in the other (`tokens`
/// of each).
pub(crate) fn overlaps(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    let shorter = a.len().min(b.len());
    shorter > 0 && a.intersection(b).count() as f64 / shorter as f64 >= MATCH_SHARE
}

/// The key measurements are grouped by: folded, digits and punctuation removed, spaces
/// collapsed, so "Wall 1" and "wall-2" group together.
pub(crate) fn group_key(description: &str) -> String {
    fold(description)
        .chars()
        .map(|c| if c.is_alphabetic() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Units compare without case or spaces, so "M2" is "m2".
pub(crate) fn unit_key(unit: &str) -> String {
    unit.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// The floor names in a sheet name: GF, G, FF, SF, TF, RF, B1, L1 to L99, 1F to 99F, and
/// الأرضي، الأول، الثاني، الثالث, as whole words in any case.
pub(crate) fn floor_tokens(sheet: &str) -> BTreeSet<String> {
    let arabic: Vec<String> = ["الأرضي", "الأول", "الثاني", "الثالث"]
        .iter()
        .map(|w| fold(w))
        .collect();
    sheet
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter_map(|word| {
            let upper = word.to_uppercase();
            let folded = fold(word);
            if is_floor(&upper) {
                Some(upper)
            } else {
                arabic.contains(&folded).then_some(folded)
            }
        })
        .collect()
}

fn is_floor(word: &str) -> bool {
    let digits = |s: &str, most: usize| {
        (1..=most).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit())
    };
    matches!(word, "GF" | "G" | "FF" | "SF" | "TF" | "RF")
        || word.strip_prefix('B').is_some_and(|d| digits(d, 1))
        || word.strip_prefix('L').is_some_and(|d| digits(d, 2))
        || word.strip_suffix('F').is_some_and(|d| digits(d, 2))
}

/// A sheet of schedules: its name says "schedule" or "جدول".
pub(crate) fn is_schedule_sheet(sheet: &str) -> bool {
    sheet.to_lowercase().contains("schedule") || sheet.contains("جدول")
}

/// The number a schedule cell starts with ("4", "4 nos", "٤"); `None` when it starts with none.
pub(crate) fn leading_number(cell: &str) -> Option<f64> {
    let digits: String = cell
        .trim()
        .chars()
        .map(|c| match c {
            '\u{0660}'..='\u{0669}' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            '\u{06F0}'..='\u{06F9}' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            '٫' => '.',
            c => c,
        })
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.parse().ok()
}

/// A block is the one a schedule mark names when its name is the mark, or has the mark as one of
/// its words ("DOOR-D1" for "D1"), in any case.
pub(crate) fn block_has_mark(block: &str, mark: &str) -> bool {
    let (block, mark) = (block.to_lowercase(), mark.trim().to_lowercase());
    !mark.is_empty()
        && (block == mark
            || block
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| w == mark))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_article_and_stop_words_go() {
        let t = tokens("خرسانة مسلحة للأعمدة في المبنى");
        assert!(t.contains("اعمده"), "{t:?}");
        assert!(t.contains("مبني"), "{t:?}");
        assert!(!t.iter().any(|w| w == "في"));
        assert_eq!(
            tokens("The walls of the 2nd floor"),
            tokens("walls floor nd")
        );
    }

    #[test]
    fn floors_are_whole_words() {
        assert_eq!(floor_tokens("GF Plan"), BTreeSet::from(["GF".to_string()]));
        assert!(floor_tokens("L02 - Lighting").contains("L02"));
        assert!(floor_tokens("1f furniture").contains("1F"));
        assert!(floor_tokens("GF_PLAN").is_empty());
        assert!(floor_tokens("Gfx Plan").is_empty());
        assert!(!floor_tokens("مخطط الطابق الأرضي").is_empty());
    }

    #[test]
    fn schedule_cells_and_marks() {
        assert_eq!(leading_number(" 4 nos"), Some(4.0));
        assert_eq!(leading_number("٤"), Some(4.0));
        assert_eq!(leading_number("-"), None);
        assert!(block_has_mark("DOOR-D1", "d1"));
        assert!(block_has_mark("D1", "D1"));
        assert!(!block_has_mark("D10", "D1"));
    }
}

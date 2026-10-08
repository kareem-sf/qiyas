//! The system prompt: who the assistant is, its rules, and a summary of the open drawing, the
//! take-off and the BOQ. English or Arabic, by the user's interface language.

use std::collections::HashMap;
use std::fmt::Write;

use qiyas_core::{EntityFilter, Kind, Session, Total};

/// Most layers and blocks listed in the prompt; the rest are counted.
const MAX_LAYERS: usize = 60;
const MAX_BLOCKS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Ar,
}

impl Lang {
    /// "ar" is Arabic; anything else is English.
    pub fn from_code(code: &str) -> Lang {
        if code == "ar" {
            Lang::Ar
        } else {
            Lang::En
        }
    }
}

/// The drawing facts the system prompt gives, captured once when the conversation starts. They
/// never change for the life of the conversation, so the system prompt stays byte-identical
/// (the API binds the model's thinking to it).
#[derive(Debug, Clone, Default)]
pub struct DrawingFacts {
    /// False when no drawing was open; the rest is then empty.
    pub open: bool,
    /// The drawing's file name.
    pub file: String,
    /// The drawing's unit name, e.g. "millimetres".
    pub units: String,
    /// Sheet names, model first as the drawing lists them.
    pub sheets: Vec<String>,
    /// The sheet shown when the conversation started.
    pub sheet: String,
    /// Layers with their entity count on that sheet, most first; empty layers left out.
    pub layers: Vec<(String, usize)>,
    /// Blocks with their reference count on that sheet (visible layers), most first.
    pub blocks: Vec<(String, usize)>,
}

impl DrawingFacts {
    pub fn none() -> DrawingFacts {
        DrawingFacts::default()
    }

    pub fn of(session: &Session) -> DrawingFacts {
        let info = session.info();
        let sheet = session.sheet.clone();
        let mut layers: Vec<(String, usize)> = info
            .layers
            .iter()
            .map(|l| {
                let total = session
                    .find_entities(&EntityFilter {
                        sheet: Some(sheet.clone()),
                        layer: Some(l.name.clone()),
                        limit: 0,
                        ..Default::default()
                    })
                    .total;
                (l.name.clone(), total)
            })
            .filter(|(_, n)| *n > 0)
            .collect();
        layers.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut per_block: HashMap<String, usize> = HashMap::new();
        for c in session.block_summary(Some(&sheet)) {
            *per_block.entry(c.block_name).or_default() += c.count;
        }
        let mut blocks: Vec<(String, usize)> = per_block.into_iter().collect();
        blocks.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        DrawingFacts {
            open: true,
            file: file_name(session),
            units: info.units.clone(),
            sheet: sheet_name(session),
            sheets: info.sheets.into_iter().map(|s| s.name).collect(),
            layers,
            blocks,
        }
    }
}

/// What changes while the user works: sent at the start of every user message, never in the
/// system prompt.
#[derive(Debug, Clone, Default)]
pub struct LiveState {
    /// False when no drawing is open; the rest is then empty.
    pub open: bool,
    pub file: String,
    /// The sheet shown now.
    pub sheet: String,
    pub takeoff: Vec<Total>,
    /// The imported BOQ's file name; `None` when there is none.
    pub boq: Option<String>,
    /// Conflicts of the last comparison still open.
    pub conflicts: usize,
}

impl LiveState {
    pub fn none() -> LiveState {
        LiveState::default()
    }

    pub fn of(session: &Session) -> LiveState {
        LiveState {
            open: true,
            file: file_name(session),
            sheet: sheet_name(session),
            takeoff: session.takeoff.totals(),
            boq: session.boq.as_ref().map(|b| {
                std::path::Path::new(&b.source)
                    .file_name()
                    .map_or_else(|| b.source.clone(), |n| n.to_string_lossy().into_owned())
            }),
            conflicts: session
                .conflicts
                .iter()
                .filter(|c| c.status == qiyas_core::ConflictStatus::Open)
                .count(),
        }
    }
}

fn file_name(session: &Session) -> String {
    let path = session.info().path;
    std::path::Path::new(&path)
        .file_name()
        .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned())
}

/// The name of the sheet shown (its layout name, not its block).
fn sheet_name(session: &Session) -> String {
    session
        .info()
        .sheets
        .into_iter()
        .find(|s| s.block == session.sheet)
        .map_or(session.sheet.clone(), |s| s.name)
}

const EN: &str = "\
You are the AI assistant inside Qiyas, a measurement CAD by QS Mind, working for a quantity \
surveyor. You read the open drawing with your tools, measure quantities, add them to the take-off, \
and explain what you did.

Rules:
- Never state a quantity unless it comes from a measurement or tool result in this conversation. \
If you have not measured it, say so and offer to measure it.
- Prefer measuring drawn entities (by handle, from find_entities) over picking points: it \
measures the drawn geometry exactly. Use points only when no entity fits.
- Say which sheets and layers you used for every quantity, so the user can check it.
- Look before you measure: list layers, find entities, read text and look at the drawing when \
the layer names are unclear.
- Coordinates, lengths and areas from the drawing are in drawing units. Measurements give their \
quantity in metres, square metres, cubic metres or pieces.
- Give each measurement a clear description: what was measured and where.
- If something in the drawing is unclear or seems wrong, say so instead of guessing.
- Each user message starts with a [state] block written by Qiyas, not the user: the sheet shown, \
the take-off totals and the BOQ status now. Trust it over the drawing summary below for those.
- Answer in the user's language, briefly, in plain words.";

const AR: &str = "\
أنت مساعد الذكاء الاصطناعي لمهندس الكميات في قياس، برنامج القياس من كيو إس مايند. \
تقرأ المخطط المفتوح بأدواتك، وتقيس الكميات، وتضيفها إلى جدول الحصر، وتشرح ما فعلت.

القواعد:
- لا تذكر أي كمية إلا إذا جاءت من قياس أو من نتيجة أداة في هذه المحادثة. إن لم تقسها فقل ذلك واعرض أن تقيسها.
- فضّل قياس العناصر المرسومة (بالمعرّف من find_entities) على تحديد النقاط، فهو يقيس الشكل المرسوم بدقة. \
استخدم النقاط فقط حين لا يناسب أي عنصر.
- اذكر اللوحات والطبقات التي استخدمتها لكل كمية، ليتحقق منها المستخدم.
- انظر قبل أن تقيس: اعرض الطبقات، وابحث عن العناصر، واقرأ النصوص، وانظر إلى المخطط حين تكون أسماء الطبقات غير واضحة.
- الإحداثيات والأطوال والمساحات من المخطط بوحدات الرسم. أما القياسات فتعطي كميتها بالمتر أو المتر المربع أو المتر المكعب أو بالعدد.
- أعط كل قياس وصفًا واضحًا: ما الذي قِيس وأين.
- إن كان شيء في المخطط غير واضح أو يبدو خطأ فقل ذلك ولا تخمّن.
- تبدأ كل رسالة من المستخدم بكتلة [state] يكتبها قياس لا المستخدم: اللوحة المعروضة ومجاميع الحصر وحالة جدول الكميات الآن. اعتمد عليها بدل ملخص المخطط أدناه في هذه الأمور.
- أجب بلغة المستخدم، باختصار وبكلمات بسيطة.";

/// The system prompt in `lang`: the fixed rules and the drawing facts. Build it once per
/// conversation; the live state goes in `state_block` instead.
pub fn system_prompt(lang: Lang, facts: &DrawingFacts) -> String {
    let ar = lang == Lang::Ar;
    let t = |en: &'static str, arabic: &'static str| if ar { arabic } else { en };
    let mut s = String::from(if ar { AR } else { EN });

    s.push_str(t(
        "\n\n## The drawing (when this conversation started)\n",
        "\n\n## المخطط (عند بدء هذه المحادثة)\n",
    ));
    if !facts.open {
        s.push_str(t(
            "No drawing was open. If the [state] block still says so, ask the user to open one.\n",
            "لم يكن هناك مخطط مفتوح. إن كانت كتلة [state] ما زالت تقول ذلك فاطلب من المستخدم أن يفتح مخططًا.\n",
        ));
        return s;
    }
    let _ = writeln!(s, "{}{}", t("File: ", "الملف: "), facts.file);
    let _ = writeln!(s, "{}{}", t("Units: ", "الوحدات: "), facts.units);
    let _ = writeln!(
        s,
        "{}{}",
        t("Sheets: ", "اللوحات: "),
        facts.sheets.join(", ")
    );
    let _ = writeln!(
        s,
        "{}{}",
        t("Sheet shown: ", "اللوحة المعروضة: "),
        facts.sheet
    );
    s.push_str(t(
        "Layers on that sheet (entities):\n",
        "الطبقات في تلك اللوحة (عدد العناصر):\n",
    ));
    list(&mut s, &facts.layers, MAX_LAYERS, ar);
    s.push_str(t(
        "Blocks on that sheet (references):\n",
        "الكتل في تلك اللوحة (عدد المراجع):\n",
    ));
    list(&mut s, &facts.blocks, MAX_BLOCKS, ar);
    s
}

/// The `[state]` text put first in each user message: what is true now.
pub fn state_block(lang: Lang, state: &LiveState) -> String {
    let ar = lang == Lang::Ar;
    let t = |en: &'static str, arabic: &'static str| if ar { arabic } else { en };
    let mut s = String::from(t(
        "[state] Written by Qiyas, not the user.\n",
        "[state] يكتبها قياس، لا المستخدم.\n",
    ));
    if !state.open {
        s.push_str(t("No drawing is open.\n", "لا يوجد مخطط مفتوح.\n"));
    } else {
        let _ = writeln!(s, "{}{}", t("Drawing: ", "المخطط: "), state.file);
        let _ = writeln!(
            s,
            "{}{}",
            t("Sheet shown: ", "اللوحة المعروضة: "),
            state.sheet
        );
    }
    s.push_str(t("Take-off:\n", "جدول الحصر:\n"));
    if state.takeoff.is_empty() {
        s.push_str(t("- no measurements yet\n", "- لا توجد قياسات بعد\n"));
    }
    for total in &state.takeoff {
        let kind = kind_name(total.kind, ar);
        let unit = unit_name(&total.unit, ar);
        let sum = total.sum;
        let n = total.count;
        let _ = if ar {
            writeln!(s, "- {kind}: القياسات: {n}، المجموع: {sum:.3} {unit}")
        } else {
            let plural = if n == 1 { "" } else { "s" };
            writeln!(
                s,
                "- {kind}: {n} measurement{plural}, total {sum:.3} {unit}"
            )
        };
    }
    let boq = match (&state.boq, ar) {
        (Some(name), _) => name.as_str(),
        (None, false) => "no BOQ loaded",
        (None, true) => "لم يُحمّل جدول كميات",
    };
    let _ = writeln!(s, "{}{boq}", t("BOQ: ", "جدول الكميات: "));
    let _ = writeln!(
        s,
        "{}{}",
        t("Open conflicts: ", "التعارضات المفتوحة: "),
        state.conflicts
    );
    s
}

fn kind_name(kind: Kind, ar: bool) -> &'static str {
    match (kind, ar) {
        (Kind::Length, false) => "length",
        (Kind::Area, false) => "area",
        (Kind::Count, false) => "count",
        (Kind::Volume, false) => "volume",
        (Kind::Item, false) => "item",
        (Kind::Length, true) => "طول",
        (Kind::Area, true) => "مساحة",
        (Kind::Count, true) => "عدد",
        (Kind::Volume, true) => "حجم",
        (Kind::Item, true) => "بند",
    }
}

/// Take-off units by name; an item's own unit is kept as it is.
fn unit_name(unit: &str, ar: bool) -> &str {
    match (unit, ar) {
        ("m", true) => "م",
        ("m2", true) => "م²",
        ("m3", true) => "م³",
        ("pcs", true) => "قطعة",
        _ => unit,
    }
}

fn list(s: &mut String, rows: &[(String, usize)], max: usize, ar: bool) {
    if rows.is_empty() {
        s.push_str(if ar { "- لا شيء\n" } else { "- none\n" });
        return;
    }
    for (name, n) in rows.iter().take(max) {
        let _ = writeln!(s, "- {name}: {n}");
    }
    if rows.len() > max {
        let more = rows.len() - max;
        let _ = if ar {
            writeln!(s, "- و{more} أخرى")
        } else {
            writeln!(s, "- and {more} more")
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> usize {
        text.split_whitespace().count()
    }

    #[test]
    fn fixed_text_is_under_400_words() {
        assert!(words(EN) < 400, "{}", words(EN));
        assert!(words(AR) < 400, "{}", words(AR));
    }

    #[test]
    fn no_drawing_says_so_in_both_languages() {
        let en = system_prompt(Lang::En, &DrawingFacts::none());
        assert!(en.contains("No drawing was open"));
        let state = state_block(Lang::En, &LiveState::none());
        assert!(state.starts_with("[state]"));
        assert!(state.contains("No drawing is open"));
        assert!(state.contains("BOQ: no BOQ loaded"));
        let ar = system_prompt(Lang::Ar, &DrawingFacts::none());
        assert!(ar.contains("قياس"));
        assert!(ar.contains("كيو إس مايند"));
        assert!(ar.contains("مساعد الذكاء الاصطناعي لمهندس الكميات"));
        assert!(ar.contains("لم يكن هناك مخطط مفتوح"));
        let state = state_block(Lang::Ar, &LiveState::none());
        assert!(state.contains("لا يوجد مخطط مفتوح"));
        assert!(state.contains("لم يُحمّل جدول كميات"));
    }

    fn sample_with_count() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
        let mut session = Session::open(&path).unwrap();
        session
            .run(qiyas_core::Command::CountBlock {
                block_name: "DOOR".into(),
                layer: None,
            })
            .unwrap();
        (dir, session)
    }

    #[test]
    fn system_prompt_lists_the_drawing_but_not_the_takeoff() {
        let (_dir, session) = sample_with_count();
        let facts = DrawingFacts::of(&session);
        assert!(facts.layers.contains(&("A-WALL".into(), 3)));
        assert!(facts.layers.contains(&("A-DOOR".into(), 3)));
        assert_eq!(facts.blocks, vec![("DOOR".to_string(), 3)]);
        let text = system_prompt(Lang::En, &facts);
        assert!(text.contains("File: sample.dxf"), "{text}");
        assert!(text.contains("Units: metres"), "{text}");
        assert!(text.contains("- A-WALL: 3"), "{text}");
        assert!(text.contains("- DOOR: 3"), "{text}");
        assert!(
            !text.contains("pcs"),
            "live state belongs in the state block: {text}"
        );
    }

    #[test]
    fn state_block_gives_takeoff_in_both_languages() {
        let (_dir, session) = sample_with_count();
        let state = LiveState::of(&session);
        let en = state_block(Lang::En, &state);
        assert!(en.contains("Drawing: sample.dxf"), "{en}");
        assert!(
            en.contains("- count: 1 measurement, total 3.000 pcs"),
            "{en}"
        );
        assert!(en.contains("Open conflicts: 0"), "{en}");
        let ar = state_block(Lang::Ar, &state);
        assert!(
            ar.contains("- عدد: القياسات: 1، المجموع: 3.000 قطعة"),
            "{ar}"
        );
        assert_eq!(unit_name("m2", true), "م²");
        assert_eq!(unit_name("m3", true), "م³");
        assert_eq!(unit_name("bags", true), "bags");
        assert_eq!(kind_name(Kind::Item, true), "بند");
    }
}

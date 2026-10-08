//! The system prompt: who the assistant is, its rules, and a summary of the open drawing, the
//! take-off and the BOQ. English or Arabic, by the user's interface language.

use std::collections::HashMap;
use std::fmt::Write;

use qiyas_core::{EntityFilter, Session, Total};

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

/// What the prompt says about the open project.
#[derive(Debug, Clone, Default)]
pub struct ProjectSummary {
    /// False when no drawing is open; the rest is then empty.
    pub open: bool,
    /// Sheet names, model first as the drawing lists them.
    pub sheets: Vec<String>,
    /// The sheet shown.
    pub sheet: String,
    /// Layers with their entity count on the sheet shown, most first; empty layers left out.
    pub layers: Vec<(String, usize)>,
    /// Blocks with their reference count on the sheet shown (visible layers), most first.
    pub blocks: Vec<(String, usize)>,
    pub takeoff: Vec<Total>,
    /// "no BOQ loaded" until the BOQ tasks land.
    pub boq: String,
}

impl ProjectSummary {
    pub fn none() -> ProjectSummary {
        ProjectSummary {
            boq: "no BOQ loaded".into(),
            ..Default::default()
        }
    }

    pub fn of(session: &Session) -> ProjectSummary {
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
        let sheet_name = info
            .sheets
            .iter()
            .find(|s| s.block == sheet)
            .map_or(sheet.clone(), |s| s.name.clone());
        ProjectSummary {
            open: true,
            sheets: info.sheets.into_iter().map(|s| s.name).collect(),
            sheet: sheet_name,
            layers,
            blocks,
            takeoff: session.takeoff.totals(),
            boq: "no BOQ loaded".into(),
        }
    }
}

const EN: &str = "\
You are the AI staff inside Qiyas, a measurement CAD by QS Mind, working for a quantity surveyor. \
You read the open drawing with your tools, measure quantities, add them to the take-off, and \
explain what you did.

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
- Answer in the user's language, briefly, in plain words.";

const AR: &str = "\
أنت فريق الذكاء الاصطناعي داخل قياس، برنامج القياس من كيو إس مايند، وتعمل لدى مهندس حصر كميات. \
تقرأ المخطط المفتوح بأدواتك، وتقيس الكميات، وتضيفها إلى جدول الحصر، وتشرح ما فعلت.

القواعد:
- لا تذكر أي كمية إلا إذا جاءت من قياس أو من نتيجة أداة في هذه المحادثة. إن لم تقسها فقل ذلك واعرض أن تقيسها.
- فضّل قياس العناصر المرسومة (بالمعرّف من find_entities) على تحديد النقاط، فهو يقيس الشكل المرسوم بدقة. \
استخدم النقاط فقط حين لا يناسب أي عنصر.
- اذكر اللوحات والطبقات التي استخدمتها لكل كمية، ليتحقق منها المستخدم.
- انظر قبل أن تقيس: اعرض الطبقات، وابحث عن العناصر، واقرأ النصوص، وانظر إلى المخطط حين تكون أسماء الطبقات غير واضحة.
- الإحداثيات والأطوال والمساحات من المخطط بوحدات الرسم. أما القياسات فتعطي كميتها بالمتر أو المتر المربع أو المتر المكعب أو بالعدد.
- أعط كل قياس وصفًا واضحًا: ماذا قيس وأين.
- إن كان شيء في المخطط غير واضح أو يبدو خطأ فقل ذلك ولا تخمّن.
- أجب بلغة المستخدم، باختصار وبكلمات بسيطة.";

/// The system prompt in `lang`, with `units` (the drawing's unit name, e.g. "millimetres") and
/// the project summary. Names from the drawing are given as they are.
pub fn system_prompt(lang: Lang, units: &str, summary: &ProjectSummary) -> String {
    let ar = lang == Lang::Ar;
    let t = |en: &'static str, arabic: &'static str| if ar { arabic } else { en };
    let mut s = String::from(if ar { AR } else { EN });

    s.push_str(t("\n\n## The drawing\n", "\n\n## المخطط\n"));
    if !summary.open {
        s.push_str(t(
            "No drawing is open. Ask the user to open one.\n",
            "لا يوجد مخطط مفتوح. اطلب من المستخدم أن يفتح مخططًا.\n",
        ));
    } else {
        let _ = writeln!(s, "{}{units}", t("Units: ", "الوحدات: "));
        let _ = writeln!(
            s,
            "{}{}",
            t("Sheets: ", "اللوحات: "),
            summary.sheets.join(", ")
        );
        let _ = writeln!(
            s,
            "{}{}",
            t("Sheet shown: ", "اللوحة المعروضة: "),
            summary.sheet
        );
        s.push_str(t(
            "Layers on the sheet shown (entities):\n",
            "الطبقات في اللوحة المعروضة (عدد العناصر):\n",
        ));
        list(&mut s, &summary.layers, MAX_LAYERS, ar);
        s.push_str(t(
            "Blocks on the sheet shown (references):\n",
            "الكتل في اللوحة المعروضة (عدد المراجع):\n",
        ));
        list(&mut s, &summary.blocks, MAX_BLOCKS, ar);
    }

    s.push_str(t("\n## The take-off\n", "\n## جدول الحصر\n"));
    if summary.takeoff.is_empty() {
        s.push_str(t("No measurements yet.\n", "لا توجد قياسات بعد.\n"));
    } else {
        for total in &summary.takeoff {
            let kind = serde_json::to_value(total.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            let _ = writeln!(
                s,
                "- {kind}: {} {}, {} {:.3} {}",
                total.count,
                t("measurements", "قياسات"),
                t("total", "المجموع"),
                total.sum,
                total.unit
            );
        }
    }

    s.push_str(t("\n## The BOQ\n", "\n## جدول الكميات\n"));
    let boq = if summary.boq == "no BOQ loaded" && ar {
        "لم يُحمّل جدول كميات."
    } else {
        summary.boq.as_str()
    };
    let _ = writeln!(s, "{boq}");
    s
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
        let en = system_prompt(Lang::En, "", &ProjectSummary::none());
        assert!(en.contains("No drawing is open"));
        assert!(en.contains("no BOQ loaded"));
        let ar = system_prompt(Lang::Ar, "", &ProjectSummary::none());
        assert!(ar.contains("قياس"));
        assert!(ar.contains("كيو إس مايند"));
        assert!(ar.contains("لا يوجد مخطط مفتوح"));
    }

    #[test]
    fn summary_lists_layers_blocks_and_takeoff() {
        let dir = tempfile::tempdir().unwrap();
        let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
        let mut session = Session::open(&path).unwrap();
        session
            .run(qiyas_core::Command::CountBlock {
                block_name: "DOOR".into(),
                layer: None,
            })
            .unwrap();
        let summary = ProjectSummary::of(&session);
        assert!(summary.layers.contains(&("A-WALL".into(), 3)));
        assert!(summary.layers.contains(&("A-DOOR".into(), 3)));
        assert_eq!(summary.blocks, vec![("DOOR".to_string(), 3)]);
        let text = system_prompt(Lang::En, &session.info().units, &summary);
        assert!(text.contains("Units: metres"), "{text}");
        assert!(text.contains("- A-WALL: 3"), "{text}");
        assert!(text.contains("- DOOR: 3"), "{text}");
        assert!(
            text.contains("- count: 1 measurements, total 3.000 pcs"),
            "{text}"
        );
    }
}

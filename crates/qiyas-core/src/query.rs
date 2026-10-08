//! Read-only questions about the open drawing: what is there, what it says, what it measures.
//! These are the AI's eyes; nothing here changes the session.

use std::collections::{BTreeSet, HashMap};

use opencadcodec::entities::mtext_format::parse_mtext;
use opencadcodec::entities::Dimension;
use opencadcodec::tables::normalize_name;
use opencadcodec::{CadDocument, EntityType};
use qiyas_geom::{flatten, insert_instances, Xform};
use serde::{Deserialize, Serialize};

use crate::session::{variant_name, Extent};
use crate::{Session, BATCH_TOL};

/// Most rows one `find_entities` page returns.
const MAX_LIMIT: usize = 500;
/// Deepest block nesting `read_text` follows; deeper inserts (or cycles) are skipped.
const MAX_DEPTH: usize = 32;
/// Most block instances `read_text` walks, so a huge array of blocks cannot hang the app.
const MAX_INSTANCES: usize = 100_000;

/// Which entities `find_entities` returns. Every filter that is set must match; names match
/// case-insensitively. Missing fields take their defaults when read from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EntityFilter {
    /// Sheet block or layout name; the current sheet when `None`.
    pub sheet: Option<String>,
    pub layer: Option<String>,
    /// The entity type, e.g. "Line", "LwPolyline", "Insert", "Text".
    pub kind: Option<String>,
    /// Only inserts of this block.
    pub block_name: Option<String>,
    /// Text in a Text or MText, or in an insert's attribute values.
    pub text_contains: Option<String>,
    /// `[[min_x, min_y], [max_x, max_y]]`: entities with at least one point inside.
    pub within: Option<[[f64; 2]; 2]>,
    pub offset: usize,
    /// Rows per page, at most 500. 0 returns only the total.
    pub limit: usize,
}

impl Default for EntityFilter {
    fn default() -> Self {
        EntityFilter {
            sheet: None,
            layer: None,
            kind: None,
            block_name: None,
            text_contains: None,
            within: None,
            offset: 0,
            limit: 100,
        }
    }
}

/// One entity found. Length and area are in drawing units; bounds in world coordinates.
#[derive(Debug, Clone, Serialize)]
pub struct EntityRow {
    pub handle: u64,
    pub kind: String,
    pub layer: String,
    pub block_name: Option<String>,
    /// A Text's value, an MText's plain text, or an insert's attribute values joined by " | ".
    pub text: Option<String>,
    pub length: Option<f64>,
    pub area: Option<f64>,
    /// `[[min_x, min_y], [max_x, max_y]]`.
    pub bounds: [[f64; 2]; 2],
}

/// One page of results. `total` counts every match; `next_offset` is set when more remain.
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: usize,
    pub next_offset: Option<usize>,
}

/// One piece of text in world coordinates. `handle` is the Text or MText, or the insert that
/// carries an attribute.
#[derive(Debug, Clone, Serialize)]
pub struct TextRow {
    pub handle: u64,
    pub text: String,
    pub at: [f64; 2],
    pub height: f64,
    /// The block the text sits in, when it was reached through an insert.
    pub inside_block: Option<String>,
}

/// One dimension. `measurement` is computed from its definition points: drawing units, or
/// degrees for angular ones; `None` for kinds not measured yet.
#[derive(Debug, Clone, Serialize)]
pub struct DimRow {
    pub handle: u64,
    /// "Linear", "Aligned", "Angular", "Angular3Point", "Diameter", "Radius", ...
    pub kind: String,
    pub measurement: Option<f64>,
    /// The text written instead of the measured value, if any.
    pub text_override: Option<String>,
    /// Where the dimension text sits.
    pub at: [f64; 2],
}

/// How many references of one block sit on one layer.
#[derive(Debug, Clone, Serialize)]
pub struct BlockCount {
    pub block_name: String,
    pub layer: String,
    pub count: usize,
    /// The distinct attribute values (trimmed, not empty) of the references counted, sorted.
    pub attributes: Vec<String>,
}

impl Session {
    /// The entities of a sheet that pass `f`, in drawing order, one page at a time.
    pub fn find_entities(&self, f: &EntityFilter) -> Page<EntityRow> {
        let layer = f.layer.as_deref().map(normalize_name);
        let block = f.block_name.as_deref().map(normalize_name);
        let needle = f.text_contains.as_deref().map(str::to_lowercase);
        let within = f.within.map(normalized_box);
        let sheet = self.sheet_block(f.sheet.as_deref());
        let matches: Vec<&EntityType> = self
            .drawing
            .entities_in(&sheet)
            .filter(|e| {
                layer
                    .as_ref()
                    .is_none_or(|l| *l == normalize_name(&e.common().layer))
            })
            .filter(|e| {
                f.kind
                    .as_ref()
                    .is_none_or(|k| variant_name(e).eq_ignore_ascii_case(k))
            })
            .filter(|e| {
                block.as_ref().is_none_or(|b| match e {
                    EntityType::Insert(insert) => *b == normalize_name(&insert.block_name),
                    _ => false,
                })
            })
            .filter(|e| {
                needle.as_ref().is_none_or(|n| {
                    texts_of(e)
                        .iter()
                        .any(|t| t.to_lowercase().contains(n.as_str()))
                })
            })
            .filter(|e| within.is_none_or(|b| self.points_of(e).iter().any(|p| inside(*p, b))))
            .collect();
        let total = matches.len();
        let limit = f.limit.min(MAX_LIMIT);
        let rows: Vec<EntityRow> = matches
            .into_iter()
            .skip(f.offset)
            .take(limit)
            .map(|e| self.entity_row(e))
            .collect();
        let end = f.offset.saturating_add(rows.len());
        Page {
            next_offset: (end < total && !rows.is_empty()).then_some(end),
            rows,
            total,
        }
    }

    /// Every Text, MText (formatting stripped) and insert attribute on a sheet, including those
    /// inside blocks, in world coordinates. `within` keeps only text placed inside that box.
    pub fn read_text(&self, sheet: Option<&str>, within: Option<[[f64; 2]; 2]>) -> Vec<TextRow> {
        let sheet = self.sheet_block(sheet);
        let mut out = Vec::new();
        let mut budget = MAX_INSTANCES;
        collect_text(
            &self.drawing.doc,
            self.drawing.entities_in(&sheet),
            &Xform::identity(),
            None,
            0,
            &mut budget,
            &mut out,
        );
        if let Some(b) = within.map(normalized_box) {
            out.retain(|t| inside(t.at, b));
        }
        out
    }

    /// The dimensions on a sheet, with the values they measure.
    pub fn read_dimensions(&self, sheet: Option<&str>) -> Vec<DimRow> {
        let sheet = self.sheet_block(sheet);
        self.drawing
            .entities_in(&sheet)
            .filter_map(|e| match e {
                EntityType::Dimension(d) => Some(dim_row(d)),
                _ => None,
            })
            .collect()
    }

    /// Block references on a sheet per block and layer, on visible layers only, most first.
    /// Counts top-level references, like the count command.
    pub fn block_summary(&self, sheet: Option<&str>) -> Vec<BlockCount> {
        self.block_counts(sheet, true)
    }

    /// Block references on a sheet as the file has them: layers the user has hidden count, only
    /// layers off or frozen in the drawing do not. One row per block, layer and set of attribute
    /// values, so the references tagged "D1" are counted apart from those tagged "D2". Used by
    /// the comparison, which must not change with what the user is looking at.
    pub fn block_summary_in_file(&self, sheet: Option<&str>) -> Vec<BlockCount> {
        self.block_counts(sheet, false)
    }

    /// `block_summary` when `as_shown`, else `block_summary_in_file`.
    fn block_counts(&self, sheet: Option<&str>, as_shown: bool) -> Vec<BlockCount> {
        let sheet = self.sheet_block(sheet);
        // (block, layer, attribute values when split by them) -> (count, attribute values).
        type Key<'a> = (&'a str, &'a str, Vec<String>);
        let mut counts: HashMap<Key, (usize, BTreeSet<String>)> = HashMap::new();
        for e in self.drawing.entities_in(&sheet) {
            if let EntityType::Insert(insert) = e {
                let common = e.common();
                let counted = if as_shown {
                    self.layer_visible(&common.layer)
                } else {
                    self.layer_on(&common.layer)
                };
                if common.invisible || !counted {
                    continue;
                }
                let values: BTreeSet<String> = insert
                    .attributes
                    .iter()
                    .map(|a| a.value.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .collect();
                let split = if as_shown {
                    Vec::new()
                } else {
                    values.iter().cloned().collect()
                };
                let entry = counts
                    .entry((insert.block_name.as_str(), common.layer.as_str(), split))
                    .or_default();
                entry.0 += 1;
                entry.1.extend(values);
            }
        }
        let mut summary: Vec<BlockCount> = counts
            .into_iter()
            .map(|((block_name, layer, _), (count, values))| BlockCount {
                block_name: block_name.to_string(),
                layer: layer.to_string(),
                count,
                attributes: values.into_iter().collect(),
            })
            .collect();
        summary.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.block_name.cmp(&b.block_name))
                .then_with(|| a.layer.cmp(&b.layer))
                .then_with(|| a.attributes.cmp(&b.attributes))
        });
        summary
    }

    /// The block of the sheet named by its block or layout name (case-insensitive); the
    /// current sheet when `None`. An unknown name is used as a block name as it is.
    pub(crate) fn sheet_block(&self, sheet: Option<&str>) -> String {
        let Some(name) = sheet else {
            return self.sheet.clone();
        };
        self.drawing
            .sheets()
            .into_iter()
            .find(|s| s.block.eq_ignore_ascii_case(name) || s.name.eq_ignore_ascii_case(name))
            .map_or_else(|| name.to_string(), |s| s.block)
    }

    fn entity_row(&self, e: &EntityType) -> EntityRow {
        let Extent { length, area, .. } = self.extent(e);
        let texts = texts_of(e);
        EntityRow {
            handle: e.common().handle.value(),
            kind: variant_name(e),
            layer: e.common().layer.clone(),
            block_name: match e {
                EntityType::Insert(insert) => Some(insert.block_name.clone()),
                _ => None,
            },
            text: (!texts.is_empty()).then(|| texts.join(" | ")),
            length,
            area,
            bounds: bounds(&self.points_of(e)),
        }
    }

    /// The entity's flattened points in world coordinates; for an entity that does not
    /// flatten (a dimension or table), the corners of its own bounding box.
    fn points_of(&self, e: &EntityType) -> Vec<[f64; 2]> {
        let mut paths = Vec::new();
        flatten(
            &self.drawing.doc,
            e,
            &Xform::identity(),
            BATCH_TOL,
            &mut paths,
        );
        let points: Vec<[f64; 2]> = paths.into_iter().flat_map(|p| p.points).collect();
        if !points.is_empty() {
            return points;
        }
        let b = e.as_entity().bounding_box();
        vec![[b.min.x, b.min.y], [b.max.x, b.max.y]]
    }
}

/// The text an entity carries: a Text's value, an MText's plain text, an insert's attribute values.
fn texts_of(e: &EntityType) -> Vec<String> {
    match e {
        EntityType::Text(t) => vec![t.value.clone()],
        EntityType::MText(t) => vec![plain_text(&t.value)],
        EntityType::Insert(insert) => insert.attributes.iter().map(|a| a.value.clone()).collect(),
        _ => Vec::new(),
    }
}

/// MText (or table cell) content with its formatting codes removed; paragraphs on new lines.
pub(crate) fn plain_text(value: &str) -> String {
    parse_mtext(value, true).to_plain_text()
}

/// Appends the text of `entities`, placed by `xf`, walking into blocks.
fn collect_text<'a>(
    doc: &'a CadDocument,
    entities: impl Iterator<Item = &'a EntityType>,
    xf: &Xform,
    inside_block: Option<&str>,
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<TextRow>,
) {
    for e in entities {
        let handle = e.common().handle.value();
        let mut push = |text: String, at: &opencadcodec::Vector3, height: f64| {
            out.push(TextRow {
                handle,
                text,
                at: xf.apply([at.x, at.y]),
                height: scaled_height(xf, height),
                inside_block: inside_block.map(str::to_string),
            })
        };
        match e {
            EntityType::Text(t) => push(t.value.clone(), &t.insertion_point, t.height),
            EntityType::MText(t) => push(plain_text(&t.value), &t.insertion_point, t.height),
            EntityType::Insert(insert) => {
                // Attributes are stored where the insert sits, not in the block's coordinates.
                for a in &insert.attributes {
                    push(a.value.clone(), &a.insertion_point, a.height);
                }
                if depth >= MAX_DEPTH {
                    continue;
                }
                for instance in insert_instances(doc, insert, xf) {
                    if *budget == 0 {
                        return;
                    }
                    *budget -= 1;
                    collect_text(
                        doc,
                        doc.entities_in_block(&insert.block_name),
                        &instance,
                        Some(&insert.block_name),
                        depth + 1,
                        budget,
                        out,
                    );
                }
            }
            _ => {}
        }
    }
}

/// A text height after `xf`: the length its vertical stroke becomes.
fn scaled_height(xf: &Xform, height: f64) -> f64 {
    let [x0, y0] = xf.apply([0.0, 0.0]);
    let [x1, y1] = xf.apply([0.0, height]);
    (x1 - x0).hypot(y1 - y0)
}

fn dim_row(d: &Dimension) -> DimRow {
    let base = d.base();
    let measurement = match d {
        Dimension::Linear(_)
        | Dimension::Aligned(_)
        | Dimension::Radius(_)
        | Dimension::Diameter(_)
        | Dimension::Angular2Ln(_)
        | Dimension::Angular3Pt(_) => Some(d.measurement()).filter(|m| m.is_finite()),
        _ => None,
    };
    DimRow {
        handle: base.common.handle.value(),
        kind: format!("{:?}", base.dimension_type),
        measurement,
        text_override: base
            .text_override()
            .filter(|t| !t.is_empty() && *t != "<>")
            .map(str::to_string),
        at: [base.text_middle_point.x, base.text_middle_point.y],
    }
}

/// The box with its corners in order, whichever way round they were given.
fn normalized_box([a, b]: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    [
        [a[0].min(b[0]), a[1].min(b[1])],
        [a[0].max(b[0]), a[1].max(b[1])],
    ]
}

fn inside(p: [f64; 2], [lo, hi]: [[f64; 2]; 2]) -> bool {
    (lo[0]..=hi[0]).contains(&p[0]) && (lo[1]..=hi[1]).contains(&p[1])
}

/// `[[min_x, min_y], [max_x, max_y]]` of `points`; all zeros when there are none.
fn bounds(points: &[[f64; 2]]) -> [[f64; 2]; 2] {
    let Some(&first) = points.first() else {
        return [[0.0; 2]; 2];
    };
    points.iter().fold([first, first], |[lo, hi], p| {
        [
            [lo[0].min(p[0]), lo[1].min(p[1])],
            [hi[0].max(p[0]), hi[1].max(p[1])],
        ]
    })
}

use std::collections::{HashMap, HashSet};

use opencadcodec::tables::normalize_name;
use opencadcodec::{CadDocument, Color, Handle};
use qiyas_geom::{flatten, Path, Xform};

use crate::color::resolve_color;

/// First four bytes of a batch: "BYIQ" read as bytes, `0x51495942` as a little-endian `u32`.
pub const MAGIC: u32 = 0x5149_5942;
/// Layout version of `Batch::to_bytes`.
pub const VERSION: u32 = 2;
/// `Range::chain` of an entity drawn at the top level, outside any block.
pub const NO_CHAIN: u32 = u32::MAX;

/// Section kinds of `Batch::to_bytes`. Later layouts add kinds; readers skip unknown ones.
/// Kinds 1 and 2 (per-segment handles) are not used: the ranges carry the handles.
pub mod section {
    pub const RANGES: u32 = 3;
    pub const CHAINS: u32 = 4;
    pub const SEGMENTS: u32 = 5;
    pub const COLORS: u32 = 6;
}

/// The segments of one drawn entity: `first..first + count`. For an entity inside blocks,
/// `top` is the outermost block reference on the sheet, `entity` the entity itself and `chain`
/// an index into `Batch::chains`; at the top level `top == entity` and `chain == NO_CHAIN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub top: u64,
    pub entity: u64,
    pub chain: u32,
    pub first: u32,
    pub count: u32,
}

/// Line segments of one sheet, ready for the GPU.
///
/// Segment coordinates are relative to `origin`, so drawings far from (0, 0), such as
/// survey grids, keep their precision in `f32`. Add `origin` back for drawing coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Batch {
    /// `x0 y0 x1 y1` per segment, relative to `origin`.
    pub segments: Vec<f32>,
    /// `0xRRGGBB` per segment.
    pub colors: Vec<u32>,
    /// One per drawn entity, in segment order; every segment is in exactly one.
    pub ranges: Vec<Range>,
    /// The block-reference handles leading to each nested entity, outermost first.
    pub chains: Vec<Vec<u64>>,
    /// `[[min_x, min_y], [max_x, max_y]]` of all segments in drawing coordinates; zeros when empty.
    pub bounds: [[f64; 2]; 2],
    /// Centre of `bounds`; zeros when empty.
    pub origin: [f64; 2],
    /// Some block content was too deep or too large to draw (see `qiyas_geom::FlattenOutcome`).
    /// Not part of `to_bytes`.
    pub truncated: bool,
}

/// What the user has hidden on top of the drawing's own layer state.
/// Layer names match case-insensitively, as in DXF.
#[derive(Debug, Clone, Default)]
pub struct Visibility {
    pub hidden_layers: HashSet<String>,
}

/// Flattens the entities of `sheet_block` (e.g. `*Model_Space`) into line segments.
/// Skips invisible entities and entities on hidden, off or frozen layers; inside blocks this
/// goes by the inner entity's own layer (layer "0" meaning its insert's), so hiding a layer
/// hides only those parts of a block. Colours are the inner entity's, ByBlock and ByLayer
/// resolved as AutoCAD does.
pub fn build_batch(doc: &CadDocument, sheet_block: &str, vis: &Visibility, tol: f64) -> Batch {
    let hidden: HashSet<String> = vis
        .hidden_layers
        .iter()
        .map(|n| normalize_name(n))
        .collect();
    // By layer name as written, so the many paths of a block look their layer up once.
    let mut shown_layers: HashMap<String, bool> = HashMap::new();
    let mut shown = |layer: &str| match shown_layers.get(layer) {
        Some(&shown) => shown,
        None => {
            let shown = !hidden.contains(&normalize_name(layer))
                && !doc
                    .layers
                    .get(layer)
                    .is_some_and(|l| l.flags.off || l.flags.frozen);
            shown_layers.insert(layer.to_string(), shown);
            shown
        }
    };
    let mut batch = Batch::default();
    let mut chain_index: HashMap<Vec<u64>, u32> = HashMap::new();
    let mut chain_shown: HashMap<Vec<u64>, bool> = HashMap::new();
    // Absolute `x0 y0 x1 y1` per segment; made relative once the bounds are known.
    let mut absolute: Vec<[f64; 4]> = Vec::new();
    for entity in doc.entities_in_block(sheet_block) {
        let common = entity.common();
        if common.invisible || !shown(&common.layer) {
            continue;
        }
        let top = common.handle.value();
        let outcome = flatten(doc, entity, &Xform::identity(), tol);
        batch.truncated |= outcome.truncated;
        for flat in &outcome.paths {
            if !shown(&flat.layer) {
                continue;
            }
            // A nested block reference on a hidden layer hides everything inside it.
            let chain_shown = match chain_shown.get(flat.chain.as_slice()) {
                Some(&v) => v,
                None => {
                    let v = nested_inserts_shown(doc, &flat.chain, &common.layer, &mut shown);
                    chain_shown.insert(flat.chain.clone(), v);
                    v
                }
            };
            if !chain_shown {
                continue;
            }
            let [r, g, b] = resolve_color(&flat.color, &Color::ByLayer);
            let rgb = u32::from_be_bytes([0, r, g, b]);
            let first = absolute.len();
            for (a, b) in segments(&flat.path) {
                absolute.push([a[0], a[1], b[0], b[1]]);
                batch.colors.push(rgb);
            }
            let count = absolute.len() - first;
            if count == 0 {
                continue;
            }
            let chain = if flat.chain.is_empty() {
                NO_CHAIN
            } else if let Some(&i) = chain_index.get(flat.chain.as_slice()) {
                i
            } else {
                let i = batch.chains.len() as u32;
                chain_index.insert(flat.chain.clone(), i);
                batch.chains.push(flat.chain.clone());
                i
            };
            // A path of the same entity right after its last one extends that range.
            match batch.ranges.last_mut() {
                Some(last)
                    if last.top == top
                        && last.entity == flat.entity
                        && last.chain == chain
                        && (last.first + last.count) as usize == first =>
                {
                    last.count += count as u32;
                }
                _ => batch.ranges.push(Range {
                    top,
                    entity: flat.entity,
                    chain,
                    first: first as u32,
                    count: count as u32,
                }),
            }
        }
    }
    if absolute.is_empty() {
        return batch;
    }
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for s in &absolute {
        lo = [lo[0].min(s[0]).min(s[2]), lo[1].min(s[1]).min(s[3])];
        hi = [hi[0].max(s[0]).max(s[2]), hi[1].max(s[1]).max(s[3])];
    }
    let origin = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    batch.segments = absolute
        .iter()
        .flat_map(|s| {
            [
                s[0] - origin[0],
                s[1] - origin[1],
                s[2] - origin[0],
                s[3] - origin[1],
            ]
            .map(|v| v as f32)
        })
        .collect();
    batch.bounds = [lo, hi];
    batch.origin = origin;
    batch
}

/// Whether every block reference of `chain` after the first (the top-level one, already checked)
/// sits on a shown layer. A reference on layer "0" is drawn on the layer of the one around it.
fn nested_inserts_shown(
    doc: &CadDocument,
    chain: &[u64],
    top_layer: &str,
    shown: &mut impl FnMut(&str) -> bool,
) -> bool {
    let mut layer = top_layer;
    for &h in chain.iter().skip(1) {
        let Some(insert) = doc.get_entity(Handle::new(h)) else {
            continue;
        };
        let own = insert.common().layer.as_str();
        if own != "0" {
            layer = own;
        }
        if !shown(layer) {
            return false;
        }
    }
    true
}

/// A path of n points gives n - 1 segments, plus the closing one when closed
/// (same rule as `Path::length`, so a closed two-point path is not drawn twice).
fn segments(path: &Path) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
    let n = path.points.len();
    let count = if path.closed && n > 2 {
        n
    } else {
        n.saturating_sub(1)
    };
    (0..count).map(move |i| (path.points[i], path.points[(i + 1) % n]))
}

impl Batch {
    /// The range holding segment `i`.
    pub fn range_of(&self, i: usize) -> Option<&Range> {
        let at = self
            .ranges
            .partition_point(|r| (r.first + r.count) as usize <= i);
        self.ranges.get(at).filter(|r| r.first as usize <= i)
    }

    /// The block-reference chain of a range; empty at the top level.
    pub fn chain_of(&self, range: &Range) -> &[u64] {
        self.chains
            .get(range.chain as usize)
            .map_or(&[], Vec::as_slice)
    }

    /// Encodes the batch for the frontend: layout version 2. Little-endian; every section
    /// starts on an 8-byte boundary (zero padding before it). With k = sections (4 here):
    ///
    /// | offset   | size  | contents                                                     |
    /// |----------|-------|--------------------------------------------------------------|
    /// | 0        | 4     | `u32` magic `0x51495942`                                     |
    /// | 4        | 4     | `u32` version, 2                                             |
    /// | 8        | 32    | `f64` x4 bounds: min x, min y, max x, max y (absolute)       |
    /// | 40       | 16    | `f64` x2 origin: x, y                                        |
    /// | 56       | 4     | `u32` k, the number of sections                              |
    /// | 60       | 12k   | per section: `u32` kind, `u32` offset, `u32` byte length     |
    ///
    /// Sections, in this order:
    ///
    /// | kind | name           | contents                                                    |
    /// |------|----------------|-------------------------------------------------------------|
    /// | 3    | ranges         | 32 bytes per range: `u64` top, `u64` entity, `u32` chain    |
    /// |      |                | index (`0xFFFFFFFF` = none), `u32` first, `u32` count, 4 pad |
    /// | 4    | chains         | per chain: `u32` length m, then m `u64` handles, outermost  |
    /// |      |                | first; chains are numbered from 0 in order                  |
    /// | 5    | segments       | `f32` x0 y0 x1 y1 per segment, relative to origin           |
    /// | 6    | colours        | `u32` 0xRRGGBB per segment                                  |
    ///
    /// Every segment belongs to exactly one range, so the ranges give each segment's handles.
    /// Kinds 1 and 2 are unused. Readers find sections by kind and skip kinds they don't know, so
    /// a later layout can add sections (glyphs, images) without a new version.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut chains = Vec::new();
        for chain in &self.chains {
            chains.extend((chain.len() as u32).to_le_bytes());
            for h in chain {
                chains.extend(h.to_le_bytes());
            }
        }
        let mut ranges = Vec::with_capacity(self.ranges.len() * 32);
        for r in &self.ranges {
            ranges.extend(r.top.to_le_bytes());
            ranges.extend(r.entity.to_le_bytes());
            ranges.extend(r.chain.to_le_bytes());
            ranges.extend(r.first.to_le_bytes());
            ranges.extend(r.count.to_le_bytes());
            ranges.extend([0u8; 4]);
        }
        let sections: [(u32, Vec<u8>); 4] = [
            (section::RANGES, ranges),
            (section::CHAINS, chains),
            (
                section::SEGMENTS,
                le_bytes(&self.segments, f32::to_le_bytes),
            ),
            (section::COLORS, le_bytes(&self.colors, u32::to_le_bytes)),
        ];

        let mut out = Vec::new();
        out.extend(MAGIC.to_le_bytes());
        out.extend(VERSION.to_le_bytes());
        for v in self.bounds.as_flattened().iter().chain(&self.origin) {
            out.extend(v.to_le_bytes());
        }
        out.extend((sections.len() as u32).to_le_bytes());
        let mut at = align8(60 + 12 * sections.len());
        for (kind, bytes) in &sections {
            out.extend(kind.to_le_bytes());
            out.extend((at as u32).to_le_bytes());
            out.extend((bytes.len() as u32).to_le_bytes());
            at = align8(at + bytes.len());
        }
        for (_, bytes) in &sections {
            out.resize(align8(out.len()), 0);
            out.extend(bytes);
        }
        out
    }
}

fn le_bytes<T: Copy, const N: usize>(values: &[T], f: fn(T) -> [u8; N]) -> Vec<u8> {
    values.iter().flat_map(|v| f(*v)).collect()
}

fn align8(n: usize) -> usize {
    n.next_multiple_of(8)
}

use std::collections::HashSet;

use opencadcodec::tables::normalize_name;
use opencadcodec::{CadDocument, Color};
use qiyas_geom::{flatten, Path, Xform};

use crate::color::resolve_color;

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
    /// Entity handle per segment.
    pub handles: Vec<u64>,
    /// `(handle, first_segment, count)` per drawn entity.
    pub ranges: Vec<(u64, u32, u32)>,
    /// `[[min_x, min_y], [max_x, max_y]]` of all segments in drawing coordinates; zeros when empty.
    pub bounds: [[f64; 2]; 2],
    /// Centre of `bounds`; zeros when empty.
    pub origin: [f64; 2],
}

/// What the user has hidden on top of the drawing's own layer state.
/// Layer names match case-insensitively, as in DXF.
#[derive(Debug, Clone, Default)]
pub struct Visibility {
    pub hidden_layers: HashSet<String>,
}

/// Flattens the entities of `sheet_block` (e.g. `*Model_Space`) into line segments.
/// Skips invisible entities and entities on hidden, off or frozen layers.
///
/// Every segment of an Insert takes the colour resolved for the Insert itself (ByLayer means
/// the insert's layer); the block's own entity colours are not followed yet.
pub fn build_batch(doc: &CadDocument, sheet_block: &str, vis: &Visibility, tol: f64) -> Batch {
    let hidden: HashSet<String> = vis
        .hidden_layers
        .iter()
        .map(|n| normalize_name(n))
        .collect();
    let mut batch = Batch::default();
    // Absolute `x0 y0 x1 y1` per segment; made relative once the bounds are known.
    let mut absolute: Vec<[f64; 4]> = Vec::new();
    let mut paths = Vec::new();
    for entity in doc.entities_in_block(sheet_block) {
        let common = entity.common();
        if common.invisible || hidden.contains(&normalize_name(&common.layer)) {
            continue;
        }
        let layer = doc.layers.get(&common.layer);
        if layer.is_some_and(|l| l.flags.off || l.flags.frozen) {
            continue;
        }
        let layer_color = layer.map_or(Color::ByLayer, |l| l.color);
        let [r, g, b] = resolve_color(&common.color, &layer_color);
        let rgb = u32::from_be_bytes([0, r, g, b]);
        let handle = common.handle.value();

        paths.clear();
        flatten(doc, entity, &Xform::identity(), tol, &mut paths);
        let first = absolute.len();
        for (a, b) in paths.iter().flat_map(segments) {
            absolute.push([a[0], a[1], b[0], b[1]]);
            batch.colors.push(rgb);
            batch.handles.push(handle);
        }
        let count = absolute.len() - first;
        if count > 0 {
            batch.ranges.push((handle, first as u32, count as u32));
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
    /// Encodes the batch for the frontend. Little-endian, no padding, and every section
    /// starts aligned for a JS TypedArray view. With n = segment count, r = range count:
    ///
    /// | offset              | size  | contents                                            |
    /// |---------------------|-------|-----------------------------------------------------|
    /// | 0                   | 4     | `u32` n                                             |
    /// | 4                   | 4     | `u32` r                                             |
    /// | 8                   | 32    | `f64` x4 bounds: min x, min y, max x, max y (absolute) |
    /// | 40                  | 16    | `f64` x2 origin: x, y                               |
    /// | 56                  | 8n    | handles: `u64` per segment                          |
    /// | 56 + 8n             | 16r   | ranges: `u64` handle, `u32` first, `u32` count      |
    /// | 56 + 8n + 16r       | 16n   | segments: `f32` x0 y0 x1 y1, relative to origin     |
    /// | 56 + 24n + 16r      | 4n    | colors: `u32` 0xRRGGBB per segment                  |
    ///
    /// Total length: 56 + 28n + 16r bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.handles.len();
        let mut out = Vec::with_capacity(56 + n * 28 + self.ranges.len() * 16);
        out.extend((n as u32).to_le_bytes());
        out.extend((self.ranges.len() as u32).to_le_bytes());
        for v in self.bounds.as_flattened().iter().chain(&self.origin) {
            out.extend(v.to_le_bytes());
        }
        for v in &self.handles {
            out.extend(v.to_le_bytes());
        }
        for (handle, first, count) in &self.ranges {
            out.extend(handle.to_le_bytes());
            out.extend(first.to_le_bytes());
            out.extend(count.to_le_bytes());
        }
        for v in &self.segments {
            out.extend(v.to_le_bytes());
        }
        for v in &self.colors {
            out.extend(v.to_le_bytes());
        }
        out
    }
}

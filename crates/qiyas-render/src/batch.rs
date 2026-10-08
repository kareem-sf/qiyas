use std::collections::HashSet;

use opencadcodec::{CadDocument, Color};
use qiyas_geom::{flatten, Path, Xform};

use crate::color::resolve_color;

/// Line segments of one sheet, ready for the GPU.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Batch {
    /// `x0 y0 x1 y1` per segment.
    pub segments: Vec<f32>,
    /// `0xRRGGBB` per segment.
    pub colors: Vec<u32>,
    /// Entity handle per segment.
    pub handles: Vec<u64>,
    /// `(handle, first_segment, count)` per drawn entity.
    pub ranges: Vec<(u64, u32, u32)>,
    /// `[[min_x, min_y], [max_x, max_y]]` of all segments; zeros when empty.
    pub bounds: [[f64; 2]; 2],
}

/// What the user has hidden on top of the drawing's own layer state.
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
    let mut batch = Batch::default();
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    let mut paths = Vec::new();
    for entity in doc.entities_in_block(sheet_block) {
        let common = entity.common();
        if common.invisible || vis.hidden_layers.contains(&common.layer) {
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
        let first = batch.handles.len();
        for (a, b) in paths.iter().flat_map(segments) {
            batch
                .segments
                .extend([a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32]);
            batch.colors.push(rgb);
            batch.handles.push(handle);
            for p in [a, b] {
                lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                hi = [hi[0].max(p[0]), hi[1].max(p[1])];
            }
        }
        let count = batch.handles.len() - first;
        if count > 0 {
            batch.ranges.push((handle, first as u32, count as u32));
        }
    }
    if !batch.handles.is_empty() {
        batch.bounds = [lo, hi];
    }
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
    /// Little-endian, no padding: `u32 segment_count`, `u32 range_count`, `f64 x4` bounds
    /// (min x, min y, max x, max y), then segments as `f32`, colours as `u32`, handles as
    /// `u64`, and ranges as `(u64 handle, u32 first, u32 count)`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.handles.len();
        let mut out = Vec::with_capacity(40 + n * 28 + self.ranges.len() * 16);
        out.extend((n as u32).to_le_bytes());
        out.extend((self.ranges.len() as u32).to_le_bytes());
        for v in self.bounds.as_flattened() {
            out.extend(v.to_le_bytes());
        }
        for v in &self.segments {
            out.extend(v.to_le_bytes());
        }
        for v in &self.colors {
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
        out
    }
}

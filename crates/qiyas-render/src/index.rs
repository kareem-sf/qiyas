use rstar::primitives::{GeomWithData, Line};
use rstar::{PointDistance, RTree};

use crate::Batch;

type Segment = GeomWithData<Line<[f64; 2]>, u64>;

/// Spatial index over a batch's segments, for picking entities under the cursor.
/// The tree holds the batch's origin-relative segments; `pick` takes drawing (absolute)
/// coordinates and subtracts the origin itself.
pub struct PickIndex {
    tree: RTree<Segment>,
    origin: [f64; 2],
}

impl PickIndex {
    pub fn new(batch: &Batch) -> Self {
        let segments = batch
            .segments
            .as_chunks::<4>()
            .0
            .iter()
            .zip(&batch.handles)
            .map(|(s, &handle)| {
                let [x0, y0, x1, y1] = s.map(f64::from);
                GeomWithData::new(Line::new([x0, y0], [x1, y1]), handle)
            })
            .collect();
        Self {
            tree: RTree::bulk_load(segments),
            origin: batch.origin,
        }
    }

    /// Handle of the segment nearest to `p` (drawing coordinates), if it lies within `tol`.
    pub fn pick(&self, p: [f64; 2], tol: f64) -> Option<u64> {
        let p = [p[0] - self.origin[0], p[1] - self.origin[1]];
        let nearest = self.tree.nearest_neighbor(&p)?;
        (nearest.distance_2(&p) <= tol * tol).then_some(nearest.data)
    }
}

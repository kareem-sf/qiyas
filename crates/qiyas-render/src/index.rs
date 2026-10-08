use rstar::primitives::{GeomWithData, Line};
use rstar::{PointDistance, RTree};

use crate::Batch;

type Segment = GeomWithData<Line<[f64; 2]>, u64>;

/// Spatial index over a batch's segments, for picking entities under the cursor.
pub struct PickIndex {
    tree: RTree<Segment>,
}

impl PickIndex {
    pub fn new(batch: &Batch) -> Self {
        let segments = batch
            .segments
            .chunks_exact(4)
            .zip(&batch.handles)
            .map(|(s, &handle)| {
                let [x0, y0, x1, y1] = [s[0], s[1], s[2], s[3]].map(f64::from);
                GeomWithData::new(Line::new([x0, y0], [x1, y1]), handle)
            })
            .collect();
        Self {
            tree: RTree::bulk_load(segments),
        }
    }

    /// Handle of the segment nearest to `p`, if it lies within `tol`.
    pub fn pick(&self, p: [f64; 2], tol: f64) -> Option<u64> {
        let nearest = self.tree.nearest_neighbor(&p)?;
        (nearest.distance_2(&p) <= tol * tol).then_some(nearest.data)
    }
}

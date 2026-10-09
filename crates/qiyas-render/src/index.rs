use rstar::primitives::{GeomWithData, Line};
use rstar::{PointDistance, RTree};

use crate::Batch;

type Segment = GeomWithData<Line<[f64; 2]>, usize>;

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
            .enumerate()
            .map(|(i, s)| {
                let [x0, y0, x1, y1] = s.map(f64::from);
                GeomWithData::new(Line::new([x0, y0], [x1, y1]), i)
            })
            .collect();
        Self {
            tree: RTree::bulk_load(segments),
            origin: batch.origin,
        }
    }

    /// Index (into the batch it was built from) of the segment nearest to `p` (drawing
    /// coordinates), if it lies within `tol`.
    pub fn pick(&self, p: [f64; 2], tol: f64) -> Option<usize> {
        let p = [p[0] - self.origin[0], p[1] - self.origin[1]];
        let nearest = self.tree.nearest_neighbor(&p)?;
        (nearest.distance_2(&p) <= tol * tol).then_some(nearest.data)
    }

    /// Indices of the segments within `tol` of `p` (drawing coordinates), nearest first, at most
    /// `max` of them.
    pub fn near(&self, p: [f64; 2], tol: f64, max: usize) -> Vec<usize> {
        let p = [p[0] - self.origin[0], p[1] - self.origin[1]];
        self.tree
            .nearest_neighbor_iter_with_distance_2(&p)
            .take_while(|(_, d2)| *d2 <= tol * tol)
            .take(max)
            .map(|(s, _)| s.data)
            .collect()
    }
}

/// Release only: guards the `codegen-units = 1` setting for this crate in `Cargo.toml`.
/// Without it rstar's bulk load runs about 4 times slower. Run by `scripts/check.sh`.
#[cfg(all(test, not(debug_assertions)))]
mod tests {
    use super::*;

    #[test]
    fn pick_index_one_million_segments_under_a_second() {
        // Short segments scattered over 1 km, as a big drawing has them.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut batch = Batch::default();
        for _ in 0..1_000_000 {
            let (x, y) = (next() * 1000.0 - 500.0, next() * 1000.0 - 500.0);
            batch
                .segments
                .extend([x, y, x + next() * 2.0, y + next() * 2.0]);
        }
        let t = std::time::Instant::now();
        let index = PickIndex::new(&batch);
        let secs = t.elapsed().as_secs_f64();
        assert!(index.pick([0.0, 0.0], 1000.0).is_some());
        assert!(
            secs < 1.0,
            "PickIndex::new took {secs:.2} s for 1 M segments"
        );
    }
}

//! Pure 2D geometry: flatten CAD entities to polylines, measure length and area.

mod flatten;
mod path;
mod transform;

pub use flatten::{arc_points, flatten, insert_instances};
pub use path::Path;
pub use transform::Xform;

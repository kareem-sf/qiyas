//! Pure 2D geometry: flatten CAD entities to polylines, measure length and area.

mod exact;
mod flatten;
mod path;
mod transform;

pub use exact::{exact_area, exact_length};
pub use flatten::{
    arc_points, flatten, insert_instances, object_to_world, FlatPath, FlattenOutcome,
};
pub use path::Path;
pub use transform::Xform;

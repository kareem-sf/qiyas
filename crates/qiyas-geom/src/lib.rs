//! Pure 2D geometry: flatten CAD entities to polylines, measure length and area.

mod exact;
mod flatten;
mod path;
mod shapes;
mod transform;

pub use exact::{exact_area, exact_length};
pub use flatten::{
    arc_points, flatten, insert_instances, object_to_world, FlatPath, FlatText, FlattenOutcome,
    HAlign, TextLayout, VAlign,
};
pub use path::Path;
pub use shapes::{snap_shapes, SnapShape};
pub use transform::Xform;

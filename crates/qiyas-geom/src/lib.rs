//! Pure 2D geometry: flatten CAD entities to polylines, measure length and area.

mod exact;
mod flatten;
mod paint;
mod path;
mod shapes;
mod transform;

pub use exact::{exact_area, exact_length};
pub use flatten::{
    arc_points, drawn_style, find_layer, flatten, flatten_with, flatten_within, insert_instances,
    is_overlay_block, is_xref_block, object_to_world, xref_symbol, FlatPath, FlatText,
    FlattenOutcome, HAlign, TextLayout, VAlign, XrefDocs, MAX_WORK, MAX_XREF_DEPTH,
};
pub use paint::{FlatFill, Paint};
pub use path::Path;
pub use shapes::{snap_shapes, SnapShape};
pub use transform::Xform;

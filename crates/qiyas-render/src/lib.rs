//! Turns a sheet of the document into GPU buffers (line segments and glyph quads over a glyph
//! atlas), and picks entities near a point.

mod atlas;
mod batch;
mod color;
mod fonts;
mod index;
mod text;
mod visibility;

pub use atlas::{Atlas, PageView, PAGE_SIZE};
pub use batch::{
    build_batch, build_batch_with_text, place_text, section, Batch, Range, TextOptions, Visibility,
    MAGIC, NO_CHAIN, TEXT_BOX, VERSION,
};
pub use color::resolve_color;
pub use fonts::{DEFAULT_CHAIN, NOTO_ARABIC, NOTO_SANS};
pub use index::PickIndex;
pub use text::{
    decode_codes, fix_reversed_arabic, size_bucket, LaidText, Quad, ShapedGlyph, ShapedLine,
    TextEngine, QUAD_IN_BLOCK, SIZE_BUCKETS,
};
pub use visibility::{layer_on_in_file, layer_shown};

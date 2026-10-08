//! Turns a sheet of the document into GPU line buffers, and picks entities near a point.

mod batch;
mod color;
mod index;

pub use batch::{build_batch, Batch, Visibility};
pub use color::resolve_color;
pub use index::PickIndex;

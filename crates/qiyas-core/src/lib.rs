//! Qiyas core: the document, selection, undo and redo, command bus, PDF import and underlays,
//! raster images, inquiry (geometry, divide and measure along, data extraction), drawing new
//! entities and modifying them, text and find and replace, dimensions, leaders and tolerances.

mod boq;
mod commands;
mod conflict;
mod dimension;
mod draw;
mod drawing;
mod embed;
mod error;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
mod image;
mod inquiry;
mod modify;
mod modify2;
mod project;
mod query;
mod session;
mod snap;
mod tables;
mod takeoff;
mod text;
mod underlay;
mod undo;
mod units;
mod views;
mod xref;

pub use self::image::{draw_picture, image_size, ImageDisplay, IMAGE_EXTENSIONS, IMAGE_LAYER};
pub use boq::{Boq, BoqItem};
pub use commands::{Command, Outcome, Target};
pub use conflict::{Conflict, ConflictKind, ConflictStatus};
pub use dimension::{DimEditOp, DimSpec, DimStyleFields, DimStyleInfo};
pub use draw::{Boundary, Current, DrawSpec, HatchPattern, BOUNDARY_SEGMENTS};
pub use drawing::{short_layer_name, Drawing, LayerInfo, Sheet};
pub use embed::{embed_measurements, read_embedded};
pub use error::CoreError;
pub use inquiry::{Field, POINT_LAYER};
pub use modify::ModifySpec;
pub use modify2::{EditSpec, LengthenBy, PEditOp};
pub use project::{project_path, Project};
pub use query::{BlockCount, DimRow, EntityFilter, EntityRow, Page, TextRow};
pub use session::{DrawingInfo, EntityInfo, Picked, Session, BATCH_TOL, EMPTY_MODEL};
pub use snap::{Drafting, Snap, SnapKind, SnapModes};
pub use tables::Schedule;
pub use takeoff::{Kind, Measurement, Source, Takeoff, Total};
pub use text::{TextMatch, TextScope};
pub use underlay::{AttachMode, PictureSource, PreparedPdf, UnderlayImage, UNDERLAY_LAYER};
pub use units::Units;
pub use views::NamedView;
pub use xref::{candidates as xref_candidates, Xref, XrefInfo, XrefSet};

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::{self, Write};
use std::path::Path;

use opencadcodec::tables::normalize_name;
use opencadcodec::{EntityType, Handle};
use qiyas_geom::{flatten, Xform};
use qiyas_render::{build_batch, Batch, PickIndex, Visibility};
use serde::Serialize;

use crate::{CoreError, Drawing, LayerInfo, Sheet, Takeoff, Units};

/// Flatten tolerance for drawing on screen, in drawing units.
pub const BATCH_TOL: f64 = 0.05;
/// Flatten tolerance for measured lengths and areas, in drawing units. Finer than `BATCH_TOL`
/// so arcs measure close to their true length.
const MEASURE_TOL: f64 = 1e-3;

/// The open drawing and what the user is looking at. Tauri-free, so it can be tested directly.
pub struct Session {
    pub drawing: Drawing,
    /// Block name of the sheet on screen, e.g. `*Model_Space`.
    pub sheet: String,
    pub visibility: Visibility,
    pub takeoff: Takeoff,
    /// Batch and pick index of the last sheet and visibility drawn; rebuilt when either changes.
    cache: RefCell<Option<Cached>>,
}

struct Cached {
    sheet: String,
    hidden_layers: HashSet<String>,
    batch: Batch,
    index: PickIndex,
}

#[derive(Debug, Clone, Serialize)]
pub struct DrawingInfo {
    pub path: String,
    pub sheets: Vec<Sheet>,
    pub layers: Vec<LayerInfo>,
    /// "inches", "feet", "millimetres", "centimetres", "metres" or "unknown".
    pub units: String,
    pub problems: Vec<String>,
}

/// What the user sees about one picked entity. Length and area are in drawing units.
#[derive(Debug, Clone, Serialize)]
pub struct EntityInfo {
    pub handle: u64,
    /// The entity type, e.g. "Line", "LwPolyline", "Insert", "Text".
    pub kind: String,
    pub layer: String,
    pub length: Option<f64>,
    /// Sum of the closed paths' areas; `None` when the entity has no closed path.
    pub area: Option<f64>,
    pub block_name: Option<String>,
    pub text: Option<String>,
}

impl Session {
    /// Opens a drawing on its model sheet with nothing hidden.
    pub fn open(path: &Path) -> Result<Session, CoreError> {
        let drawing = Drawing::open(path)?;
        let sheet = drawing.sheets()[0].block.clone();
        Ok(Session {
            drawing,
            sheet,
            visibility: Visibility::default(),
            takeoff: Takeoff,
            cache: RefCell::new(None),
        })
    }

    pub fn info(&self) -> DrawingInfo {
        DrawingInfo {
            path: self.drawing.path.display().to_string(),
            sheets: self.drawing.sheets(),
            layers: self.drawing.layers(),
            units: units_name(self.drawing.units).to_string(),
            problems: self.drawing.problems.clone(),
        }
    }

    /// Shows the sheet whose block is `block` (case-insensitive).
    pub fn set_sheet(&mut self, block: &str) -> Result<(), CoreError> {
        let sheet = self
            .drawing
            .sheets()
            .into_iter()
            .find(|s| s.block.eq_ignore_ascii_case(block))
            .ok_or_else(|| CoreError::NoSuchSheet(block.to_string()))?;
        self.sheet = sheet.block;
        Ok(())
    }

    /// Hides or shows a layer on top of the drawing's own layer state. Names match case-insensitively.
    pub fn set_layer_hidden(&mut self, name: &str, hidden: bool) {
        let key = normalize_name(name);
        let layers = &mut self.visibility.hidden_layers;
        layers.retain(|n| normalize_name(n) != key);
        if hidden {
            layers.insert(name.to_string());
        }
    }

    /// The layers the user has hidden, sorted.
    pub fn hidden_layers(&self) -> Vec<String> {
        let mut names: Vec<String> = self.visibility.hidden_layers.iter().cloned().collect();
        names.sort();
        names
    }

    /// Line segments of the current sheet with the current visibility.
    pub fn batch(&self) -> Batch {
        self.with_cache(|cached| cached.batch.clone())
    }

    /// Handle of the entity nearest to `p` (drawing coordinates) within `tol`, among what is drawn.
    pub fn pick(&self, p: [f64; 2], tol: f64) -> Option<u64> {
        self.with_cache(|cached| cached.index.pick(p, tol))
    }

    pub fn entity_info(&self, handle: u64) -> Result<EntityInfo, CoreError> {
        let doc = &self.drawing.doc;
        let entity = doc
            .get_entity(Handle::new(handle))
            .ok_or(CoreError::NoSuchEntity(handle))?;
        let mut paths = Vec::new();
        flatten(doc, entity, &Xform::identity(), MEASURE_TOL, &mut paths);
        let length = (!paths.is_empty()).then(|| paths.iter().map(|p| p.length()).sum());
        let closed: Vec<_> = paths.iter().filter(|p| p.closed).collect();
        let area = (!closed.is_empty()).then(|| closed.iter().map(|p| p.area()).sum());
        let (block_name, text) = match entity {
            EntityType::Insert(insert) => (Some(insert.block_name.clone()), None),
            EntityType::Text(t) => (None, Some(t.value.clone())),
            EntityType::MText(t) => (None, Some(t.value.clone())),
            _ => (None, None),
        };
        Ok(EntityInfo {
            handle,
            kind: variant_name(entity),
            layer: entity.common().layer.clone(),
            length,
            area,
            block_name,
            text,
        })
    }

    /// Runs `f` on the cached batch, rebuilding it first if the sheet or visibility changed.
    fn with_cache<R>(&self, f: impl FnOnce(&Cached) -> R) -> R {
        let mut cache = self.cache.borrow_mut();
        let stale = cache.as_ref().is_none_or(|c| {
            c.sheet != self.sheet || c.hidden_layers != self.visibility.hidden_layers
        });
        if stale {
            let batch = build_batch(&self.drawing.doc, &self.sheet, &self.visibility, BATCH_TOL);
            *cache = Some(Cached {
                sheet: self.sheet.clone(),
                hidden_layers: self.visibility.hidden_layers.clone(),
                index: PickIndex::new(&batch),
                batch,
            });
        }
        f(cache.as_ref().expect("cache was just filled"))
    }
}

fn units_name(units: Units) -> &'static str {
    match units {
        Units::Inches => "inches",
        Units::Feet => "feet",
        Units::Millimetres => "millimetres",
        Units::Centimetres => "centimetres",
        Units::Metres => "metres",
        Units::Unknown => "unknown",
    }
}

/// The `EntityType` variant name, e.g. "LwPolyline". Taken from the derived `Debug` output,
/// which starts with the variant name; the writer stops at the first `(` so the entity's
/// data is never formatted.
fn variant_name(entity: &EntityType) -> String {
    struct UpToParen(String);
    impl Write for UpToParen {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            match s.find('(') {
                Some(i) => {
                    self.0.push_str(&s[..i]);
                    Err(fmt::Error)
                }
                None => {
                    self.0.push_str(s);
                    Ok(())
                }
            }
        }
    }
    let mut name = UpToParen(String::new());
    // The Err is our own early stop, not a failure.
    let _ = write!(name, "{entity:?}");
    name.0
}

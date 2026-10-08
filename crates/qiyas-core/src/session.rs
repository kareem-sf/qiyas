use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::{self, Write};
use std::path::{Path, PathBuf};

use opencadcodec::entities::Insert;
use opencadcodec::tables::normalize_name;
use opencadcodec::{CadDocument, DwgWriter, DxfWriter, EntityType, Handle};
use qiyas_geom::{exact_area, exact_length, flatten, insert_instances, Xform};
use qiyas_render::{build_batch, Batch, PickIndex, Visibility};
use serde::{Deserialize, Serialize};

use crate::commands::Part;
use crate::embed::{embed_measurements, read_embedded};
use crate::project::{self, project_path, Project};
use crate::undo::{History, Parts, Snapshot};
use crate::{
    Boq, Command, Conflict, CoreError, Drawing, LayerInfo, Outcome, Schedule, Sheet, Source,
    Takeoff, Units,
};

/// Flatten tolerance for drawing on screen, in drawing units.
pub const BATCH_TOL: f64 = 0.05;
/// Flatten tolerance for measured lengths and areas, in drawing units. Finer than `BATCH_TOL`
/// so arcs measure close to their true length.
pub(crate) const MEASURE_TOL: f64 = 1e-3;

/// The open drawing and what the user is looking at. Tauri-free, so it can be tested directly.
pub struct Session {
    pub drawing: Drawing,
    /// Block name of the sheet on screen, e.g. `*Model_Space`.
    pub sheet: String,
    pub visibility: Visibility,
    pub takeoff: Takeoff,
    /// The imported BOQ; `None` until one is imported.
    pub boq: Option<Boq>,
    /// The conflicts of the last comparison, with the user's statuses and notes.
    pub conflicts: Vec<Conflict>,
    /// The schedules read off every sheet at the last comparison.
    pub schedules: Vec<Schedule>,
    history: History,
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

/// Shown when some block content was too deep or too large to draw.
const TRUNCATED: &str = "Drawing too complex, some block content not drawn";

/// What is under the cursor: the entity itself (`entity`), the block references it sits inside
/// (`chain`, outermost first, empty at the top level) and the top-level entity on the sheet
/// (`top`: the first of `chain`, or `entity` itself).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Picked {
    pub top: u64,
    pub entity: u64,
    pub chain: Vec<u64>,
    /// Name of the xref the entity belongs to; always `None` for now.
    pub xref: Option<String>,
}

/// What the user sees about one picked entity. Length and area are in drawing units, in world
/// coordinates (an entity inside blocks is measured as placed by its block references).
#[derive(Debug, Clone, Serialize)]
pub struct EntityInfo {
    pub handle: u64,
    /// The entity type, e.g. "Line", "LwPolyline", "Insert", "Text".
    pub kind: String,
    /// The layer it is drawn on: its own, or for one on layer "0" inside a block, its insert's.
    pub layer: String,
    pub length: Option<f64>,
    /// Sum of the closed paths' areas; `None` when the entity has no closed path.
    pub area: Option<f64>,
    /// For a block reference, its block (for a dynamic block, the block it was made from).
    pub block_name: Option<String>,
    pub text: Option<String>,
    /// The block the entity sits in, when it was picked inside one.
    pub inside_block: Option<String>,
    /// The block references the entity sits inside, outermost first; empty at the top level.
    pub chain: Vec<u64>,
    /// The block names along `chain`, outermost first.
    pub block_path: Vec<String>,
}

/// An entity found by handle and chain, with what places it in the world.
pub(crate) struct Located<'a> {
    pub entity: &'a EntityType,
    /// Block-to-world transform of the chain (identity at the top level).
    pub xf: Xform,
    /// The layer the entity is drawn on (layer "0" inside a block takes its insert's).
    pub layer: String,
    /// The block names along the chain, outermost first (dynamic blocks by their own name).
    pub block_path: Vec<String>,
}

/// Finds `handle` inside the block references `chain` (outermost first; empty for an entity
/// on a sheet). Each reference must sit in the block of the one before it, and the entity in
/// the last one's block. An array (MINSERT) is placed by its first cell.
pub(crate) fn locate<'a>(
    doc: &'a CadDocument,
    handle: u64,
    chain: &[u64],
) -> Result<Located<'a>, CoreError> {
    let get = |h: u64| {
        doc.get_entity(Handle::new(h))
            .ok_or(CoreError::NoSuchEntity(h))
    };
    let mut xf = Xform::identity();
    let mut layer: Option<&str> = None;
    let mut block: Option<&str> = None;
    let mut block_path = Vec::new();
    for &h in chain {
        let e = get(h)?;
        let EntityType::Insert(insert) = e else {
            return Err(CoreError::InvalidCommand(format!(
                "Entity {h} in the chain is not a block reference"
            )));
        };
        check_inside(doc, h, block)?;
        let placed = insert_instances(doc, insert, &xf).next();
        xf = placed.ok_or_else(|| {
            CoreError::InvalidCommand(format!("Block {} is missing", insert.block_name))
        })?;
        layer = Some(drawn_layer(&insert.common.layer, layer));
        block = Some(&insert.block_name);
        block_path.push(effective_block_name(doc, insert));
    }
    let entity = get(handle)?;
    check_inside(doc, handle, block)?;
    Ok(Located {
        entity,
        xf,
        layer: drawn_layer(&entity.common().layer, layer).to_string(),
        block_path,
    })
}

/// An entity's layer as drawn: layer "0" inside a block takes the insert's.
fn drawn_layer<'a>(own: &'a str, insert: Option<&'a str>) -> &'a str {
    match insert {
        Some(outer) if own == "0" => outer,
        _ => own,
    }
}

/// Ok when `block` is `None` (no chain yet) or holds the entity `handle`.
fn check_inside(doc: &CadDocument, handle: u64, block: Option<&str>) -> Result<(), CoreError> {
    let Some(block) = block else {
        return Ok(());
    };
    let inside = doc
        .block_records
        .get(block)
        .is_some_and(|b| b.entity_handles.contains(&Handle::new(handle)));
    if inside {
        Ok(())
    } else {
        Err(CoreError::InvalidCommand(format!(
            "Entity {handle} is not inside block {block}"
        )))
    }
}

/// The block an insert shows. An anonymous dynamic-block copy (`*U…`) is named after the
/// dynamic block it was made from, when the drawing links them; otherwise the insert's own name.
pub(crate) fn effective_block_name(doc: &CadDocument, insert: &Insert) -> String {
    let anonymous = insert
        .block_name
        .get(..2)
        .is_some_and(|p| p.eq_ignore_ascii_case("*U"));
    if anonymous && !doc.block_representations.is_empty() {
        if let Some(definition) = doc.dynamic_definition_for_insert(insert.common.handle) {
            if let Some(record) = doc.block_records.iter().find(|b| b.handle == definition) {
                return record.name.clone();
            }
        }
    }
    insert.block_name.clone()
}

impl Session {
    /// Opens a drawing on its model sheet with nothing hidden, with the take-off and BOQ from its
    /// project file when there is one, else from the measurements embedded in the drawing. A project file
    /// that can't be read is a problem, not a failure. When the project file exists it wins: the
    /// embedded measurements are ignored and rewritten on the next save.
    pub fn open(path: &Path) -> Result<Session, CoreError> {
        let mut drawing = Drawing::open(path)?;
        let sheet = drawing.sheets()[0].block.clone();
        let mut takeoff = Takeoff::default();
        let mut boq = None;
        let mut conflicts = Vec::new();
        let mut schedules = Vec::new();
        let project_file = project_path(path);
        if project_file.exists() {
            match project::load(&project_file) {
                Ok(project) => {
                    if !project
                        .drawing
                        .eq_ignore_ascii_case(&project::file_name(path))
                    {
                        drawing
                            .problems
                            .push(format!("Project file was saved for {}", project.drawing));
                    }
                    takeoff = project.takeoff;
                    boq = project.boq;
                    conflicts = project.conflicts;
                    schedules = project.schedules;
                }
                // Keep a copy, since the next save replaces the unreadable file.
                Err(e) => drawing
                    .problems
                    .push(match project::back_up(&project_file) {
                        Ok(bak) => {
                            format!("Project file could not be read (backed up to {bak}): {e}")
                        }
                        Err(_) => format!("Project file could not be read: {e}"),
                    }),
            }
        } else {
            takeoff.items = read_embedded(&drawing.doc);
        }
        Ok(Session {
            drawing,
            sheet,
            visibility: Visibility::default(),
            takeoff,
            boq,
            conflicts,
            schedules,
            history: History::default(),
            cache: RefCell::new(None),
        })
    }

    /// Saves the take-off, the BOQ, the conflicts and the schedules to the drawing's project file
    /// and returns its path.
    pub fn save_project(&self) -> Result<PathBuf, CoreError> {
        let path = project_path(&self.drawing.path);
        let project = Project {
            version: 2,
            drawing: project::file_name(&self.drawing.path),
            takeoff: self.takeoff.clone(),
            boq: self.boq.clone(),
            conflicts: self.conflicts.clone(),
            schedules: self.schedules.clone(),
        };
        project::save(&path, &project)?;
        Ok(path)
    }

    /// Saves the drawing over its own file; see `save_as`.
    pub fn save(&mut self) -> Result<(), CoreError> {
        let path = self.drawing.path.clone();
        self.save_as(&path)
    }

    /// Writes the drawing to `path` (DWG or DXF by extension, in the drawing's own version) with
    /// the take-off embedded on the `QIYAS-*` layers, then saves the project file next to it.
    /// A file already at `path` is first copied to `<path>.bak`. From then on the session's
    /// drawing is the one at `path`; after a failed write it is unchanged.
    pub fn save_as(&mut self, path: &Path) -> Result<(), CoreError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let dwg = match ext.as_deref() {
            Some("dwg") => true,
            Some("dxf") => false,
            _ => {
                return Err(CoreError::InvalidCommand(
                    "Save as .dwg or .dxf".to_string(),
                ))
            }
        };
        // Embedded into a copy, so a failed write leaves the drawing in memory as it was.
        let mut doc = self.drawing.doc.clone();
        embed_measurements(&mut doc, &self.takeoff.items);
        let bytes = if dwg {
            DwgWriter::write_to_vec(&doc)
        } else {
            DxfWriter::new(&doc).write_to_vec()
        }
        .map_err(|e| CoreError::Unwritable(e.to_string()))?;
        // Into a temp file first, so a failed save never damages the drawing already there.
        project::write_with_backup(path, &bytes)?;
        self.drawing.doc = doc;
        // The embedded entities are part of the drawing now: draw and pick them too.
        *self.cache.get_mut() = None;
        self.drawing.path = path.to_path_buf();
        self.save_project().map_err(|e| {
            CoreError::Unwritable(format!(
                "Drawing saved, but the project file could not be written: {e}"
            ))
        })?;
        Ok(())
    }

    /// The drawing's sheets, layers and units, and what went wrong reading it or drawing the
    /// current sheet (builds the sheet's batch when it is not cached yet).
    pub fn info(&self) -> DrawingInfo {
        let mut problems = self.drawing.problems.clone();
        if self.with_cache(|c| c.batch.truncated) {
            problems.push(TRUNCATED.to_string());
        }
        DrawingInfo {
            path: self.drawing.path.display().to_string(),
            sheets: self.drawing.sheets(),
            layers: self.drawing.layers(),
            units: units_name(self.drawing.units).to_string(),
            problems,
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

    /// The entity nearest to `p` (drawing coordinates) within `tol`, among what is drawn, down
    /// to the entity inside any blocks.
    pub fn pick(&self, p: [f64; 2], tol: f64) -> Option<Picked> {
        self.with_cache(|cached| {
            let range = cached.batch.range_of(cached.index.pick(p, tol)?)?;
            Some(Picked {
                top: range.top,
                entity: range.entity,
                chain: cached.batch.chain_of(range).to_vec(),
                xref: None,
            })
        })
    }

    /// The entity `handle` inside the block references `chain` (outermost first; empty for an
    /// entity on a sheet), measured where those references place it.
    pub fn entity_info(&self, handle: u64, chain: &[u64]) -> Result<EntityInfo, CoreError> {
        let located = locate(&self.drawing.doc, handle, chain)?;
        let entity = located.entity;
        let Extent { length, area, .. } = self.extent(entity, &located.xf);
        let (block_name, text) = match entity {
            EntityType::Insert(insert) => {
                (Some(effective_block_name(&self.drawing.doc, insert)), None)
            }
            EntityType::Text(t) => (None, Some(t.value.clone())),
            EntityType::MText(t) => (None, Some(t.value.clone())),
            _ => (None, None),
        };
        Ok(EntityInfo {
            handle,
            kind: variant_name(entity),
            layer: located.layer,
            length,
            area,
            block_name,
            text,
            inside_block: located.block_path.last().cloned(),
            chain: chain.to_vec(),
            block_path: located.block_path,
        })
    }

    /// Runs `cmd` for the user; see `run_as`.
    pub fn run(&mut self, cmd: Command) -> Result<Outcome, CoreError> {
        self.run_as(cmd, Source::User)
    }

    /// Runs `cmd` for `source` and, when it changes the take-off, the BOQ or the conflicts, records
    /// an undo step.
    /// New measurements are marked as made by `source`.
    pub fn run_as(&mut self, cmd: Command, source: Source) -> Result<Outcome, CoreError> {
        let before = cmd.changes().map(|part| match part {
            Part::Takeoff => Snapshot::Takeoff(self.takeoff.clone()),
            Part::Boq => Snapshot::Boq(self.boq.clone()),
            Part::Conflicts => Snapshot::Conflicts(self.conflicts.clone(), self.schedules.clone()),
        });
        let outcome = self.execute(cmd, source)?;
        if let Some(before) = before {
            if !matches!(outcome, Outcome::Nothing) {
                self.history.record(before, source);
            }
        }
        Ok(outcome)
    }

    /// Undoes the last take-off, BOQ or conflicts change. False when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.history.undo(Parts {
            takeoff: &mut self.takeoff,
            boq: &mut self.boq,
            conflicts: &mut self.conflicts,
            schedules: &mut self.schedules,
        })
    }

    /// Undoes the last take-off or BOQ change only when `source` made it. False otherwise, with nothing
    /// changed: the AI's Undo never takes back a change the user made after it.
    pub fn undo_if(&mut self, source: Source) -> bool {
        self.history.top_source() == Some(source) && self.undo()
    }

    /// Who made the change `undo` would take back; None when there is nothing to undo.
    pub fn top_step_source(&self) -> Option<Source> {
        self.history.top_source()
    }

    /// Redoes the last undone take-off, BOQ or conflicts change. False when there is nothing to
    /// redo.
    pub fn redo(&mut self) -> bool {
        self.history.redo(Parts {
            takeoff: &mut self.takeoff,
            boq: &mut self.boq,
            conflicts: &mut self.conflicts,
            schedules: &mut self.schedules,
        })
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Length and area of `entity` placed by `xf`: exact for lines, arcs, circles, full
    /// ellipses and (bulged) polylines under a uniform scale; otherwise the length of all its
    /// flattened paths and the area of the closed ones.
    pub(crate) fn extent(&self, entity: &EntityType, xf: &Xform) -> Extent {
        let paths = flatten(&self.drawing.doc, entity, xf, MEASURE_TOL).into_paths();
        let closed: Vec<_> = paths.iter().filter(|p| p.closed).collect();
        let self_intersecting = closed.iter().any(|p| p.self_intersects());
        let length = (!paths.is_empty()).then(|| {
            exact_length(entity, xf).unwrap_or_else(|| paths.iter().map(|p| p.length()).sum())
        });
        // A crossing outline's lobes would cancel in the exact sum; the flattened one adds them.
        let exact = exact_area(entity, xf).filter(|_| !self_intersecting);
        let area = (!closed.is_empty())
            .then(|| exact.unwrap_or_else(|| closed.iter().map(|p| p.area()).sum()));
        Extent {
            length,
            area,
            self_intersecting,
        }
    }

    /// False when the user has hidden the layer or the drawing has it off or frozen.
    pub(crate) fn layer_visible(&self, layer: &str) -> bool {
        let key = normalize_name(layer);
        let hidden = self
            .visibility
            .hidden_layers
            .iter()
            .any(|n| normalize_name(n) == key);
        !hidden && self.layer_on(layer)
    }

    /// False when the drawing has the layer off or frozen, whatever the user has hidden.
    pub(crate) fn layer_on(&self, layer: &str) -> bool {
        !self
            .drawing
            .doc
            .layers
            .get(layer)
            .is_some_and(|l| l.flags.off || l.flags.frozen)
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

/// What one entity measures. `None` when it has no path, or no closed path for `area`.
pub(crate) struct Extent {
    pub length: Option<f64>,
    pub area: Option<f64>,
    /// True when any closed path crosses itself.
    pub self_intersecting: bool,
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
pub(crate) fn variant_name(entity: &EntityType) -> String {
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

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use opencadcodec::entities::EntityCommon;
use opencadcodec::entities::Insert;
use opencadcodec::tables::normalize_name;
use opencadcodec::{CadDocument, Color, DwgWriter, DxfWriter, EntityType, Handle};
use opencadkernel::geom2d::Curve;
use qiyas_geom::{
    drawn_style, exact_area, exact_length, flatten_with, insert_instances, is_xref_block,
    snap_shapes, Xform, XrefDocs, MAX_XREF_DEPTH,
};
use qiyas_render::{
    add_images, add_pick_outlines, build_batch_with_xrefs, layer_shown, place_text, resolve_color,
    Batch, PickIndex, TextEngine, TextOptions, Visibility, PATTERN, TEXT_BOX,
};
use serde::{Deserialize, Serialize};

use crate::commands::Part;
use crate::drawing::short_layer_name;
use crate::embed::{embed_measurements, read_embedded};
use crate::project::{self, project_path, Project};
use crate::underlay::{rebase_links, Underlays};
use crate::undo::{DocChange, History, Parts, Step, TablesSnapshot};
use crate::xref::{XrefInfo, XrefSet};
use crate::{
    Boq, Command, Conflict, CoreError, Current, Drafting, Drawing, LayerInfo, Outcome, Schedule,
    Sheet, Source, Takeoff, Units,
};

/// Flatten tolerance for drawing on screen, in drawing units.
pub const BATCH_TOL: f64 = 0.05;
/// What `Session::segments_near` finds.
pub(crate) struct Near {
    pub segments: Vec<[f64; 4]>,
    /// The square held the whole sheet: there is nothing further out.
    pub whole_sheet: bool,
}

/// Flatten tolerance for measured lengths and areas, in drawing units. Finer than `BATCH_TOL`
/// so arcs measure close to their true length.
pub(crate) const MEASURE_TOL: f64 = 1e-3;

/// Gives every opened session its own number, so the batch version of one drawing never equals
/// that of another opened later in the same window.
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

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
    /// The drawing holds its Arabic text backwards (typed in visual order for old CAD fonts):
    /// draw every Arabic run reversed. Saved in the project file.
    pub arabic_reversed: bool,
    /// Snaps, ortho, polar, grid and display precision. Saved in the project file.
    pub drafting: Drafting,
    /// The drawing's xrefs, opened read-only.
    pub xrefs: XrefSet,
    /// The file the user chose for an xref, by xref name; looked at first. Saved in the project
    /// file.
    pub xref_overrides: HashMap<String, String>,
    /// The layer, colour, linetype and lineweight new entities take. Saved in the project file;
    /// not an undo step.
    pub current: Current,
    history: History,
    /// Which session this is; see `NEXT_EPOCH`.
    epoch: u64,
    /// Counts the changes to the drawing itself (not the take-off), which is all a batch shows.
    document_revision: u64,
    /// `document_revision` at the last save (or the open): the drawing has unsaved changes
    /// when they differ.
    saved_revision: u64,
    /// Model space holds nothing (`EMPTY_MODEL` is then among the problems); looked at again on
    /// every drawing change.
    empty_model: bool,
    /// Batch and pick index of the last sheet and visibility drawn; rebuilt when either changes.
    cache: RefCell<Option<Cached>>,
    /// Fonts, glyph atlas and glyph cache, kept across batches so the atlas pages the window
    /// holds stay valid.
    text: RefCell<TextEngine>,
    /// The pictures' (PDF underlays and raster images) image ids, PDF page sizes and problems.
    pub(crate) underlays: RefCell<Underlays>,
}

struct Cached {
    sheet: String,
    hidden_layers: HashSet<String>,
    arabic_reversed: bool,
    batch: Batch,
    index: PickIndex,
    /// The point entities on the sheet (top level, on shown layers), for node snaps: where, and
    /// their handle. They are not drawn, so the pick index can't find them.
    points: Vec<PointAt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DrawingInfo {
    pub path: String,
    pub sheets: Vec<Sheet>,
    pub layers: Vec<LayerInfo>,
    /// "inches", "feet", "millimetres", "centimetres", "metres" or "unknown".
    pub units: String,
    pub problems: Vec<String>,
    /// See `Session::arabic_reversed`.
    pub arabic_reversed: bool,
    /// The drawing's xrefs (and those inside them), in load order.
    pub xrefs: Vec<XrefInfo>,
    /// Block name of the sheet on screen.
    pub sheet: String,
    /// The layers the user has hidden, sorted.
    pub hidden_layers: Vec<String>,
    /// What new entities are drawn with (`Session::current`).
    pub current: Current,
}

/// An entity by its handle and the block references it sits inside (outermost first).
pub(crate) type EntityAt = (u64, Vec<u64>);
/// A point entity: where it is, and its handle.
pub(crate) type PointAt = ([f64; 2], u64);

/// In the problems of a drawing that opens with nothing in model space.
pub const EMPTY_MODEL: &str = "This drawing has no geometry in Model";

/// Shown when some block content was too deep or too large to draw.
const TRUNCATED: &str = "Drawing too complex, some block content not drawn";

/// What is under the cursor: the entity itself (`entity`), the block references it sits inside
/// (`chain`, outermost first, empty at the top level) and the top-level entity on the sheet
/// (`top`: the first of `chain`, or `entity` itself).
///
/// Inside an xref, `chain` ends with the xref's reference (it lists the host's references only),
/// `entity` is the entity at the top of the xref's model space (a handle of the xref drawing; a
/// block reference or a nested xref there is picked whole) and `xref` names the xref.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Picked {
    pub top: u64,
    pub entity: u64,
    pub chain: Vec<u64>,
    /// Name of the xref the entity belongs to (the outermost, for a nested xref).
    #[serde(default)]
    pub xref: Option<String>,
    /// The kind of the entity at the top (`top`), e.g. "Line", "Insert", "Dimension".
    #[serde(default)]
    pub kind: Option<String>,
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
    /// `layer` as shown to the QS: a bound xref's layer without its `XREF$0$` (see
    /// `short_layer_name`).
    pub layer_short: String,
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
    /// The xref the entity belongs to; its `handle` is then one of that drawing's.
    pub xref: Option<String>,
    /// Its own colour: "ByLayer", "ByBlock", "None", "ACI n" or "#rrggbb".
    pub color: String,
    /// The colour it is drawn with: ByLayer takes its layer's, ByBlock its block reference's;
    /// white when nothing gives one.
    pub color_rgb: [u8; 3],
    /// Its linetype name, or "ByLayer".
    pub linetype: String,
    /// Its lineweight in hundredths of a millimetre; -1 ByLayer, -2 ByBlock, -3 Default.
    pub lineweight: i16,
    /// A circle's or an arc's radius in drawing units, as placed in the world.
    pub radius: Option<f64>,
    /// A block reference's attributes, as (tag, value).
    pub attributes: Vec<(String, String)>,
    /// A text's height, in drawing units.
    pub height: Option<f64>,
    /// A text's style name.
    pub style: Option<String>,
}

/// An entity found by handle and chain, with what places it in the world.
pub(crate) struct Located<'a> {
    pub entity: &'a EntityType,
    /// The drawing the entity is in: the host, or the xref the chain ends in.
    pub doc: &'a CadDocument,
    /// The xref the entity belongs to, when the chain ends in an xref reference.
    pub xref: Option<String>,
    /// Block-to-world transform of the chain (identity at the top level).
    pub xf: Xform,
    /// The layer the entity is drawn on (layer "0" inside a block takes its insert's).
    pub layer: String,
    /// The block names along the chain, outermost first (dynamic blocks by their own name).
    pub block_path: Vec<String>,
}

/// Finds `handle` inside the block references `chain` (outermost first; empty for an entity
/// on a sheet). Each reference must sit in the block of the one before it, and the entity in
/// the last one's block. An array (MINSERT) is placed by its first cell. A chain may end with a
/// reference to an xref in `xrefs`: the entity is then one at the top of that xref's model space,
/// on layer `XREF|LAYER` (layer "0" takes the reference's).
pub(crate) fn locate<'a>(
    doc: &'a CadDocument,
    xrefs: &XrefDocs<'a>,
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
    let mut xref: Option<(&'a str, &'a CadDocument)> = None;
    for &h in chain {
        if xref.is_some() {
            return Err(CoreError::InvalidCommand(format!(
                "Entity {h} is inside an xref: an xref reference ends the chain"
            )));
        }
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
        if is_xref_block(doc, &insert.block_name) {
            let xdoc = xrefs
                .get(&normalize_name(&insert.block_name))
                .ok_or_else(|| {
                    CoreError::InvalidCommand(format!("Xref {} is not loaded", insert.block_name))
                })?;
            xref = Some((&insert.block_name, xdoc));
        }
    }
    let Some((name, xdoc)) = xref else {
        let entity = get(handle)?;
        check_inside(doc, handle, block)?;
        return Ok(Located {
            entity,
            doc,
            xref: None,
            xf,
            layer: drawn_layer(&entity.common().layer, layer).to_string(),
            block_path,
        });
    };
    let entity = xdoc
        .get_entity(Handle::new(handle))
        .ok_or(CoreError::NoSuchEntity(handle))?;
    check_inside(xdoc, handle, Some("*Model_Space"))?;
    let own = &entity.common().layer;
    let layer = match layer {
        Some(outer) if own == "0" => outer.to_string(),
        _ => format!("{name}|{own}"),
    };
    Ok(Located {
        entity,
        doc: xdoc,
        xref: Some(name.to_string()),
        xf,
        layer,
        block_path,
    })
}

/// The xref an entity picked with `chain` belongs to: the block of the chain's last reference,
/// when that is an xref.
pub(crate) fn xref_of_chain(doc: &CadDocument, chain: &[u64]) -> Option<String> {
    match doc.get_entity(Handle::new(*chain.last()?))? {
        EntityType::Insert(insert) if is_xref_block(doc, &insert.block_name) => {
            Some(insert.block_name.clone())
        }
        _ => None,
    }
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

/// An entity's own colour as the properties palette shows it.
fn color_name(color: &Color) -> String {
    match color {
        Color::ByLayer => "ByLayer".into(),
        Color::ByBlock => "ByBlock".into(),
        Color::None => "None".into(),
        Color::Index(i) => format!("ACI {i}"),
        Color::Rgb { r, g, b } => format!("#{r:02x}{g:02x}{b:02x}"),
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
    /// embedded measurements are ignored and rewritten on the next save. Then opens its xrefs
    /// (with the files the project chose for them); a missing xref is a problem, not a failure.
    /// Measurements embedded inside xrefs are never read.
    pub fn open(path: &Path) -> Result<Session, CoreError> {
        let mut drawing = Drawing::open(path)?;
        let sheet = drawing.sheets()[0].block.clone();
        let mut takeoff = Takeoff::default();
        let mut boq = None;
        let mut conflicts = Vec::new();
        let mut schedules = Vec::new();
        let mut arabic_reversed = false;
        let mut drafting = Drafting::default();
        let mut xref_overrides = HashMap::new();
        let mut current = Current::default();
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
                    arabic_reversed = project.arabic_reversed;
                    drafting = project.drafting.clamped();
                    xref_overrides = project.xref_overrides;
                    // A layer gone from the drawing since: back to layer "0".
                    if drawing.doc.layers.contains(&project.current.layer) {
                        current = project.current;
                    } else {
                        current = Current {
                            layer: "0".to_string(),
                            ..project.current
                        };
                    }
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
        let xrefs = XrefSet::load(&drawing, &xref_overrides, MAX_XREF_DEPTH);
        let empty_model = model_is_empty(&drawing);
        Ok(Session {
            drawing,
            sheet,
            visibility: Visibility::default(),
            takeoff,
            boq,
            conflicts,
            schedules,
            arabic_reversed,
            drafting,
            xrefs,
            xref_overrides,
            current,
            history: History::default(),
            epoch: NEXT_EPOCH.fetch_add(1, Ordering::Relaxed),
            document_revision: 0,
            saved_revision: 0,
            empty_model,
            cache: RefCell::new(None),
            text: RefCell::new(TextEngine::new()),
            underlays: RefCell::new(Underlays::default()),
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
            arabic_reversed: self.arabic_reversed,
            drafting: self.drafting.clone(),
            xref_overrides: self.xref_overrides.clone(),
            current: self.current.clone(),
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
    /// Only the host drawing is written: xref files are never touched, and a measurement of an
    /// xref's entity is embedded from its points only.
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
        // PDF underlay and image links stay right from the new folder.
        rebase_links(&mut doc, &self.drawing.path, path);
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
        self.drawing_changed();
        self.saved_revision = self.document_revision;
        self.drawing.path = path.to_path_buf();
        self.save_project().map_err(|e| {
            CoreError::Unwritable(format!(
                "Drawing saved, but the project file could not be written: {e}"
            ))
        })?;
        Ok(())
    }

    /// The drawing's sheets, layers and units, the sheet on screen and the layers hidden, and what went wrong reading it or drawing the
    /// current sheet (builds the sheet's batch when it is not cached yet).
    pub fn info(&self) -> DrawingInfo {
        let mut problems = self.drawing.problems.clone();
        if self.empty_model {
            problems.push(EMPTY_MODEL.to_string());
        }
        problems.extend(self.xrefs.problems().cloned());
        if self.with_cache(None, |c| c.batch.truncated) {
            problems.push(TRUNCATED.to_string());
        }
        problems.extend(self.underlays.borrow().problems.iter().cloned());
        DrawingInfo {
            path: self.drawing.path.display().to_string(),
            sheets: self.drawing.sheets(),
            layers: self.layers(),
            units: units_name(self.drawing.units).to_string(),
            problems,
            arabic_reversed: self.arabic_reversed,
            xrefs: self.xrefs.info(&self.xref_overrides),
            sheet: self.sheet.clone(),
            hidden_layers: self.hidden_layers(),
            current: self.current.clone(),
        }
    }

    /// The drawing's layers and those its xrefs add (`XREF|LAYER`), sorted by name. An xref
    /// layer the drawing keeps a copy of itself is listed once, as the drawing has it.
    pub fn layers(&self) -> Vec<LayerInfo> {
        let mut layers = self.drawing.layers();
        let own: HashSet<String> = layers.iter().map(|l| normalize_name(&l.name)).collect();
        layers.extend(
            self.xrefs
                .layers()
                .into_iter()
                .filter(|l| !own.contains(&normalize_name(&l.name))),
        );
        layers.sort_by(|a, b| a.name.cmp(&b.name));
        layers
    }

    /// The opened xref drawings by upper-case name, for drawing, measuring and searching.
    pub(crate) fn xref_docs(&self) -> XrefDocs<'_> {
        self.xrefs.docs()
    }

    /// Looks for every xref again and reopens it (a changed or newly found file shows).
    pub fn reload_xrefs(&mut self) {
        self.xrefs = XrefSet::load(&self.drawing, &self.xref_overrides, MAX_XREF_DEPTH);
        *self.cache.get_mut() = None;
    }

    /// Uses `path` for the xref `name` from now on (`None` goes back to looking for it), then
    /// reloads the xrefs. Saved in the project file. Fails when the drawing has no such xref.
    pub fn set_xref_path(&mut self, name: &str, path: Option<&Path>) -> Result<(), CoreError> {
        let Some(xref) = self.xrefs.get(name) else {
            return Err(CoreError::InvalidCommand(format!("No xref named {name}")));
        };
        let key = normalize_name(&xref.name);
        let name = xref.name.clone();
        self.xref_overrides.retain(|n, _| normalize_name(n) != key);
        if let Some(path) = path {
            self.xref_overrides.insert(name, path.display().to_string());
        }
        self.reload_xrefs();
        Ok(())
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

    /// Line segments and text glyphs of the current sheet with the current visibility.
    pub fn batch(&self) -> Batch {
        self.with_cache(None, |cached| cached.batch.clone())
    }

    /// Names what `batch` would give: it changes when the sheet, the hidden layers, the Arabic
    /// setting, the drawing itself (every drawing command, its undo and its redo) or its xrefs
    /// change, and not for take-off, BOQ or conflict changes or their undo or redo. Not for the zoom the glyphs are drawn for either: the window asks
    /// for that. A 64-bit FNV-1a hash of the session, sheet, hidden layers (sorted, case-folded),
    /// Arabic setting, document revision and xrefs (name, file found, when it was last changed
    /// and its size, each), cut to 53 bits so it survives a trip through a JavaScript number.
    pub fn batch_version(&self) -> u64 {
        let mut hash = Fnv::new();
        hash.write(&self.epoch.to_le_bytes());
        hash.write(self.sheet.as_bytes());
        hash.write(&[0xFF]);
        let mut hidden: Vec<String> = self
            .visibility
            .hidden_layers
            .iter()
            .map(|n| normalize_name(n))
            .collect();
        hidden.sort();
        for name in hidden {
            hash.write(name.as_bytes());
            hash.write(&[0xFF]);
        }
        hash.write(&[u8::from(self.arabic_reversed)]);
        hash.write(&self.document_revision.to_le_bytes());
        for x in &self.xrefs.entries {
            hash.write(x.name.as_bytes());
            hash.write(&[0xFF]);
            if let Some(found) = &x.found {
                hash.write(found.as_os_str().as_encoded_bytes());
            }
            hash.write(&[0xFF, u8::from(x.drawing.is_some())]);
            let modified = x
                .modified
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            hash.write(&modified.to_le_bytes());
            hash.write(&x.size.unwrap_or(u64::MAX).to_le_bytes());
        }
        hash.0 & ((1 << 53) - 1)
    }

    /// The batch as `Batch::to_bytes` lays it out, or `None` when `if_version` is the current
    /// `batch_version`, so the caller keeps the batch it already holds. `px_per_unit` is the
    /// zoom to draw the glyphs for (screen pixels per drawing unit); `None` keeps the zoom of the
    /// batch in hand, or fits the sheet on a new one.
    pub fn batch_bytes_if(
        &self,
        if_version: Option<u64>,
        px_per_unit: Option<f64>,
    ) -> Option<Vec<u8>> {
        if if_version == Some(self.batch_version()) {
            return None;
        }
        Some(self.with_cache(px_per_unit, |cached| cached.batch.to_bytes()))
    }

    /// The glyph atlas pages changed after version `since` (all when `None`), as
    /// `Atlas::to_bytes` lays them out.
    pub fn atlas_bytes(&self, since: Option<u64>) -> Vec<u8> {
        self.text.borrow().atlas_bytes(since)
    }

    /// The entity nearest to `p` (drawing coordinates) within `tol`, among what is drawn, down
    /// to the entity inside any blocks.
    pub fn pick(&self, p: [f64; 2], tol: f64) -> Option<Picked> {
        self.with_cache(None, |cached| {
            let range = cached.batch.range_of(cached.index.pick(p, tol)?)?;
            let chain = cached.batch.chain_of(range).to_vec();
            Some(Picked {
                top: range.top,
                entity: range.entity,
                xref: xref_of_chain(&self.drawing.doc, &chain),
                chain,
                kind: self.kind_of(range.top),
            })
        })
    }

    /// The entities a selection box picks on the sheet on screen, among what is drawn: with
    /// `crossing`, each with a drawn segment inside or crossing `bounds` (`[[min_x, min_y],
    /// [max_x, max_y]]`, drawing coordinates); otherwise each drawn wholly inside it. Always the
    /// entity at the top of the sheet: a block reference, or an xref's reference, is picked whole
    /// (never what is inside it), so `entity == top` and `chain` is empty. Sorted by handle.
    pub fn pick_window(&self, bounds: [[f64; 2]; 2], crossing: bool) -> Vec<Picked> {
        self.with_cache(None, |cached| {
            let batch = &cached.batch;
            let [ox, oy] = batch.origin;
            // In the batch's own coordinates, as its segments are.
            let lo = [bounds[0][0] - ox, bounds[0][1] - oy];
            let hi = [bounds[1][0] - ox, bounds[1][1] - oy];
            let segment = |i: usize| -> [f64; 4] {
                let s = &batch.segments[4 * i..4 * i + 4];
                [s[0], s[1], s[2], s[3]].map(f64::from)
            };
            let near = cached
                .index
                .within(bounds[0], bounds[1], usize::MAX, |_| true);
            let mut tops = std::collections::BTreeSet::new();
            if crossing {
                for i in near {
                    if crosses_box(segment(i), lo, hi) {
                        tops.extend(batch.range_of(i).map(|r| r.top));
                    }
                }
            } else {
                let inside =
                    |p: [f64; 2]| lo[0] <= p[0] && p[0] <= hi[0] && lo[1] <= p[1] && p[1] <= hi[1];
                let reached: HashSet<u64> = near
                    .into_iter()
                    .filter_map(|i| batch.range_of(i))
                    .map(|r| r.top)
                    .collect();
                let mut outside = HashSet::new();
                for r in batch.ranges.iter().filter(|r| reached.contains(&r.top)) {
                    let out = (r.first..r.first + r.count).any(|i| {
                        let [x0, y0, x1, y1] = segment(i as usize);
                        !inside([x0, y0]) || !inside([x1, y1])
                    });
                    if out {
                        outside.insert(r.top);
                    }
                }
                tops.extend(reached.difference(&outside));
            }
            tops.into_iter()
                .map(|top| Picked {
                    top,
                    entity: top,
                    chain: Vec::new(),
                    xref: None,
                    kind: self.kind_of(top),
                })
                .collect()
        })
    }

    /// The kind of the drawing's entity `handle` ("Line", "Insert"…).
    fn kind_of(&self, handle: u64) -> Option<String> {
        self.drawing
            .doc
            .get_entity(Handle::new(handle))
            .map(variant_name)
    }

    /// Up to `max_entities` drawn entities with a segment within `tol` of `p`, nearest first (by
    /// handle and block-reference chain; text boxes left out), looking at no more than
    /// `max_segments` segments; and the sheet's point entities within `tol`.
    pub(crate) fn near_entities(
        &self,
        p: [f64; 2],
        tol: f64,
        max_entities: usize,
        max_segments: usize,
    ) -> (Vec<EntityAt>, Vec<PointAt>) {
        self.with_cache(None, |cached| {
            let mut entities: Vec<EntityAt> = Vec::new();
            for i in cached.index.near(p, tol, max_segments) {
                let Some(range) = cached.batch.range_of(i) else {
                    continue;
                };
                if range.flags & TEXT_BOX != 0 {
                    continue;
                }
                let chain = cached.batch.chain_of(range);
                if entities
                    .iter()
                    .any(|(h, c)| *h == range.entity && c == chain)
                {
                    continue;
                }
                entities.push((range.entity, chain.to_vec()));
                if entities.len() == max_entities {
                    break;
                }
            }
            let points = cached
                .points
                .iter()
                .filter(|(q, _)| (q[0] - p[0]).hypot(q[1] - p[1]) <= tol)
                .copied()
                .collect();
            (entities, points)
        })
    }

    /// The drawn segments (absolute `x0 y0 x1 y1`) of the sheet on screen that reach into the
    /// square `radius` around `p`, text boxes and hatch pattern lines left out; `None` when more
    /// than `max` of them do.
    pub(crate) fn segments_near(&self, p: [f64; 2], radius: f64, max: usize) -> Option<Near> {
        self.with_cache(None, |cached| {
            let batch = &cached.batch;
            let [ox, oy] = batch.origin;
            let lo = [p[0] - radius, p[1] - radius];
            let hi = [p[0] + radius, p[1] + radius];
            // Counted after text boxes and pattern lines go: a densely hatched room nearby
            // must not refuse a plain one. The tree walk stops at the cap all the same.
            let found = cached.index.within(lo, hi, max + 1, |i| {
                batch
                    .range_of(i)
                    .is_some_and(|r| r.flags & (TEXT_BOX | PATTERN) == 0)
            });
            if found.len() > max {
                return None;
            }
            let [b0, b1] = batch.bounds;
            let whole_sheet = lo[0] <= b0[0] && lo[1] <= b0[1] && hi[0] >= b1[0] && hi[1] >= b1[1];
            let segments = found
                .into_iter()
                .map(|i| {
                    let s = &batch.segments[4 * i..4 * i + 4];
                    [
                        f64::from(s[0]) + ox,
                        f64::from(s[1]) + oy,
                        f64::from(s[2]) + ox,
                        f64::from(s[3]) + oy,
                    ]
                })
                .collect();
            Some(Near {
                segments,
                whole_sheet,
            })
        })
    }

    /// The entity `handle` inside the block references `chain` (outermost first; empty for an
    /// entity on a sheet), measured where those references place it. A chain ending in an xref
    /// reference names an entity of that xref.
    pub fn entity_info(&self, handle: u64, chain: &[u64]) -> Result<EntityInfo, CoreError> {
        let xrefs = self.xref_docs();
        let located = locate(&self.drawing.doc, &xrefs, handle, chain)?;
        let entity = located.entity;
        let Extent { length, area, .. } = self.extent(located.doc, entity, &located.xf);
        // A block reference is counted: the sum of its lines would only confuse.
        let (length, area) = match entity {
            EntityType::Insert(_) => (None, None),
            _ => (length, area),
        };
        let (block_name, text) = match entity {
            EntityType::Insert(insert) => (Some(effective_block_name(located.doc, insert)), None),
            EntityType::Text(t) => (None, Some(t.value.clone())),
            EntityType::MText(t) => (None, Some(t.value.clone())),
            _ => (None, None),
        };
        let (height, style) = match entity {
            EntityType::Text(t) => (Some(t.height), Some(t.style.clone())),
            EntityType::MText(t) => (Some(t.height), Some(t.style.clone())),
            _ => (None, None),
        };
        let attributes = match entity {
            EntityType::Insert(insert) => insert
                .attributes
                .iter()
                .map(|a| (a.tag.clone(), a.value.clone()))
                .collect(),
            _ => Vec::new(),
        };
        let radius = match entity {
            EntityType::Circle(_) | EntityType::Arc(_) => {
                snap_shapes(located.doc, entity, &located.xf, MEASURE_TOL)
                    .first()
                    .and_then(|shape| match shape.pieces.first() {
                        Some(Curve::Circle(c)) => Some(c.radius),
                        Some(Curve::Arc(a)) => Some(a.radius),
                        _ => None,
                    })
            }
            _ => None,
        };
        let common = entity.common();
        let linetype = if common.linetype.is_empty() {
            "ByLayer".to_string()
        } else {
            common.linetype.clone()
        };
        let color_rgb = self.drawn_color(common, chain, located.xref.as_deref());
        Ok(EntityInfo {
            handle,
            kind: variant_name(entity),
            layer_short: short_layer_name(&located.layer).to_string(),
            layer: located.layer,
            length,
            area,
            block_name,
            text,
            inside_block: located.block_path.last().cloned(),
            chain: chain.to_vec(),
            color: color_name(&common.color),
            color_rgb,
            linetype,
            lineweight: common.line_weight.value(),
            radius,
            attributes,
            height,
            style,
            block_path: located.block_path,
            xref: located.xref,
        })
    }

    /// The colour the entity `common` inside the block references `chain` is drawn with,
    /// resolved as the renderer resolves it (`drawn_style`); white when nothing gives one.
    fn drawn_color(&self, common: &EntityCommon, chain: &[u64], xref: Option<&str>) -> [u8; 3] {
        let doc = &self.drawing.doc;
        let commons: Vec<&EntityCommon> = chain
            .iter()
            .filter_map(|&h| doc.get_entity(Handle::new(h)).map(EntityType::common))
            .collect();
        let (_, color) = drawn_style(doc, &self.xref_docs(), &commons, common, xref);
        resolve_color(&color, &Color::ByLayer)
    }

    /// Runs `cmd` for the user; see `run_as`.
    pub fn run(&mut self, cmd: Command) -> Result<Outcome, CoreError> {
        self.run_as(cmd, Source::User)
    }

    /// Runs `cmd` for `source` and, when it changes the take-off, the BOQ, the conflicts or the
    /// drawing, records an undo step.
    /// New measurements are marked as made by `source`.
    pub fn run_as(&mut self, cmd: Command, source: Source) -> Result<Outcome, CoreError> {
        let before = match cmd.changes() {
            None => return self.execute(cmd, source),
            Some(Part::Document { tables, takeoff }) => {
                let (more_tables, more_takeoff) = self.document_extras(&cmd);
                return self.record_document_change(
                    source,
                    tables || more_tables,
                    takeoff || more_takeoff,
                    |s| s.execute(cmd, source),
                );
            }
            Some(Part::Takeoff) => Step::Takeoff(self.takeoff.clone()),
            Some(Part::Boq) => Step::Boq(self.boq.clone()),
            Some(Part::Conflicts) => {
                Step::Conflicts(self.conflicts.clone(), self.schedules.clone())
            }
        };
        let outcome = self.execute(cmd, source)?;
        if !matches!(outcome, Outcome::Nothing) {
            self.history.record(before, source);
        }
        Ok(outcome)
    }

    /// Undoes the last take-off, BOQ, conflicts or drawing change. False when there is nothing to
    /// undo.
    pub fn undo(&mut self) -> bool {
        let mut doc_changed = false;
        let undone = self.history.undo(Parts {
            takeoff: &mut self.takeoff,
            boq: &mut self.boq,
            conflicts: &mut self.conflicts,
            schedules: &mut self.schedules,
            doc: &mut self.drawing.doc,
            doc_changed: &mut doc_changed,
        });
        if doc_changed {
            self.drawing_changed();
        }
        undone
    }

    /// True when the drawing itself (not the take-off, which is saved after every command) has
    /// changed since it was opened or last saved: every drawing command (imports, underlays,
    /// calibrations, deletions…), and their undo and redo.
    pub fn is_dirty(&self) -> bool {
        self.document_revision != self.saved_revision
    }

    /// Runs `f`, a change to the drawing, as one undo step made by `source`. The step holds the
    /// before images of the entities `f` touches (recorded by the drawing as it goes), with the
    /// drawing's tables when `tables` and the take-off when `takeoff` (taken whole before `f`).
    /// A change that leaves everything as it was is no step. Every step moves
    /// `document_revision` on, so the window draws the batch again. On failure everything is put
    /// back as it was.
    pub(crate) fn record_document_change<T>(
        &mut self,
        source: Source,
        tables: bool,
        takeoff: bool,
        f: impl FnOnce(&mut Self) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let tables = tables.then(|| TablesSnapshot::take(&self.drawing.doc));
        let takeoff = takeoff.then(|| self.takeoff.clone());
        let recorder = self.drawing.doc.begin_entity_change_recording();
        let done = f(self);
        self.drawing.doc.end_entity_change_recording();
        let change = DocChange::from_before_images(
            &self.drawing.doc,
            recorder.take_before_images(),
            tables,
            takeoff,
        );
        match done {
            Ok(done) => {
                if !change.is_empty(&self.drawing.doc) {
                    self.drawing_changed();
                    self.history.record(Step::Document(change), source);
                }
                Ok(done)
            }
            Err(e) => {
                change.swap(&mut self.drawing.doc, &mut self.takeoff);
                *self.cache.get_mut() = None;
                Err(e)
            }
        }
    }

    /// Drops the drawn batch and pick index (for a drawing put aside in a tab, which on a big
    /// drawing hold hundreds of MB); the next draw or pick builds them again.
    pub fn park(&mut self) {
        *self.cache.get_mut() = None;
    }

    /// The batch and pick index are built and kept (see `park`).
    pub fn is_drawn(&self) -> bool {
        self.cache.borrow().is_some()
    }

    /// The drawing itself changed: the batch is drawn again and its version moves on.
    pub(crate) fn drawing_changed(&mut self) {
        self.document_revision += 1;
        self.empty_model = model_is_empty(&self.drawing);
        *self.cache.get_mut() = None;
    }

    /// Which opening of a drawing this session is: no two sessions in one run share it. What the
    /// window or the assistant fetched for one session is never used for another.
    pub fn epoch(&self) -> u64 {
        self.epoch
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

    /// Redoes the last undone take-off, BOQ, conflicts or drawing change. False when there is
    /// nothing to redo.
    pub fn redo(&mut self) -> bool {
        let mut doc_changed = false;
        let redone = self.history.redo(Parts {
            takeoff: &mut self.takeoff,
            boq: &mut self.boq,
            conflicts: &mut self.conflicts,
            schedules: &mut self.schedules,
            doc: &mut self.drawing.doc,
            doc_changed: &mut doc_changed,
        });
        if doc_changed {
            self.drawing_changed();
        }
        redone
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Length and area of `entity` of `doc` (the drawing or one of its xrefs) placed by `xf`:
    /// exact for lines, arcs, circles, full ellipses and (bulged) polylines under a uniform scale;
    /// otherwise the length of all its flattened paths and the area of the closed ones.
    pub(crate) fn extent(&self, doc: &CadDocument, entity: &EntityType, xf: &Xform) -> Extent {
        let xrefs = self.xref_docs();
        let paths = flatten_with(doc, &xrefs, entity, xf, MEASURE_TOL).into_paths();
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

    /// Runs `f` on the cached batch, rebuilding it first if the sheet, visibility or Arabic
    /// setting changed, and drawing its glyphs again for `px_per_unit` when given and new.
    fn with_cache<R>(&self, px_per_unit: Option<f64>, f: impl FnOnce(&Cached) -> R) -> R {
        let mut cache = self.cache.borrow_mut();
        let mut text = self.text.borrow_mut();
        let stale = cache.as_ref().is_none_or(|c| {
            c.sheet != self.sheet
                || c.hidden_layers != self.visibility.hidden_layers
                || c.arabic_reversed != self.arabic_reversed
        });
        if stale {
            let options = TextOptions {
                arabic_reversed: self.arabic_reversed,
                px_per_unit,
            };
            let mut batch = build_batch_with_xrefs(
                &self.drawing.doc,
                &self.xref_docs(),
                &self.sheet,
                &self.visibility,
                BATCH_TOL,
                &mut text,
                options,
            );
            let (pictures, outlines) = self.underlay_placements();
            add_images(&mut batch, &pictures);
            add_pick_outlines(&mut batch, &outlines);
            let doc = &self.drawing.doc;
            let points = doc
                .entities_in_block(&self.sheet)
                .filter_map(|e| match e {
                    EntityType::Point(pt)
                        if !pt.common.invisible
                            && layer_shown(doc, &self.visibility, &pt.common.layer) =>
                    {
                        Some(([pt.location.x, pt.location.y], pt.common.handle.value()))
                    }
                    _ => None,
                })
                .collect();
            *cache = Some(Cached {
                sheet: self.sheet.clone(),
                hidden_layers: self.visibility.hidden_layers.clone(),
                arabic_reversed: self.arabic_reversed,
                index: PickIndex::new(&batch),
                batch,
                points,
            });
        }
        let cached = cache.as_mut().expect("cache was just filled");
        if let Some(px) = px_per_unit.filter(|p| *p > 0.0 && p.is_finite()) {
            if !cached.batch.texts.is_empty() && px != cached.batch.px_per_unit {
                place_text(&mut cached.batch, &mut text, px);
            }
        }
        drop(text);
        f(cached)
    }
}

/// The segment `x0 y0 x1 y1` has a point in the box `lo hi` (Liang-Barsky clipping).
fn crosses_box([x0, y0, x1, y1]: [f64; 4], lo: [f64; 2], hi: [f64; 2]) -> bool {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, x0 - lo[0]),
        (dx, hi[0] - x0),
        (-dy, y0 - lo[1]),
        (dy, hi[1] - y0),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Model space holds no entity.
fn model_is_empty(drawing: &Drawing) -> bool {
    drawing.entities_in("*Model_Space").next().is_none()
}

/// 64-bit FNV-1a.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
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

#[cfg(test)]
mod tests {
    use opencadcodec::Line;

    use super::*;

    #[test]
    fn a_failed_drawing_change_is_put_back_and_is_no_step() {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::fixtures::write_sample(dir.path(), "dxf");
        let mut s = Session::open(&path).unwrap();
        let count = |s: &Session| s.drawing.doc.entities_in_block("*Model_Space").count();
        let (entities, version) = (count(&s), s.batch_version());
        let first = s
            .drawing
            .doc
            .entities_in_block("*Model_Space")
            .next()
            .unwrap();
        let first = first.common().handle;
        let err = s.record_document_change(Source::User, true, false, |s| {
            s.ensure_layer("NEW", Color::Index(1));
            s.drawing
                .doc
                .add_entity(EntityType::Line(Line::from_coords(
                    0.0, 0.0, 0.0, 1.0, 1.0, 0.0,
                )))
                .unwrap();
            s.drawing.doc.remove_entity_arc(first);
            Err::<(), _>(CoreError::InvalidCommand("stop".into()))
        });
        assert!(err.is_err());
        assert_eq!(count(&s), entities);
        assert!(s.drawing.doc.get_entity(first).is_some());
        assert!(!s.drawing.doc.layers.contains("NEW"));
        assert!(!s.can_undo());
        assert_eq!(s.batch_version(), version);
    }
}

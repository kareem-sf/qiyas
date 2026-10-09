//! The command bus: every change to the take-off, the BOQ, the conflicts or the drawing goes
//! through `Session::run` (the user) or `Session::run_as` (the AI, or an outside AI through MCP).

use opencadcodec::objects::ObjectType;
use opencadcodec::tables::normalize_name;
use opencadcodec::{EntityType, Handle};
use qiyas_geom::{object_to_world, Path};
use qiyas_render::layer_shown_with;
use serde::{Deserialize, Serialize};

use crate::drawing::short_layer_name;
use crate::inquiry::{Field, Spacing};
use crate::session::{effective_block_name, locate, variant_name, xref_of_chain, Located};
use crate::takeoff::in_metres;
use crate::{
    AttachMode, Boq, Conflict, ConflictStatus, CoreError, Current, Drafting, DrawSpec, EditSpec,
    EntityFilter, Kind, Measurement, ModifySpec, PreparedPdf, Schedule, Session, SnapKind, Source,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Length along `points`, in drawing coordinates. Needs 2 points or more.
    MeasureDistance {
        points: Vec<[f64; 2]>,
    },
    /// Area inside the ring `points` (closed for you). Needs 3 points or more.
    MeasureArea {
        points: Vec<[f64; 2]>,
    },
    /// Area of a closed entity, length of an open one, or 1 piece for a block reference.
    /// `chain` lists the block references the entity sits inside, outermost first (empty, the
    /// default, for an entity on the sheet); it is measured where they place it. A chain ending in
    /// an xref reference names an entity at the top of that xref (see `Picked`).
    MeasureEntity {
        handle: u64,
        #[serde(default)]
        chain: Vec<u64>,
    },
    /// Block references of `block_name` on the current sheet, on visible layers only
    /// (and only on `layer` when given), and those at the top of the sheet's xrefs.
    CountBlock {
        block_name: String,
        layer: Option<String>,
    },
    RemoveMeasurement {
        id: String,
    },
    SetDescription {
        id: String,
        text: String,
    },
    SetLayerHidden {
        name: String,
        hidden: bool,
    },
    SetSheet {
        block: String,
    },
    /// Says whether the drawing holds its Arabic text backwards (`Session::arabic_reversed`).
    /// Not an undo step.
    SetArabicReversed {
        on: bool,
    },
    /// Replaces the drafting settings (`Session::drafting`). Not an undo step.
    SetDrafting {
        drafting: Drafting,
    },
    /// A measurement of any kind, described by the caller (usually the AI). Geometry is `points`
    /// (drawing coordinates) or `handles` (measured from the entities' flattened paths, summed),
    /// not both. A length, area or count is worked out from the geometry, so `value` must be
    /// `None`; a volume is an area from the geometry times `height` (drawing units, above 0); an
    /// item needs `value` and `unit` and may have no geometry.
    AddMeasurement {
        kind: Kind,
        value: Option<f64>,
        unit: Option<String>,
        height: Option<f64>,
        points: Vec<[f64; 2]>,
        handles: Vec<u64>,
        /// The block references every handle sits inside, outermost first; empty (the default)
        /// for entities on the sheet.
        #[serde(default)]
        chain: Vec<u64>,
        description: String,
        note: String,
    },
    /// Turns an area into a volume of that area times `height` (drawing units, above 0), keeping
    /// its geometry; on a volume, changes its height.
    SetHeight {
        id: String,
        height: f64,
    },
    /// Links the measurement to the BOQ item `code`; `None` unlinks it.
    LinkBoq {
        id: String,
        code: Option<String>,
    },
    SetNote {
        id: String,
        text: String,
    },
    /// Replaces the BOQ with `boq`, usually one just read by `qiyas_boq::import_boq`.
    SetBoq {
        boq: Boq,
    },
    /// Sets the allowed difference, in percent (0 or more), of every BOQ item with `code`;
    /// `None` goes back to the user's setting.
    SetBoqTolerance {
        code: String,
        percent: Option<f64>,
    },
    /// Replaces the conflicts with those of a new comparison (`qiyas_takeoff::compare`), and the
    /// schedules with the ones it read off the sheets.
    SetConflicts {
        conflicts: Vec<Conflict>,
        schedules: Vec<Schedule>,
    },
    /// Sets the status and note of the conflict `id`.
    ResolveConflict {
        id: String,
        status: ConflictStatus,
        note: String,
    },
    /// Adds page `page` (from 1) of the PDF at `path` to the sheet on screen as lines, polylines
    /// and texts (`qiyas_pdf::extract_vectors`), `scale` drawing units per PDF point, the page's
    /// bottom-left corner at `insert_at`. One undo step; `Outcome::Added` lists the new entities.
    ImportPdf {
        path: String,
        page: u32,
        insert_at: [f64; 2],
        scale: f64,
    },
    /// Attaches page `page` of the PDF at `path` to the sheet on screen as a PDF underlay
    /// (an `Underlay` entity and its `UnderlayDefinition`, as AutoCAD stores them), `scale`
    /// drawing units per PDF point, bottom-left corner at `insert_at`. `Outcome::Added` holds the
    /// underlay's handle.
    AttachPdfUnderlay {
        path: String,
        page: u32,
        insert_at: [f64; 2],
        scale: f64,
    },
    /// Rescales the underlay or image `handle` about `p1` so that `p1` to `p2` (drawing
    /// coordinates) becomes `real_distance` drawing units long.
    Calibrate {
        handle: u64,
        p1: [f64; 2],
        p2: [f64; 2],
        real_distance: f64,
    },
    /// Attaches the raster image at `path` (png, jpg, tif, bmp, webp) to the sheet on screen as
    /// an AutoCAD image (a `RasterImage` and its `ImageDefinition`) on layer `IMAGE-UNDERLAY`,
    /// bottom-left corner at `insert_at`, `width_units` drawing units wide. `Outcome::Added` holds
    /// the image's handle.
    AttachImage {
        path: String,
        insert_at: [f64; 2],
        width_units: f64,
    },
    /// Sets the brightness, contrast and fade (0 to 100 each; 50, 50 and 0 show the image as it
    /// is) of the image `handle`.
    SetImageDisplay {
        handle: u64,
        brightness: u8,
        contrast: u8,
        fade: u8,
    },
    /// The geometry of one entity as placed in the world (radius, diameter, centre and angles of
    /// an arc, circle or ellipse; length, angle and deltas of a line; a polyline's segment nearest
    /// `at`; a block reference's insertion, scale and rotation; a text's content, height and
    /// style), in metres and degrees. `Outcome::Info`; not an undo step.
    MeasureGeometry {
        handle: u64,
        #[serde(default)]
        chain: Vec<u64>,
        #[serde(default)]
        at: Option<[f64; 2]>,
    },
    /// Divides the entity's path into `count` equal parts (2 to 1000) with point entities on
    /// layer `QIYAS-POINT`, and counts them in the take-off: one undo step.
    DivideAlong {
        handle: u64,
        #[serde(default)]
        chain: Vec<u64>,
        count: usize,
    },
    /// Places a point entity every `step` drawing units along the entity's path from its start,
    /// on layer `QIYAS-POINT`, and counts them in the take-off: one undo step.
    MeasureAlong {
        handle: u64,
        #[serde(default)]
        chain: Vec<u64>,
        step: f64,
    },
    /// Where the point `p` (drawing coordinates) is, in metres, and the snap `kind` that found it.
    /// `Outcome::Info`; not an undo step.
    IdPoint {
        p: [f64; 2],
        #[serde(default)]
        kind: Option<SnapKind>,
    },
    /// One row of `fields` per entity `filter` finds (up to 5,000; `limit` says how many).
    /// `Outcome::Table`; not an undo step.
    ExtractData {
        filter: EntityFilter,
        fields: Vec<Field>,
    },
    /// Writes the named view `name` (replacing one of that name) showing `bounds`,
    /// `[[min_x, min_y], [max_x, max_y]]` in drawing coordinates, to the drawing's view table.
    /// Model sheet only.
    SaveView {
        name: String,
        bounds: [[f64; 2]; 2],
    },
    /// Removes the named view `name`; `Outcome::Nothing` when there is none.
    DeleteView {
        name: String,
    },
    /// Deletes the entities `targets` from the sheet on screen: one undo step. Each must be an
    /// entity of the drawing itself at the top of that sheet (an empty chain: not inside a block,
    /// not an xref's); otherwise nothing is deleted.
    DeleteEntities {
        targets: Vec<Target>,
    },
    /// Adds what the spec describes to the sheet on screen, on the current layer with the current
    /// colour, linetype and lineweight (`Session::current`): one undo step. `Outcome::Added` lists
    /// the new entities.
    Draw(DrawSpec),
    /// Sets the drawing state new entities take (`Session::current`). Not an undo step.
    SetCurrent {
        current: Current,
    },
    /// Moves, copies, turns, scales, mirrors, stretches, aligns, arrays or explodes entities at
    /// the top of the sheet on screen (see `ModifySpec`): one undo step.
    Modify(ModifySpec),
    /// Trims, extends, breaks, joins, lengthens, reverses, offsets, fillets, chamfers, blends,
    /// edits polylines, removes duplicates or flattens entities at the top of the sheet on screen
    /// (see `EditSpec`): one undo step.
    Edit(EditSpec),
}

/// An entity as picked (see `Picked`): its handle and the block references it sits inside,
/// outermost first (empty, the default, for an entity on the sheet). A chain ending in an xref
/// reference names an entity of that xref.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub handle: u64,
    #[serde(default)]
    pub chain: Vec<u64>,
}

/// The part of the session a command changes, kept whole in its undo step.
pub(crate) enum Part {
    Takeoff,
    Boq,
    /// The conflicts and the schedules.
    Conflicts,
    /// The drawing itself: the entities the command touches are kept as they were (their
    /// before images), with the drawing's tables when `tables` and the take-off when `takeoff`
    /// (divide and measure along change both).
    Document {
        tables: bool,
        takeoff: bool,
    },
}

impl Command {
    /// The part of the session the command changes, and so keeps in its undo step; `None` when
    /// it is not an undo step.
    pub(crate) fn changes(&self) -> Option<Part> {
        match self {
            Command::SetLayerHidden { .. }
            | Command::SetSheet { .. }
            | Command::SetArabicReversed { .. }
            | Command::SetDrafting { .. }
            | Command::MeasureGeometry { .. }
            | Command::IdPoint { .. }
            | Command::ExtractData { .. }
            | Command::SetCurrent { .. } => None,
            Command::SetBoq { .. } | Command::SetBoqTolerance { .. } => Some(Part::Boq),
            Command::SetConflicts { .. } | Command::ResolveConflict { .. } => Some(Part::Conflicts),
            // Entities only.
            Command::Calibrate { .. }
            | Command::SetImageDisplay { .. }
            | Command::DeleteEntities { .. }
            | Command::Modify(_)
            | Command::Edit(_) => Some(Part::Document {
                tables: false,
                takeoff: false,
            }),
            // A wipeout registers its class with the drawing.
            Command::Draw(spec) => Some(Part::Document {
                tables: matches!(spec, DrawSpec::Wipeout { .. }),
                takeoff: false,
            }),
            // New layers, views, or underlay and image definitions too.
            Command::ImportPdf { .. }
            | Command::AttachPdfUnderlay { .. }
            | Command::AttachImage { .. }
            | Command::SaveView { .. }
            | Command::DeleteView { .. } => Some(Part::Document {
                tables: true,
                takeoff: false,
            }),
            // Points on their own layer, and their count in the take-off.
            Command::DivideAlong { .. } | Command::MeasureAlong { .. } => Some(Part::Document {
                tables: true,
                takeoff: true,
            }),
            _ => Some(Part::Takeoff),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outcome {
    Measured {
        measurement: Box<Measurement>,
    },
    Removed {
        id: String,
    },
    Changed,
    /// Entities added to the drawing, by handle.
    Added {
        handles: Vec<u64>,
    },
    /// An explode's parts, by handle, and how many entities picked had no parts and were left
    /// as they are.
    Exploded {
        handles: Vec<u64>,
        left_alone: usize,
    },
    /// An edit made in place: the entities it added (pieces of a trim or a break, a fillet's
    /// arc...) and removed (joined or duplicates), and how many picked it left as they were.
    Edited {
        added: Vec<u64>,
        removed: Vec<u64>,
        left_alone: usize,
    },
    /// The command named a measurement or BOQ item that does not exist.
    Nothing,
    /// Facts to show: a title (an entity type, or "Point") and label-value rows. Labels are keys
    /// such as "radius" or "centre"; values are written out in metres and degrees.
    Info {
        title: String,
        rows: Vec<(String, String)>,
    },
    /// A table: its column headings and its rows of text.
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

impl Session {
    /// Carries out `cmd` without recording an undo step; `run_as` does that. A new measurement
    /// is marked as made by `source`.
    pub(crate) fn execute(&mut self, cmd: Command, source: Source) -> Result<Outcome, CoreError> {
        let mut measurement = match cmd {
            Command::MeasureDistance { points } => {
                if points.len() < 2 {
                    return Err(invalid("A distance needs at least 2 points"));
                }
                let path = Path {
                    points,
                    closed: false,
                };
                let mut m = self.measurement(Kind::Length, path.length(), "Distance", "");
                m.points = path.points;
                m.auto_description = true;
                m
            }
            Command::MeasureArea { points } => {
                if points.len() < 3 {
                    return Err(invalid("An area needs at least 3 points"));
                }
                let ring = Path {
                    points,
                    closed: true,
                };
                let mut m = self.measurement(Kind::Area, ring.area(), "Area", "");
                m.self_intersecting = ring.self_intersects();
                m.points = ring.points;
                m.auto_description = true;
                m
            }
            Command::MeasureEntity { handle, chain } => {
                let mut m = self.measure_entity(handle, chain)?;
                m.auto_description = true;
                m
            }
            Command::CountBlock { block_name, layer } => {
                let mut m = self.count_block(block_name, layer);
                m.auto_description = true;
                m
            }
            Command::RemoveMeasurement { id } => {
                return Ok(match self.takeoff.remove(&id) {
                    Some(_) => Outcome::Removed { id },
                    None => Outcome::Nothing,
                });
            }
            Command::SetDescription { id, text } => {
                return Ok(match self.takeoff.items.iter_mut().find(|m| m.id == id) {
                    Some(m) => {
                        m.description = text;
                        m.auto_description = false;
                        Outcome::Changed
                    }
                    None => Outcome::Nothing,
                });
            }
            Command::SetLayerHidden { name, hidden } => {
                self.set_layer_hidden(&name, hidden);
                return Ok(Outcome::Changed);
            }
            Command::SetSheet { block } => {
                self.set_sheet(&block)?;
                return Ok(Outcome::Changed);
            }
            Command::SetArabicReversed { on } => {
                self.arabic_reversed = on;
                return Ok(Outcome::Changed);
            }
            Command::SetDrafting { drafting } => {
                drafting.check()?;
                self.drafting = drafting;
                return Ok(Outcome::Changed);
            }
            Command::AddMeasurement {
                kind,
                value,
                unit,
                height,
                points,
                handles,
                chain,
                description,
                note,
            } => {
                let mut m =
                    self.add_measurement(kind, value, unit, height, points, handles, chain)?;
                m.description = description;
                m.note = note;
                m
            }
            Command::SetHeight { id, height } => {
                check_height(height)?;
                let units = self.drawing.units;
                return self.edit(&id, |m| {
                    let area = match (m.kind, m.height) {
                        (Kind::Area, _) => m.value,
                        (Kind::Volume, Some(old)) if old > 0.0 => m.value / old,
                        _ => return Err(invalid("Only an area or a volume takes a height")),
                    };
                    let value = area * height;
                    let (unit, shown) = in_metres(Kind::Volume, value, units);
                    m.kind = Kind::Volume;
                    m.value = value;
                    m.unit = unit.to_string();
                    m.shown = shown;
                    m.height = Some(height);
                    Ok(())
                });
            }
            Command::LinkBoq { id, code } => {
                return self.edit(&id, |m| {
                    m.boq_code = code;
                    Ok(())
                });
            }
            Command::SetNote { id, text } => {
                return self.edit(&id, |m| {
                    m.note = text;
                    Ok(())
                });
            }
            Command::SetBoq { boq } => {
                self.boq = Some(boq);
                return Ok(Outcome::Changed);
            }
            Command::SetBoqTolerance { code, percent } => {
                if percent.is_some_and(|p| !(p >= 0.0 && p.is_finite())) {
                    return Err(invalid("The tolerance must be a percentage of 0 or more"));
                }
                let mut items = self
                    .boq
                    .iter_mut()
                    .flat_map(|b| &mut b.items)
                    .filter(|i| i.code == code)
                    .peekable();
                if items.peek().is_none() {
                    return Ok(Outcome::Nothing);
                }
                items.for_each(|i| i.tolerance = percent);
                return Ok(Outcome::Changed);
            }
            Command::SetConflicts {
                conflicts,
                schedules,
            } => {
                self.conflicts = conflicts;
                self.schedules = schedules;
                return Ok(Outcome::Changed);
            }
            Command::ResolveConflict { id, status, note } => {
                return Ok(match self.conflicts.iter_mut().find(|c| c.id == id) {
                    Some(c) => {
                        c.status = status;
                        c.note = note;
                        Outcome::Changed
                    }
                    None => Outcome::Nothing,
                });
            }
            Command::ImportPdf {
                path,
                page,
                insert_at,
                scale,
            } => {
                let read = PreparedPdf::read(path.as_ref(), page, scale, AttachMode::Import)?;
                let handles = self.apply_prepared(read, insert_at)?;
                return Ok(Outcome::Added { handles });
            }
            Command::AttachPdfUnderlay {
                path,
                page,
                insert_at,
                scale,
            } => {
                let read = PreparedPdf::read(path.as_ref(), page, scale, AttachMode::Underlay)?;
                let handles = self.apply_prepared(read, insert_at)?;
                return Ok(Outcome::Added { handles });
            }
            Command::Calibrate {
                handle,
                p1,
                p2,
                real_distance,
            } => {
                self.rescale(handle, p1, p2, real_distance)?;
                return Ok(Outcome::Changed);
            }
            Command::AttachImage {
                path,
                insert_at,
                width_units,
            } => {
                let handle = self.add_image(path.as_ref(), insert_at, width_units)?;
                return Ok(Outcome::Added {
                    handles: vec![handle],
                });
            }
            Command::SetImageDisplay {
                handle,
                brightness,
                contrast,
                fade,
            } => {
                let display = crate::ImageDisplay {
                    brightness,
                    contrast,
                    fade,
                };
                self.apply_image_display(handle, display)?;
                return Ok(Outcome::Changed);
            }
            Command::MeasureGeometry { handle, chain, at } => {
                let (title, rows) = self.geometry_info(handle, &chain, at)?;
                return Ok(Outcome::Info { title, rows });
            }
            Command::DivideAlong {
                handle,
                chain,
                count,
            } => self.place_along(handle, chain, Spacing::Count(count))?,
            Command::MeasureAlong {
                handle,
                chain,
                step,
            } => self.place_along(handle, chain, Spacing::Step(step))?,
            Command::IdPoint { p, kind } => {
                if !p.iter().all(|c| c.is_finite()) {
                    return Err(invalid("The point must have finite coordinates"));
                }
                let rows = self.point_info(p, kind);
                return Ok(Outcome::Info {
                    title: "Point".to_string(),
                    rows,
                });
            }
            Command::ExtractData { filter, fields } => {
                let (columns, rows) = self.extract(&filter, &fields)?;
                return Ok(Outcome::Table { columns, rows });
            }
            Command::SaveView { name, bounds } => {
                self.save_view(&name, bounds)?;
                return Ok(Outcome::Changed);
            }
            Command::DeleteView { name } => {
                return Ok(if self.delete_view(&name) {
                    Outcome::Changed
                } else {
                    Outcome::Nothing
                });
            }
            Command::DeleteEntities { targets } => {
                self.delete_entities(&targets)?;
                return Ok(Outcome::Changed);
            }
            Command::Draw(spec) => {
                let handles = self.draw(spec)?;
                return Ok(Outcome::Added { handles });
            }
            Command::SetCurrent { current } => {
                self.set_current(current)?;
                return Ok(Outcome::Changed);
            }
            Command::Modify(spec) => return self.modify(spec),
            Command::Edit(spec) => return self.edit_entities(spec),
        };
        measurement.source = source.name().to_string();
        self.takeoff.add(measurement.clone());
        Ok(Outcome::Measured {
            measurement: Box::new(measurement),
        })
    }

    /// Runs `f` on the measurement with `id`: `Changed`, or `Nothing` when there is none.
    fn edit(
        &mut self,
        id: &str,
        f: impl FnOnce(&mut Measurement) -> Result<(), CoreError>,
    ) -> Result<Outcome, CoreError> {
        match self.takeoff.items.iter_mut().find(|m| m.id == id) {
            Some(m) => f(m).map(|()| Outcome::Changed),
            None => Ok(Outcome::Nothing),
        }
    }

    /// The measurement of an `AddMeasurement`, without its description and note.
    #[allow(clippy::too_many_arguments)]
    fn add_measurement(
        &self,
        kind: Kind,
        value: Option<f64>,
        unit: Option<String>,
        height: Option<f64>,
        points: Vec<[f64; 2]>,
        handles: Vec<u64>,
        chain: Vec<u64>,
    ) -> Result<Measurement, CoreError> {
        if kind != Kind::Item && points.is_empty() && handles.is_empty() {
            return Err(invalid("A measurement needs geometry"));
        }
        if !points.is_empty() && !handles.is_empty() {
            return Err(invalid("Give points or handles, not both"));
        }
        if kind == Kind::Item {
            if !value.is_some_and(f64::is_finite) {
                return Err(invalid("An item needs a value"));
            }
            if unit.as_deref().is_none_or(|u| u.trim().is_empty()) {
                return Err(invalid("An item needs a unit"));
            }
        } else if value.is_some() || unit.is_some() {
            return Err(invalid(
                "Only an item takes a value and a unit; this is measured from its geometry",
            ));
        }
        match (kind, height) {
            (Kind::Volume, Some(h)) => check_height(h)?,
            (Kind::Volume, None) => return Err(invalid("A volume needs a height")),
            (_, Some(_)) => return Err(invalid("Only a volume takes a height")),
            (_, None) => {}
        }
        if !chain.is_empty() && handles.is_empty() {
            return Err(invalid("A chain goes with handles"));
        }
        let xrefs = self.xref_docs();
        let entities = handles
            .iter()
            .map(|&h| locate(&self.drawing.doc, &xrefs, h, &chain))
            .collect::<Result<Vec<_>, _>>()?;

        let mut self_intersecting = false;
        let value = match kind {
            Kind::Length if !points.is_empty() => {
                if points.len() < 2 {
                    return Err(invalid("A distance needs at least 2 points"));
                }
                Path {
                    points: points.clone(),
                    closed: false,
                }
                .length()
            }
            Kind::Length => entities
                .iter()
                .map(|e| {
                    self.extent(e.doc, e.entity, &e.xf)
                        .length
                        .ok_or_else(|| invalid("This entity has no length"))
                })
                .sum::<Result<f64, _>>()?,
            Kind::Area | Kind::Volume if !points.is_empty() => {
                if points.len() < 3 {
                    return Err(invalid("An area needs at least 3 points"));
                }
                let ring = Path {
                    points: points.clone(),
                    closed: true,
                };
                self_intersecting = ring.self_intersects();
                ring.area()
            }
            Kind::Area | Kind::Volume => entities
                .iter()
                .map(|e| {
                    let extent = self.extent(e.doc, e.entity, &e.xf);
                    self_intersecting |= extent.self_intersecting;
                    extent
                        .area
                        .ok_or_else(|| invalid("This entity has no area"))
                })
                .sum::<Result<f64, _>>()?,
            // One piece per point or per handle, whichever was given.
            Kind::Count => points.len().max(handles.len()) as f64,
            Kind::Item => value.unwrap_or_default(),
        };
        let value = match height {
            Some(h) => value * h,
            None => value,
        };
        let layer = entities.first().map_or("", |e| e.layer.as_str());
        let mut m = self.measurement(kind, value, "", layer);
        if kind == Kind::Item {
            m.unit = unit.unwrap_or_default();
        }
        m.height = height;
        m.self_intersecting = self_intersecting;
        m.points = points;
        m.handles = handles;
        if let Some(first) = entities.first() {
            m.block_path = first.block_path.clone();
            m.xref = first.xref.clone();
        }
        if kind == Kind::Count && m.xref.is_some() {
            // An xref's handles are never embedded: keep where each reference sits instead.
            m.points = entities.iter().filter_map(insert_point).collect();
        }
        m.chain = chain;
        Ok(m)
    }

    /// A measurement on the current sheet in the drawing's units.
    fn measurement(&self, kind: Kind, value: f64, description: &str, layer: &str) -> Measurement {
        Measurement::new(
            kind,
            value,
            self.drawing.units,
            description.to_string(),
            layer.to_string(),
            self.sheet.clone(),
        )
    }

    fn measure_entity(&self, handle: u64, chain: Vec<u64>) -> Result<Measurement, CoreError> {
        let xrefs = self.xref_docs();
        let located = locate(&self.drawing.doc, &xrefs, handle, &chain)?;
        let entity = located.entity;
        let layer = &located.layer;
        let description = format!("{} on {}", variant_name(entity), short_layer_name(layer));
        let mut m = if let EntityType::Insert(insert) = entity {
            let mut m = self.measurement(Kind::Count, 1.0, &description, layer);
            m.block_name = Some(effective_block_name(located.doc, insert));
            m
        } else {
            let extent = self.extent(located.doc, entity, &located.xf);
            match (extent.area, extent.length) {
                (Some(area), _) => {
                    let mut m = self.measurement(Kind::Area, area, &description, layer);
                    m.self_intersecting = extent.self_intersecting;
                    m
                }
                (None, Some(length)) => self.measurement(Kind::Length, length, &description, layer),
                (None, None) => return Err(invalid("This entity has no length or area")),
            }
        };
        if m.kind == Kind::Count && located.xref.is_some() {
            // An xref's handles are never embedded: keep where the reference sits instead.
            m.points = insert_point(&located).into_iter().collect();
        }
        m.handles = vec![handle];
        m.block_path = located.block_path;
        m.xref = located.xref;
        m.chain = chain;
        Ok(m)
    }

    /// Counts top-level references only: a block nested inside another block is not counted yet.
    /// A dynamic block's anonymous copies count as the dynamic block. References at the top of
    /// the sheet's xrefs count too (on their `XREF|LAYER`): they are kept as their points, since
    /// their handles are the xref drawing's.
    fn count_block(&self, block_name: String, layer: Option<String>) -> Measurement {
        let block = normalize_name(&block_name);
        let only_on = layer.as_deref().map(normalize_name);
        let doc = &self.drawing.doc;
        let xrefs = self.xref_docs();
        let shown = |layer: &str| layer_shown_with(doc, &xrefs, &self.visibility, layer);
        let wanted = |layer: &str| only_on.as_ref().is_none_or(|l| *l == normalize_name(layer));
        let handles: Vec<u64> = self
            .drawing
            .entities_in(&self.sheet)
            .filter_map(|e| match e {
                EntityType::Insert(insert)
                    if normalize_name(&effective_block_name(doc, insert)) == block =>
                {
                    Some(e.common())
                }
                _ => None,
            })
            .filter(|c| !c.invisible && shown(&c.layer) && wanted(&c.layer))
            .map(|c| c.handle.value())
            .collect();
        let mut points = Vec::new();
        for x in self.sheet_xrefs(&self.sheet) {
            if !shown(&x.insert.common.layer) {
                continue;
            }
            for e in x.doc.entities_in_block("*Model_Space") {
                let EntityType::Insert(insert) = e else {
                    continue;
                };
                let on = x.layer_of(e);
                let counted = !e.common().invisible
                    && normalize_name(&effective_block_name(x.doc, insert)) == block
                    && shown(&on)
                    && wanted(&on);
                if counted {
                    let p = insert.insert_point;
                    let on_plan = object_to_world(&insert.normal, p.z).apply([p.x, p.y]);
                    points.push(x.xf.apply(on_plan));
                }
            }
        }
        let description = format!("{block_name} count");
        let mut m = self.measurement(
            Kind::Count,
            (handles.len() + points.len()) as f64,
            &description,
            layer.as_deref().unwrap_or(""),
        );
        m.handles = handles;
        m.points = points;
        m.block_name = Some(block_name);
        m
    }

    /// Removes the entities `targets` from the sheet on screen, once every one is found at its
    /// top.
    /// Images and PDF underlays leave their definitions' links; take-off rows measuring a
    /// deleted entity keep their value without the link.
    fn delete_entities(&mut self, targets: &[Target]) -> Result<(), CoreError> {
        let handles = self.sheet_targets(targets)?;
        for &h in &handles {
            let doc = &mut self.drawing.doc;
            match doc.get_entity(h) {
                Some(EntityType::RasterImage(image)) => {
                    if let Some(reactor) = image.definition_reactor_handle {
                        doc.objects.remove(&reactor);
                    }
                }
                Some(EntityType::Underlay(underlay)) => {
                    let definition = underlay.definition_handle;
                    if let Some(ObjectType::UnderlayDefinition(def)) =
                        doc.objects.get_mut(&definition)
                    {
                        def.reactors.retain(|r| *r != h);
                    }
                }
                _ => {}
            }
            self.drawing.doc.remove_entity_arc(h);
        }
        self.follow_takeoff(&[], &handles);
        Ok(())
    }

    /// The handles of `targets` (each once, in order) when every one is an entity of the drawing
    /// itself at the top of the sheet on screen (an empty chain: not inside a block, not an
    /// xref's); otherwise why not.
    pub(crate) fn sheet_targets(&self, targets: &[Target]) -> Result<Vec<Handle>, CoreError> {
        if targets.is_empty() {
            return Err(invalid("Pick the entities first"));
        }
        let doc = &self.drawing.doc;
        let sheet = self.sheet_owner();
        let mut handles: Vec<Handle> = Vec::with_capacity(targets.len());
        let mut seen = std::collections::HashSet::with_capacity(targets.len());
        for Target { handle: h, chain } in targets {
            if !chain.is_empty() {
                return Err(CoreError::InvalidCommand(match xref_of_chain(doc, chain) {
                    Some(name) => {
                        format!("Entity {h:X} belongs to the xref {name}, which is read-only")
                    }
                    None => {
                        format!("Entity {h:X} is inside a block: pick the block reference instead")
                    }
                }));
            }
            let entity = doc
                .get_entity(Handle::new(*h))
                .ok_or(CoreError::NoSuchEntity(*h))?;
            if Some(entity.common().owner_handle) != sheet {
                return Err(CoreError::InvalidCommand(format!(
                    "Entity {h:X} is not on this sheet"
                )));
            }
            if seen.insert(*h) {
                handles.push(Handle::new(*h));
            }
        }
        Ok(handles)
    }
}

/// Where a located block reference sits in the world: its insertion point (in its own object
/// coordinates) placed by the chain. `None` for anything else.
pub(crate) fn insert_point(located: &Located) -> Option<[f64; 2]> {
    let EntityType::Insert(insert) = located.entity else {
        return None;
    };
    let p = insert.insert_point;
    let on_plan = object_to_world(&insert.normal, p.z).apply([p.x, p.y]);
    Some(located.xf.apply(on_plan))
}

/// A height must be a number above 0.
fn check_height(height: f64) -> Result<(), CoreError> {
    if height > 0.0 && height.is_finite() {
        Ok(())
    } else {
        Err(invalid("The height must be more than 0"))
    }
}

fn invalid(why: &str) -> CoreError {
    CoreError::InvalidCommand(why.to_string())
}

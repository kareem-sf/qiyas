//! The command bus: every change to the take-off goes through `Session::run` (the user) or
//! `Session::run_as` (the AI, or an outside AI through MCP).

use opencadcodec::tables::normalize_name;
use opencadcodec::{EntityType, Handle};
use qiyas_geom::Path;
use serde::{Deserialize, Serialize};

use crate::session::variant_name;
use crate::takeoff::in_metres;
use crate::{CoreError, Kind, Measurement, Session, Source};

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
    MeasureEntity {
        handle: u64,
    },
    /// Block references of `block_name` on the current sheet, on visible layers only
    /// (and only on `layer` when given).
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
}

impl Command {
    /// Whether the command changes the take-off and so is an undo step.
    pub(crate) fn is_undoable(&self) -> bool {
        !matches!(
            self,
            Command::SetLayerHidden { .. } | Command::SetSheet { .. }
        )
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
    /// The command named a measurement that does not exist.
    Nothing,
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
                m
            }
            Command::MeasureEntity { handle } => self.measure_entity(handle)?,
            Command::CountBlock { block_name, layer } => self.count_block(block_name, layer),
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
            Command::AddMeasurement {
                kind,
                value,
                unit,
                height,
                points,
                handles,
                description,
                note,
            } => {
                let mut m = self.add_measurement(kind, value, unit, height, points, handles)?;
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
    fn add_measurement(
        &self,
        kind: Kind,
        value: Option<f64>,
        unit: Option<String>,
        height: Option<f64>,
        points: Vec<[f64; 2]>,
        handles: Vec<u64>,
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
        let entities = handles
            .iter()
            .map(|&h| {
                self.drawing
                    .doc
                    .get_entity(Handle::new(h))
                    .ok_or(CoreError::NoSuchEntity(h))
            })
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
                    self.extent(e)
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
                    let extent = self.extent(e);
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
        let layer = entities.first().map_or("", |e| e.common().layer.as_str());
        let mut m = self.measurement(kind, value, "", layer);
        if kind == Kind::Item {
            m.unit = unit.unwrap_or_default();
        }
        m.height = height;
        m.self_intersecting = self_intersecting;
        m.points = points;
        m.handles = handles;
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

    fn measure_entity(&self, handle: u64) -> Result<Measurement, CoreError> {
        let entity = self
            .drawing
            .doc
            .get_entity(Handle::new(handle))
            .ok_or(CoreError::NoSuchEntity(handle))?;
        let layer = &entity.common().layer;
        let description = format!("{} on {layer}", variant_name(entity));
        let mut m = if let EntityType::Insert(insert) = entity {
            let mut m = self.measurement(Kind::Count, 1.0, &description, layer);
            m.block_name = Some(insert.block_name.clone());
            m
        } else {
            let extent = self.extent(entity);
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
        m.handles = vec![handle];
        Ok(m)
    }

    /// Counts top-level references only: a block nested inside another block is not counted yet.
    fn count_block(&self, block_name: String, layer: Option<String>) -> Measurement {
        let block = normalize_name(&block_name);
        let only_on = layer.as_deref().map(normalize_name);
        let handles: Vec<u64> = self
            .drawing
            .entities_in(&self.sheet)
            .filter_map(|e| match e {
                EntityType::Insert(insert) if normalize_name(&insert.block_name) == block => {
                    Some(e.common())
                }
                _ => None,
            })
            .filter(|c| {
                !c.invisible
                    && self.layer_visible(&c.layer)
                    && only_on
                        .as_ref()
                        .is_none_or(|l| *l == normalize_name(&c.layer))
            })
            .map(|c| c.handle.value())
            .collect();
        let description = format!("{block_name} count");
        let mut m = self.measurement(
            Kind::Count,
            handles.len() as f64,
            &description,
            layer.as_deref().unwrap_or(""),
        );
        m.handles = handles;
        m.block_name = Some(block_name);
        m
    }
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

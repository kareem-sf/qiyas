//! The command bus: every change to the take-off goes through `Session::run`, whether the
//! user or the AI asks for it.

use opencadcodec::tables::normalize_name;
use opencadcodec::{EntityType, Handle};
use qiyas_geom::Path;
use serde::{Deserialize, Serialize};

use crate::session::variant_name;
use crate::{CoreError, Kind, Measurement, Session};

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
    /// Carries out `cmd` without recording an undo step; `run` does that.
    pub(crate) fn execute(&mut self, cmd: Command) -> Result<Outcome, CoreError> {
        let measurement = match cmd {
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
        };
        self.takeoff.add(measurement.clone());
        Ok(Outcome::Measured {
            measurement: Box::new(measurement),
        })
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

fn invalid(why: &str) -> CoreError {
    CoreError::InvalidCommand(why.to_string())
}

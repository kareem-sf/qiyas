//! The user coordinate system (`Session::ucs`): an origin and an x axis in the plan. The window
//! shows the cursor in it, reads typed points in it and lays the grid and ortho along it. View
//! state, like the sheet on screen: `Command::Ucs` and `Command::UcsObject` are no undo step. It
//! is kept in the project file; saving writes it to the drawing's header (UCSORG, UCSXDIR,
//! UCSYDIR) and its UCS table (an entry `QIYAS`), so AutoCAD opens with the same one. A drawing
//! opened without a project file takes the one its header keeps.

use opencadcodec::tables::Ucs as UcsEntry;
use opencadcodec::{CadDocument, EntityType, Vector3};
use serde::{Deserialize, Serialize};

use crate::session::locate;
use crate::{CoreError, Session, Target};

/// The name of the UCS table entry Qiyas writes.
const NAME: &str = "QIYAS";

fn invalid(why: impl Into<String>) -> CoreError {
    CoreError::InvalidCommand(why.into())
}

/// An origin and a unit x axis in the plan; the y axis is the x axis turned a quarter left.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ucs {
    pub origin: [f64; 2],
    pub x_axis: [f64; 2],
}

impl Default for Ucs {
    /// The world coordinate system.
    fn default() -> Self {
        Ucs {
            origin: [0.0, 0.0],
            x_axis: [1.0, 0.0],
        }
    }
}

impl Ucs {
    /// A UCS at `origin` with its x axis along `x_axis` (any length above 0).
    pub fn new(origin: [f64; 2], x_axis: [f64; 2]) -> Result<Ucs, CoreError> {
        let length = x_axis[0].hypot(x_axis[1]);
        if !origin.iter().all(|v| v.is_finite()) || !(length > 1e-12 && length.is_finite()) {
            return Err(invalid(
                "A UCS needs a finite origin and an x axis of some length",
            ));
        }
        Ok(Ucs {
            origin,
            x_axis: [x_axis[0] / length, x_axis[1] / length],
        })
    }

    pub fn is_world(&self) -> bool {
        *self == Ucs::default()
    }

    pub fn y_axis(&self) -> [f64; 2] {
        [-self.x_axis[1], self.x_axis[0]]
    }

    /// The world point `p` in this UCS.
    pub fn to_ucs(&self, p: [f64; 2]) -> [f64; 2] {
        let d = [p[0] - self.origin[0], p[1] - self.origin[1]];
        let [x, y] = [self.x_axis, self.y_axis()];
        [d[0] * x[0] + d[1] * x[1], d[0] * y[0] + d[1] * y[1]]
    }

    /// The point `p` of this UCS in the world.
    pub fn to_world(&self, p: [f64; 2]) -> [f64; 2] {
        let [x, y] = [self.x_axis, self.y_axis()];
        [
            self.origin[0] + p[0] * x[0] + p[1] * y[0],
            self.origin[1] + p[0] * x[1] + p[1] * y[1],
        ]
    }

    /// The UCS `doc`'s header keeps for model space, when its axes lie in the plan.
    pub(crate) fn from_header(doc: &CadDocument) -> Option<Ucs> {
        let h = &doc.header;
        let (o, x) = (h.model_space_ucs_origin, h.model_space_ucs_x_axis);
        if x.z.abs() > 1e-9 {
            return None;
        }
        Ucs::new([o.x, o.y], [x.x, x.y]).ok()
    }

    /// Writes this UCS to `doc`'s header, and to its UCS table when it is not the world's.
    pub(crate) fn write_to(&self, doc: &mut CadDocument) {
        let [ox, oy] = self.origin;
        let ([xx, xy], [yx, yy]) = (self.x_axis, self.y_axis());
        let (origin, x, y) = (
            Vector3::new(ox, oy, 0.0),
            Vector3::new(xx, xy, 0.0),
            Vector3::new(yx, yy, 0.0),
        );
        if !self.is_world() {
            match doc.ucss.get_mut(NAME) {
                Some(entry) => {
                    entry.origin = origin;
                    entry.x_axis = x;
                    entry.y_axis = y;
                }
                None => {
                    let mut entry = UcsEntry::from_origin_axes(NAME, origin, x, y);
                    entry.handle = doc.allocate_handle();
                    let _ = doc.ucss.add(entry);
                }
            }
        }
        let h = &mut doc.header;
        h.model_space_ucs_name = if self.is_world() {
            String::new()
        } else {
            NAME.to_string()
        };
        h.model_space_ucs_origin = origin;
        h.model_space_ucs_x_axis = x;
        h.model_space_ucs_y_axis = y;
    }
}

impl Session {
    /// `Command::UcsObject`: the UCS of the entity `target` (inside blocks too): a line's or a
    /// polyline's first segment from its start, an arc's or a circle's centre towards angle 0
    /// (an arc's start), a text's, an MText's or a block reference's insertion point along its
    /// rotation.
    pub(crate) fn ucs_of_entity(&self, target: &Target) -> Result<Ucs, CoreError> {
        let xrefs = self.xref_docs();
        let located = locate(&self.drawing.doc, &xrefs, target.handle, &target.chain)?;
        let along = |p: [f64; 2], angle: f64| (p, [p[0] + angle.cos(), p[1] + angle.sin()]);
        let (a, b) = match located.entity {
            EntityType::Line(l) => ([l.start.x, l.start.y], [l.end.x, l.end.y]),
            EntityType::LwPolyline(p) if p.vertices.len() >= 2 => {
                let [v0, v1] = [&p.vertices[0], &p.vertices[1]];
                (
                    [v0.location.x, v0.location.y],
                    [v1.location.x, v1.location.y],
                )
            }
            EntityType::Arc(a) => along([a.center.x, a.center.y], a.start_angle),
            EntityType::Circle(c) => along([c.center.x, c.center.y], 0.0),
            EntityType::Text(t) => along([t.insertion_point.x, t.insertion_point.y], t.rotation),
            EntityType::MText(t) => along([t.insertion_point.x, t.insertion_point.y], t.rotation),
            EntityType::Insert(i) => along([i.insert_point.x, i.insert_point.y], i.rotation),
            _ => return Err(invalid(
                "UCS OB takes a line, a polyline, an arc, a circle, a text or a block reference",
            )),
        };
        let (a, b) = (located.xf.apply(a), located.xf.apply(b));
        Ucs::new(a, [b[0] - a[0], b[1] - a[1]])
    }
}

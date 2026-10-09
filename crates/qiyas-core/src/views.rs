//! Named views: the drawing's `VIEW` table, model-space entries only, as boxes on the drawing.

use opencadcodec::tables::View;
use opencadcodec::Vector3;
use serde::Serialize;

use crate::{CoreError, Session};

/// A named view of model space: its name and the box it shows, `[[min_x, min_y], [max_x, max_y]]`
/// in drawing coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NamedView {
    pub name: String,
    pub bounds: [[f64; 2]; 2],
}

impl Session {
    /// The drawing's named plan views of model space, in the table's order: a view looking
    /// another way than straight down (direction (0, 0, 1)) is left out. A view's centre is
    /// taken from its target.
    pub fn views(&self) -> Vec<NamedView> {
        self.drawing
            .doc
            .views
            .iter()
            .filter(|v| {
                let d = &v.direction;
                let plan = d.x.abs() < 1e-9 && d.y.abs() < 1e-9 && d.z > 0.0;
                plan && !v.paper_space && v.width > 0.0 && v.height > 0.0
            })
            .map(|v| {
                let (cx, cy) = (v.center.x + v.target.x, v.center.y + v.target.y);
                let (hw, hh) = (v.width / 2.0, v.height / 2.0);
                NamedView {
                    name: v.name.clone(),
                    bounds: [[cx - hw, cy - hh], [cx + hw, cy + hh]],
                }
            })
            .collect()
    }

    /// Writes the view `name` showing `bounds` (replacing one of that name, keeping its handle).
    /// Only on the Model sheet.
    pub(crate) fn save_view(&mut self, name: &str, bounds: [[f64; 2]; 2]) -> Result<(), CoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::InvalidCommand("The view needs a name".into()));
        }
        if !self.sheet.eq_ignore_ascii_case("*Model_Space") {
            return Err(CoreError::InvalidCommand(
                "Named views are saved on the Model sheet".into(),
            ));
        }
        let [[x0, y0], [x1, y1]] = bounds;
        let (w, h) = (x1 - x0, y1 - y0);
        if ![x0, y0, x1, y1].iter().all(|c| c.is_finite()) || w <= 0.0 || h <= 0.0 {
            return Err(CoreError::InvalidCommand(
                "The view's box must have a width and a height".into(),
            ));
        }
        let doc = &mut self.drawing.doc;
        let handle = match doc.views.get(name) {
            Some(old) => old.handle,
            None => doc.allocate_handle(),
        };
        let mut view = View::new(name);
        view.handle = handle;
        view.center = Vector3::new(x0 + w / 2.0, y0 + h / 2.0, 0.0);
        view.width = w;
        view.height = h;
        // By name, any case: an entry of another case is the same view.
        doc.views.add_or_replace(view);
        Ok(())
    }

    /// Removes the view `name`; false when there is none.
    pub(crate) fn delete_view(&mut self, name: &str) -> bool {
        self.drawing.doc.views.remove(name).is_some()
    }
}

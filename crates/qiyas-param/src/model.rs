//! The sketch the solver works on: points (two unknowns each), circle radii (one unknown each),
//! curves made of them, and the constraints between them.

/// A point of the sketch, by its place in `Sketch::points`.
pub type PointId = usize;
/// A curve of the sketch, by its place in `Sketch::curves`.
pub type CurveId = usize;

/// A curve, made of the sketch's points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    /// A straight segment from `a` to `b`.
    Line { a: PointId, b: PointId },
    /// A circle round `centre`; its radius is `Sketch::radii[radius]`.
    Circle { centre: PointId, radius: usize },
    /// An arc round `centre` from `start` to `end` (counter-clockwise). The solver keeps both ends
    /// on one circle; the radius is how far `start` is from `centre`.
    Arc {
        centre: PointId,
        start: PointId,
        end: PointId,
    },
}

/// A constraint between points and curves. Angles are in radians, lengths in drawing units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    /// The two points are one.
    Coincident(PointId, PointId),
    /// The point stays where it is given.
    Fixed(PointId, [f64; 2]),
    /// The line runs along the x axis.
    Horizontal(CurveId),
    /// The two points lie on one horizontal line.
    HorizontalPoints(PointId, PointId),
    /// The line runs along the y axis.
    Vertical(CurveId),
    /// The two points lie on one vertical line.
    VerticalPoints(PointId, PointId),
    /// Two lines run the same way.
    Parallel(CurveId, CurveId),
    /// Two lines meet at a right angle.
    Perpendicular(CurveId, CurveId),
    /// Two lines lie on one infinite line.
    Collinear(CurveId, CurveId),
    /// Two circles or arcs share their centre.
    Concentric(CurveId, CurveId),
    /// A line and a circle or arc, or two circles or arcs, touch (on the side they are on now).
    Tangent(CurveId, CurveId),
    /// Two lines are as long, or two circles or arcs as big.
    Equal(CurveId, CurveId),
    /// The two points are mirror images in the line.
    Symmetric(PointId, PointId, CurveId),
    /// The distance from `a` to `b`: measured along `axis` (a unit vector; a linear dimension)
    /// when given, else straight (aligned). Along an axis it keeps its sign as it is now.
    Distance {
        a: PointId,
        b: PointId,
        axis: Option<[f64; 2]>,
        value: f64,
    },
    /// The angle between two lines, `value` radians (0 to pi), turning the way it turns now.
    Angle {
        first: CurveId,
        second: CurveId,
        value: f64,
    },
    /// A circle's or an arc's radius.
    Radius { curve: CurveId, value: f64 },
}

/// Points, radii, curves and constraints, as `qiyas-core` builds them from the drawing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sketch {
    pub points: Vec<[f64; 2]>,
    pub radii: Vec<f64>,
    pub curves: Vec<Curve>,
    pub constraints: Vec<Constraint>,
}

impl Sketch {
    /// Adds a point; its id.
    pub fn point(&mut self, p: [f64; 2]) -> PointId {
        self.points.push(p);
        self.points.len() - 1
    }

    /// Adds a line from `a` to `b` (new points); its id.
    pub fn line(&mut self, a: [f64; 2], b: [f64; 2]) -> CurveId {
        let (a, b) = (self.point(a), self.point(b));
        self.curve(Curve::Line { a, b })
    }

    /// Adds a circle (a new centre point and radius); its id.
    pub fn circle(&mut self, centre: [f64; 2], radius: f64) -> CurveId {
        let centre = self.point(centre);
        self.radii.push(radius);
        let radius = self.radii.len() - 1;
        self.curve(Curve::Circle { centre, radius })
    }

    /// Adds `curve` over points already in the sketch; its id.
    pub fn curve(&mut self, curve: Curve) -> CurveId {
        self.curves.push(curve);
        self.curves.len() - 1
    }

    /// Adds a constraint.
    pub fn constrain(&mut self, c: Constraint) {
        self.constraints.push(c);
    }

    /// The two end points of the line `c`, if it is one.
    pub fn line_ends(&self, c: CurveId) -> Option<(PointId, PointId)> {
        match self.curves.get(c)? {
            Curve::Line { a, b } => Some((*a, *b)),
            _ => None,
        }
    }

    /// The radius of the circle or arc `c` now.
    pub fn radius_of(&self, c: CurveId) -> Option<f64> {
        match *self.curves.get(c)? {
            Curve::Circle { radius, .. } => self.radii.get(radius).copied(),
            Curve::Arc { centre, start, .. } => {
                let (c, s) = (self.points[centre], self.points[start]);
                Some((s[0] - c[0]).hypot(s[1] - c[1]))
            }
            Curve::Line { .. } => None,
        }
    }
}

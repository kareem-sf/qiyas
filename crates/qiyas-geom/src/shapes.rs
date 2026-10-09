//! Entities as opencadkernel curves, in world coordinates, for object snaps. A snap needs the
//! true curve, not the chords it is drawn with: the centre of an arc is not a vertex of its
//! flattened path, and the middle of a chord is not the middle of the arc.

use std::f64::consts::TAU;

use opencadcodec::{CadDocument, EntityType, Vector3};
use opencadkernel::geom2d::{
    Arc, Circle, Curve, Ellipse, EllipseArc, Line, Polyline, PolylineVertex,
};

use crate::exact::uniform_scale;
use crate::flatten::{minor_axis, ocs_of, sweep};
use crate::{flatten, Xform};

/// Bulges smaller than this are straight segments, as in `flatten`.
const MIN_BULGE: f64 = 1e-12;
/// Relative tolerance for telling perpendicular axes from skewed ones.
const SQUARE: f64 = 1e-9;

/// One piece of an entity's outline, ready for snapping.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapShape {
    /// Lines and arcs (or one circle or ellipse) the shape is made of, in world coordinates.
    pub pieces: Vec<Curve>,
    /// The pieces are the entity's true geometry, so their ends, middles and centres are snap
    /// points. False for an outline sampled from a curve with no exact form here (a spline, a
    /// hatch, an arc stretched by its block): only `ends` are offered then.
    pub exact: bool,
    /// The vertices of a polyline (node snaps); empty for other entities.
    pub vertices: Vec<[f64; 2]>,
    /// For a sampled open outline, its two ends; empty otherwise.
    pub ends: Vec<[f64; 2]>,
}

/// The snap shapes of `entity` placed by `xf` (its block references; the entity's own extrusion
/// normal is applied here). Lines, arcs, circles, ellipses and polylines are exact, as long as
/// `xf` keeps circles round; everything else that is drawn is its flattened outline, sampled to
/// `tol`. Texts, block references and points give none.
pub fn snap_shapes(doc: &CadDocument, entity: &EntityType, xf: &Xform, tol: f64) -> Vec<SnapShape> {
    let world = match ocs_of(entity) {
        Some(ocs) => ocs.then(xf),
        None => *xf,
    };
    let exact = match entity {
        EntityType::Line(e) => line(world.apply(xy(&e.start)), world.apply(xy(&e.end))),
        EntityType::Circle(e) => similar(&world).map(|(s, _)| {
            single(Curve::Circle(Circle {
                centre: world.apply(xy(&e.center)),
                radius: e.radius * s,
            }))
        }),
        EntityType::Arc(e) => similar(&world).map(|(s, mirrored)| {
            let c = xy(&e.center);
            let at = |a: f64| world.apply([c[0] + e.radius * a.cos(), c[1] + e.radius * a.sin()]);
            let centre = world.apply(c);
            let angle = |p: [f64; 2]| (p[1] - centre[1]).atan2(p[0] - centre[0]);
            let full = sweep(e.start_angle, e.end_angle) == TAU;
            let (a0, a1) = (angle(at(e.start_angle)), angle(at(e.end_angle)));
            // A mirror turns the arc clockwise: run it the other way round.
            let (start_angle, end_angle) = match (full, mirrored) {
                (true, _) => (a0, a0 + TAU),
                (false, false) => (a0, a1),
                (false, true) => (a1, a0),
            };
            single(Curve::Arc(Arc {
                centre,
                radius: e.radius * s,
                start_angle,
                end_angle,
            }))
        }),
        EntityType::Ellipse(e) => ellipse(e, &world),
        EntityType::LwPolyline(e) => {
            let vertices: Vec<_> = e
                .vertices
                .iter()
                .map(|v| ([v.location.x, v.location.y], v.bulge))
                .collect();
            polyline(&vertices, e.is_closed, &world)
        }
        EntityType::Polyline2D(e) => {
            let vertices: Vec<_> = e
                .vertices
                .iter()
                .map(|v| (xy(&v.location), v.bulge))
                .collect();
            polyline(&vertices, e.is_closed(), &world)
        }
        EntityType::Polyline(e) => {
            let vertices: Vec<_> = e.vertices.iter().map(|v| (xy(&v.location), 0.0)).collect();
            polyline(&vertices, e.is_closed(), &world)
        }
        EntityType::Polyline3D(e) => {
            let vertices: Vec<_> = e.vertices.iter().map(|v| (xy(&v.position), 0.0)).collect();
            polyline(&vertices, e.is_closed(), &world)
        }
        EntityType::Text(_)
        | EntityType::MText(_)
        | EntityType::Insert(_)
        | EntityType::Point(_) => return Vec::new(),
        _ => None,
    };
    match exact {
        Some(shape) => vec![shape],
        // `flatten` applies the extrusion normal itself.
        None => sampled(doc, entity, xf, tol),
    }
}

/// The scale of `xf` and whether it mirrors, when it keeps circles round.
fn similar(xf: &Xform) -> Option<(f64, bool)> {
    let s = uniform_scale(xf)?;
    let [[a, b, _], [c, d, _], _] = xf.m;
    Some((s, a * d - b * c < 0.0))
}

fn single(curve: Curve) -> SnapShape {
    SnapShape {
        pieces: vec![curve],
        exact: true,
        vertices: Vec::new(),
        ends: Vec::new(),
    }
}

/// A straight segment; `None` (sampled instead) when it has no length.
fn line(start: [f64; 2], end: [f64; 2]) -> Option<SnapShape> {
    let long = (end[0] - start[0]).hypot(end[1] - start[1]) > 1e-12;
    // A zero-length line still offers its one point, through the sampled outline.
    long.then(|| single(Curve::Line(Line { start, end })))
}

/// An ellipse or elliptical arc on plan, when its axes stay perpendicular after `xf` (not
/// stretched askew, and lying flat).
fn ellipse(e: &opencadcodec::Ellipse, xf: &Xform) -> Option<SnapShape> {
    let linear = |v: [f64; 2]| {
        let m = &xf.m;
        [
            m[0][0] * v[0] + m[0][1] * v[1],
            m[1][0] * v[0] + m[1][1] * v[1],
        ]
    };
    let u = linear(xy(&e.major_axis));
    let v = linear(minor_axis(&e.normal, &e.major_axis, e.minor_axis_ratio));
    let (a, b) = (u[0].hypot(u[1]), v[0].hypot(v[1]));
    if a < 1e-12 || b < 1e-12 || (u[0] * v[0] + u[1] * v[1]).abs() > SQUARE * a * b {
        return None;
    }
    let ellipse = Ellipse {
        centre: xf.apply(xy(&e.center)),
        major_radius: a,
        minor_radius: b,
        major_axis: [u[0] / a, u[1] / a],
    };
    let arc = if sweep(e.start_parameter, e.end_parameter) == TAU {
        EllipseArc::full(ellipse)
    } else if u[0] * v[1] - u[1] * v[0] > 0.0 {
        EllipseArc {
            ellipse,
            start_parameter: e.start_parameter,
            end_parameter: e.end_parameter,
        }
    } else {
        // The minor axis points clockwise of the major one: the parameter runs the other way.
        EllipseArc {
            ellipse,
            start_parameter: -e.end_parameter,
            end_parameter: -e.start_parameter,
        }
    };
    Some(single(Curve::Ellipse(arc)))
}

/// A polyline of (possibly bulged) segments. Bulges need a transform that keeps circles round,
/// and flip with a mirror; straight ones survive any.
fn polyline(vertices: &[([f64; 2], f64)], closed: bool, xf: &Xform) -> Option<SnapShape> {
    if vertices.len() < 2 {
        return None;
    }
    let bulged = vertices.iter().any(|(_, b)| b.abs() >= MIN_BULGE);
    let flip = if bulged {
        if similar(xf)?.1 {
            -1.0
        } else {
            1.0
        }
    } else {
        1.0
    };
    let placed: Vec<PolylineVertex> = vertices
        .iter()
        .map(|&(p, bulge)| PolylineVertex {
            position: xf.apply(p),
            bulge: if bulge.abs() < MIN_BULGE {
                0.0
            } else {
                bulge * flip
            },
        })
        .collect();
    let positions = placed.iter().map(|v| v.position).collect();
    let curve = Curve::Polyline(Polyline {
        vertices: placed,
        closed,
    });
    Some(SnapShape {
        // Zero-length segments have nothing to snap to and no direction.
        pieces: curve
            .segments()
            .into_iter()
            .filter(|piece| match piece {
                Curve::Line(l) => l.length() > 1e-12,
                _ => true,
            })
            .collect(),
        exact: true,
        vertices: positions,
        ends: Vec::new(),
    })
}

/// The flattened outline of `entity`: straight pieces, and the two ends of each open path.
fn sampled(doc: &CadDocument, entity: &EntityType, xf: &Xform, tol: f64) -> Vec<SnapShape> {
    flatten(doc, entity, xf, tol)
        .into_paths()
        .into_iter()
        .filter(|path| !path.points.is_empty())
        .map(|path| {
            let n = path.points.len();
            let count = if path.closed && n > 2 { n } else { n - 1 };
            let pieces = (0..count)
                .map(|i| (path.points[i], path.points[(i + 1) % n]))
                .filter(|(a, b)| (b[0] - a[0]).hypot(b[1] - a[1]) > 1e-12)
                .map(|(start, end)| Curve::Line(Line { start, end }))
                .collect();
            let ends = if path.closed {
                Vec::new()
            } else {
                vec![path.points[0], path.points[n - 1]]
            };
            SnapShape {
                pieces,
                exact: false,
                vertices: Vec::new(),
                ends,
            }
        })
        .collect()
}

fn xy(v: &Vector3) -> [f64; 2] {
    [v.x, v.y]
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use opencadcodec::{Arc as CadArc, LwPolyline, Vector2};

    use super::*;

    fn arc_entity() -> EntityType {
        EntityType::Arc(CadArc::from_center_radius_angles(
            Vector3::new(1.0, 0.0, 0.0),
            1.0,
            0.0,
            PI / 2.0,
        ))
    }

    fn the_arc(shapes: &[SnapShape]) -> Arc {
        match &shapes[0].pieces[..] {
            [Curve::Arc(a)] => *a,
            other => panic!("expected one arc, got {other:?}"),
        }
    }

    fn near(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9
    }

    #[test]
    fn an_arc_scaled_and_moved_stays_an_arc() {
        let doc = CadDocument::new();
        let xf = Xform::scale(2.0, 2.0).then(&Xform::translate(10.0, 0.0));
        let shapes = snap_shapes(&doc, &arc_entity(), &xf, 0.01);
        let arc = the_arc(&shapes);
        assert!(near(arc.centre, [12.0, 0.0]));
        assert!((arc.radius - 2.0).abs() < 1e-12);
        let curve = Curve::Arc(arc);
        assert!(near(curve.point_at(0.0), [14.0, 0.0]));
        assert!(near(curve.point_at(1.0), [12.0, 2.0]));
    }

    #[test]
    fn a_mirrored_arc_still_covers_the_same_quarter() {
        let doc = CadDocument::new();
        let shapes = snap_shapes(&doc, &arc_entity(), &Xform::scale(-1.0, 1.0), 0.01);
        let curve = Curve::Arc(the_arc(&shapes));
        // The quarter from (-2, 0) round to (-1, 1), whichever way it is stored.
        let ends = [curve.point_at(0.0), curve.point_at(1.0)];
        assert!(ends.iter().any(|p| near(*p, [-2.0, 0.0])), "{ends:?}");
        assert!(ends.iter().any(|p| near(*p, [-1.0, 1.0])), "{ends:?}");
        assert!(near(
            curve.point_at(0.5),
            [-1.0 - 0.5f64.sqrt(), 0.5f64.sqrt()]
        ));
    }

    #[test]
    fn a_stretched_arc_is_sampled() {
        let doc = CadDocument::new();
        let shapes = snap_shapes(&doc, &arc_entity(), &Xform::scale(2.0, 1.0), 0.01);
        assert!(!shapes[0].exact);
        assert!(shapes[0].ends.iter().any(|p| near(*p, [4.0, 0.0])));
    }

    #[test]
    fn a_bulged_polyline_keeps_its_arc_when_mirrored() {
        let doc = CadDocument::new();
        let mut p = LwPolyline::new();
        p.add_point(Vector2::new(0.0, 0.0));
        p.add_point(Vector2::new(2.0, 0.0));
        // A half turn below the chord (counter-clockwise from (0,0) to (2,0)).
        p.vertices[0].bulge = 1.0;
        let shapes = snap_shapes(
            &doc,
            &EntityType::LwPolyline(p),
            &Xform::scale(1.0, -1.0),
            0.01,
        );
        let Curve::Arc(arc) = shapes[0].pieces[0] else {
            panic!("expected an arc");
        };
        // Mirrored in y, the half turn is above the chord.
        assert!(near(Curve::Arc(arc).point_at(0.5), [1.0, 1.0]));
        assert_eq!(shapes[0].vertices.len(), 2);
    }
}

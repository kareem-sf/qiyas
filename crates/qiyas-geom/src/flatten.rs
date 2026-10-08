use std::f64::consts::TAU;

use opencadcodec::entities::{BoundaryEdge, EntityType, Insert};
use opencadcodec::{CadDocument, Vector3};

use crate::path::same_point;
use crate::{Path, Xform};

/// Deepest block nesting followed; deeper inserts (or cycles) are skipped.
const MAX_DEPTH: usize = 32;
/// Upper bound on segments per curve, so a zero or tiny `tol` cannot exhaust memory.
const MAX_SEGMENTS: usize = 1 << 16;
/// Most block instances plus paths one `flatten` call produces; past it, output stops.
/// Keeps a block that inserts itself, or a huge nested array, from hanging the app.
const MAX_WORK: usize = 1_000_000;
/// Bulges smaller than this are straight segments.
const MIN_BULGE: f64 = 1e-12;

/// Appends the 2D outline of `entity`, mapped through `xf`, to `out`.
/// Curves are sampled so the chord error stays within `tol` after `xf` is applied.
pub fn flatten(doc: &CadDocument, entity: &EntityType, xf: &Xform, tol: f64, out: &mut Vec<Path>) {
    let mut budget = MAX_WORK;
    flatten_at(doc, entity, xf, tol, out, 0, &mut budget);
}

fn flatten_at(
    doc: &CadDocument,
    entity: &EntityType,
    xf: &Xform,
    tol: f64,
    out: &mut Vec<Path>,
    depth: usize,
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    // Sample in entity coordinates with the tolerance shrunk by the transform's largest stretch.
    let local_tol = tol / max_stretch(xf);
    let (points, closed) = match entity {
        EntityType::Line(e) => (vec![xy(&e.start), xy(&e.end)], false),
        EntityType::LwPolyline(e) => {
            let vertices: Vec<_> = e
                .vertices
                .iter()
                .map(|v| ([v.location.x, v.location.y], v.bulge))
                .collect();
            (bulge_points(&vertices, e.is_closed, local_tol), e.is_closed)
        }
        EntityType::Polyline2D(e) => {
            let vertices: Vec<_> = e
                .vertices
                .iter()
                .map(|v| (xy(&v.location), v.bulge))
                .collect();
            (
                bulge_points(&vertices, e.is_closed(), local_tol),
                e.is_closed(),
            )
        }
        EntityType::Polyline(e) => (
            e.vertices.iter().map(|v| xy(&v.location)).collect(),
            e.is_closed(),
        ),
        EntityType::Polyline3D(e) => (
            e.vertices.iter().map(|v| xy(&v.position)).collect(),
            e.is_closed(),
        ),
        EntityType::Circle(e) => (
            arc_points(xy(&e.center), e.radius, 0.0, TAU, local_tol),
            true,
        ),
        EntityType::Arc(e) => (
            arc_points(
                xy(&e.center),
                e.radius,
                e.start_angle,
                e.end_angle,
                local_tol,
            ),
            false,
        ),
        EntityType::Ellipse(e) => (
            ellipse_points(
                xy(&e.center),
                xy(&e.major_axis),
                e.minor_axis_ratio,
                e.start_parameter,
                e.end_parameter,
                local_tol,
            ),
            sweep(e.start_parameter, e.end_parameter) == TAU,
        ),
        EntityType::Spline(e) => {
            let defining = if e.fit_points.len() >= 2 {
                &e.fit_points
            } else {
                &e.control_points
            };
            (defining.iter().map(xy).collect(), e.flags.closed)
        }
        EntityType::Hatch(e) => {
            for path in &e.paths {
                let mut ring = Vec::new();
                for edge in &path.edges {
                    extend_ring(&mut ring, edge_points(edge, local_tol));
                }
                push(out, xf, ring, true, budget);
            }
            return;
        }
        EntityType::Insert(e) => {
            if depth >= MAX_DEPTH {
                return;
            }
            for instance in insert_instances(doc, e, xf) {
                if *budget == 0 {
                    return;
                }
                *budget -= 1;
                for child in doc.entities_in_block(&e.block_name) {
                    flatten_at(doc, child, &instance, tol, out, depth + 1, budget);
                }
            }
            return;
        }
        EntityType::Text(e) => (
            text_box(&e.insertion_point, e.height, e.rotation, &e.value),
            true,
        ),
        EntityType::MText(e) => (
            text_box(&e.insertion_point, e.height, e.rotation, &e.value),
            true,
        ),
        _ => return,
    };
    push(out, xf, points, closed, budget);
}

/// Block-to-world transform of each instance of `e` (one per array cell), followed by `xf`.
/// Nothing when the block is missing. Lazy, so a caller can stop early on a huge array.
pub fn insert_instances<'a>(
    doc: &CadDocument,
    e: &'a Insert,
    xf: &'a Xform,
) -> impl Iterator<Item = Xform> + 'a {
    let base = doc.block_records.get(&e.block_name).map(|block| {
        Xform::translate(-block.base_point.x, -block.base_point.y)
            .then(&Xform::scale(e.x_scale(), e.y_scale()))
    });
    base.into_iter().flat_map(move |base| {
        (0..e.column_count.max(1)).flat_map(move |col| {
            (0..e.row_count.max(1)).map(move |row| {
                // Array offsets sit in the insert's own axes, so they rotate with it.
                let offset = Xform::translate(
                    f64::from(col) * e.column_spacing,
                    f64::from(row) * e.row_spacing,
                );
                base.then(&offset)
                    .then(&Xform::rotate(e.rotation))
                    .then(&Xform::translate(e.insert_point.x, e.insert_point.y))
                    .then(xf)
            })
        })
    })
}

/// Points on a circular arc, counter-clockwise from `a0` to `a1` (radians), ends included.
/// Equal angles give the full circle. Chord error stays within `tol`.
pub fn arc_points(center: [f64; 2], r: f64, a0: f64, a1: f64, tol: f64) -> Vec<[f64; 2]> {
    let sweep = sweep(a0, a1);
    let n = segment_count(r, sweep, tol);
    (0..=n)
        .map(|i| {
            let a = a0 + sweep * i as f64 / n as f64;
            [center[0] + r * a.cos(), center[1] + r * a.sin()]
        })
        .collect()
}

/// Counter-clockwise sweep from `a0` to `a1` in `(0, TAU]`; equal angles mean a full turn.
fn sweep(a0: f64, a1: f64) -> f64 {
    let s = (a1 - a0).rem_euclid(TAU);
    if s < 1e-12 {
        TAU
    } else {
        s
    }
}

/// Segments needed so a chord of an arc of radius `r` stays within `tol` of it.
fn segment_count(r: f64, sweep: f64, tol: f64) -> usize {
    let step = 2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos();
    ((sweep / step).ceil() as usize).clamp(1, MAX_SEGMENTS)
}

/// Points on an ellipse from parameter `t0` to `t1`, counter-clockwise, ends included.
fn ellipse_points(
    center: [f64; 2],
    major: [f64; 2],
    ratio: f64,
    t0: f64,
    t1: f64,
    tol: f64,
) -> Vec<[f64; 2]> {
    let sweep = sweep(t0, t1);
    let n = segment_count(major[0].hypot(major[1]), sweep, tol);
    let minor = [-major[1] * ratio, major[0] * ratio];
    (0..=n)
        .map(|i| {
            let (s, c) = (t0 + sweep * i as f64 / n as f64).sin_cos();
            [
                center[0] + major[0] * c + minor[0] * s,
                center[1] + major[1] * c + minor[1] * s,
            ]
        })
        .collect()
}

/// A polyline whose segments may be bulged arcs. Each vertex carries the bulge of the
/// segment that starts at it. A closed result does not repeat the first point.
fn bulge_points(vertices: &[([f64; 2], f64)], closed: bool, tol: f64) -> Vec<[f64; 2]> {
    let n = vertices.len();
    if n == 0 {
        return Vec::new();
    }
    let mut points = vec![vertices[0].0];
    let segments = if closed { n } else { n - 1 };
    for i in 0..segments {
        let (p0, bulge) = vertices[i];
        let p1 = vertices[(i + 1) % n].0;
        if bulge.abs() >= MIN_BULGE {
            points.extend(bulge_interior(p0, p1, bulge, tol));
        }
        points.push(p1);
    }
    if closed {
        points.pop();
    }
    points
}

/// The sampled points strictly between `p0` and `p1` on the arc with this bulge.
/// Included angle is `4·atan(b)`; positive bulges turn counter-clockwise.
fn bulge_interior(p0: [f64; 2], p1: [f64; 2], b: f64, tol: f64) -> Vec<[f64; 2]> {
    let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
    let d = dx.hypot(dy);
    if d == 0.0 {
        return Vec::new();
    }
    let r = d * (1.0 + b * b) / (4.0 * b.abs());
    // Signed distance from the chord midpoint to the centre, along the chord's left normal.
    let h = d * (1.0 - b * b) / (4.0 * b);
    let c = [
        (p0[0] + p1[0]) / 2.0 - dy / d * h,
        (p0[1] + p1[1]) / 2.0 + dx / d * h,
    ];
    let a0 = (p0[1] - c[1]).atan2(p0[0] - c[0]);
    let a1 = (p1[1] - c[1]).atan2(p1[0] - c[0]);
    let mut arc = if b > 0.0 {
        arc_points(c, r, a0, a1, tol)
    } else {
        let mut arc = arc_points(c, r, a1, a0, tol);
        arc.reverse();
        arc
    };
    arc.pop();
    arc.remove(0);
    arc
}

/// Points of one hatch boundary edge, in edge order.
fn edge_points(edge: &BoundaryEdge, tol: f64) -> Vec<[f64; 2]> {
    match edge {
        BoundaryEdge::Line(l) => vec![[l.start.x, l.start.y], [l.end.x, l.end.y]],
        BoundaryEdge::CircularArc(a) => {
            let c = [a.center.x, a.center.y];
            if a.counter_clockwise {
                arc_points(c, a.radius, a.start_angle, a.end_angle, tol)
            } else {
                // Clockwise edges store their angles mirrored: walk -start to -end clockwise.
                let mut points = arc_points(c, a.radius, -a.end_angle, -a.start_angle, tol);
                points.reverse();
                points
            }
        }
        BoundaryEdge::EllipticArc(a) => {
            let (c, major) = (
                [a.center.x, a.center.y],
                [a.major_axis_endpoint.x, a.major_axis_endpoint.y],
            );
            let ratio = a.minor_axis_ratio;
            if a.counter_clockwise {
                ellipse_points(c, major, ratio, a.start_angle, a.end_angle, tol)
            } else {
                let mut points = ellipse_points(c, major, ratio, -a.end_angle, -a.start_angle, tol);
                points.reverse();
                points
            }
        }
        BoundaryEdge::Spline(s) => {
            if s.fit_points.len() >= 2 {
                s.fit_points.iter().map(|p| [p.x, p.y]).collect()
            } else {
                s.control_points.iter().map(xy).collect()
            }
        }
        BoundaryEdge::Polyline(p) => {
            // Polyline edge vertices are (x, y, bulge).
            let vertices: Vec<_> = p.vertices.iter().map(|v| ([v.x, v.y], v.z)).collect();
            bulge_points(&vertices, p.is_closed, tol)
        }
    }
}

/// Appends `points` to `ring`, skipping a point that repeats the ring's last one.
fn extend_ring(ring: &mut Vec<[f64; 2]>, points: Vec<[f64; 2]>) {
    for p in points {
        if ring.last().is_none_or(|&last| !same_point(last, p)) {
            ring.push(p);
        }
    }
}

/// A closed rectangle standing in for a line of text, for picking.
fn text_box(insertion: &Vector3, height: f64, rotation: f64, value: &str) -> Vec<[f64; 2]> {
    let width = value.chars().count() as f64 * height * 0.6;
    let place = Xform::rotate(rotation).then(&Xform::translate(insertion.x, insertion.y));
    [[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]]
        .into_iter()
        .map(|p| place.apply(p))
        .collect()
}

/// Maps `points` through `xf` and appends them as a path, spending one unit of `budget`;
/// a closed path drops a final point that repeats its first.
fn push(
    out: &mut Vec<Path>,
    xf: &Xform,
    mut points: Vec<[f64; 2]>,
    closed: bool,
    budget: &mut usize,
) {
    if points.is_empty() || *budget == 0 {
        return;
    }
    *budget -= 1;
    if closed && points.len() > 1 && same_point(points[0], points[points.len() - 1]) {
        points.pop();
    }
    out.push(Path {
        points: points.into_iter().map(|p| xf.apply(p)).collect(),
        closed,
    });
}

/// The largest factor by which `xf` stretches a unit length along either axis.
fn max_stretch(xf: &Xform) -> f64 {
    let m = &xf.m;
    m[0][0].hypot(m[1][0]).max(m[0][1].hypot(m[1][1]))
}

fn xy(v: &Vector3) -> [f64; 2] {
    [v.x, v.y]
}

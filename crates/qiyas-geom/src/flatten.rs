use std::collections::HashMap;
use std::f64::consts::TAU;

use opencadcodec::entities::{
    AttributeEntity, BoundaryEdge, EntityCommon, EntityType, HorizontalAlignment, Insert, MText,
    Text, TextHorizontalAlignment, TextVerticalAlignment, VerticalAlignment,
};
use opencadcodec::types::Matrix3;
use opencadcodec::{CadDocument, Color, Vector3};

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

/// One flattened path with what it is drawn with. `layer` and `color` are resolved the AutoCAD
/// way inside blocks: an entity on layer "0" takes its insert's layer, ByLayer takes that
/// layer's colour, ByBlock takes the insert's colour (ByBlock stays only at the top level).
#[derive(Debug, Clone, PartialEq)]
pub struct FlatPath {
    pub path: Path,
    pub layer: String,
    pub color: Color,
    /// Handles of the block references the path sits inside, outermost first; empty at the top.
    pub chain: Vec<u64>,
    /// Handle of the entity the path comes from: the innermost one, or the insert itself for an
    /// attribute's text box.
    pub entity: u64,
    /// Name of the xref the path comes from; always `None` for now.
    pub xref: Option<String>,
    /// For a text's box (picked, not drawn): what the text draws.
    pub text: Option<Box<FlatText>>,
}

/// What a text (TEXT, MTEXT, or an attribute) draws, for the text renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatText {
    /// A TEXT's or attribute's string, or an MTEXT's with its formatting codes.
    pub value: String,
    /// Text style name.
    pub style: String,
    /// Text height in entity coordinates; 0 when the entity gives none (the style's then).
    pub height: f64,
    pub layout: TextLayout,
    /// Entity (object) coordinates to world: the extrusion normal, then the block references.
    pub xf: Xform,
}

/// Where a text sits and how it is aligned, in entity coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextLayout {
    /// TEXT and attributes: one line. When the alignment is not left and baseline, `alignment`
    /// (if any) is the point aligned to; aligned and fit text runs from `insertion` to it.
    Line {
        insertion: [f64; 2],
        alignment: Option<[f64; 2]>,
        rotation: f64,
        /// 0 when the entity gives none (the style's then).
        width_factor: f64,
        horizontal: HAlign,
        vertical: VAlign,
        /// Generation flag 2: drawn mirrored in X.
        backward: bool,
        /// Generation flag 4: drawn mirrored in Y.
        upside_down: bool,
    },
    /// MTEXT: lines wrapped at `width` (0: no wrapping), placed by `attachment` (1 top left to
    /// 9 bottom right, as in DXF), `line_spacing` a factor of the default spacing.
    Block {
        insertion: [f64; 2],
        rotation: f64,
        width: f64,
        attachment: u8,
        line_spacing: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
    Aligned,
    Middle,
    Fit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Baseline,
    Bottom,
    Middle,
    Top,
}

/// What `flatten` produced. `truncated` is true when block content was left out: nesting deeper
/// than 32 (or a block inserting itself), or more than a million instances and paths.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlattenOutcome {
    pub paths: Vec<FlatPath>,
    pub truncated: bool,
}

impl FlattenOutcome {
    /// The bare paths, for measuring.
    pub fn into_paths(self) -> Vec<Path> {
        self.paths.into_iter().map(|f| f.path).collect()
    }
}

/// The 2D outline of `entity`, mapped through `xf`. Curves are sampled so the chord error stays
/// within `tol` after `xf` is applied. Entities in object coordinates (arcs, circles, polylines,
/// hatches, text, inserts) are mapped through their extrusion normal first, so a mirrored block
/// (normal 0,0,-1) lands where it is drawn. Invisible entities inside blocks are left out.
pub fn flatten(doc: &CadDocument, entity: &EntityType, xf: &Xform, tol: f64) -> FlattenOutcome {
    let mut walk = Walk {
        doc,
        tol,
        out: Vec::new(),
        budget: MAX_WORK,
        truncated: false,
        chain: Vec::new(),
        layer_colors: HashMap::new(),
    };
    let style = walk.style(entity.common(), None);
    walk.entity(entity, xf, &style, 0);
    FlattenOutcome {
        paths: walk.out,
        truncated: walk.truncated,
    }
}

/// The layer and colour an entity is drawn with.
struct Style {
    layer: String,
    color: Color,
}

struct Walk<'a> {
    doc: &'a CadDocument,
    tol: f64,
    out: Vec<FlatPath>,
    /// Block instances plus paths still allowed.
    budget: usize,
    truncated: bool,
    /// Inserts entered so far, outermost first.
    chain: Vec<u64>,
    /// Each layer's colour by its name as written, so a block drawn many times looks it up once.
    layer_colors: HashMap<String, Color>,
}

impl Walk<'_> {
    /// The style of an entity with `common`, inside a block reference drawn with `parent`
    /// (`None` at the top level).
    fn style(&mut self, common: &EntityCommon, parent: Option<&Style>) -> Style {
        let layer = match parent {
            Some(p) if common.layer == "0" => &p.layer,
            _ => &common.layer,
        };
        let color = match common.color {
            Color::ByLayer => match self.layer_colors.get(layer.as_str()) {
                Some(c) => *c,
                None => {
                    let c = self
                        .doc
                        .layers
                        .get(layer)
                        .map_or(Color::ByLayer, |l| l.color);
                    self.layer_colors.insert(layer.clone(), c);
                    c
                }
            },
            Color::ByBlock => parent.map_or(Color::ByBlock, |p| p.color),
            other => other,
        };
        Style {
            layer: layer.clone(),
            color,
        }
    }

    fn entity(&mut self, entity: &EntityType, xf: &Xform, style: &Style, depth: usize) {
        if self.budget == 0 {
            self.truncated = true;
            return;
        }
        let handle = entity.common().handle.value();
        let xf = &match ocs_of(entity) {
            Some(ocs) => ocs.then(xf),
            None => *xf,
        };
        // Sample in entity coordinates with the tolerance shrunk by the transform's largest stretch.
        let local_tol = self.tol / max_stretch(xf);
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
                    minor_axis(&e.normal, &e.major_axis, e.minor_axis_ratio),
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
                    self.push(ring, true, xf, style, handle);
                }
                return;
            }
            EntityType::Insert(e) => {
                self.insert(e, xf, style, depth);
                return;
            }
            EntityType::Text(e) => {
                let points = text_box(&e.insertion_point, e.height, e.rotation, &e.value);
                let text = line_text(e, xf);
                self.push_text(points, xf, style, handle, text);
                return;
            }
            EntityType::MText(e) => {
                let points = text_box(&e.insertion_point, e.height, e.rotation, &e.value);
                let text = block_text(e, xf);
                self.push_text(points, xf, style, handle, text);
                return;
            }
            _ => return,
        };
        self.push(points, closed, xf, style, handle);
    }

    /// The insert's attribute text boxes (picked as the insert), then its block's content.
    fn insert(&mut self, e: &Insert, xf: &Xform, style: &Style, depth: usize) {
        let handle = e.common.handle.value();
        // Attributes sit where the insert sits, not in the block's coordinates.
        for a in &e.attributes {
            if a.common.invisible || a.flags.invisible || a.value.is_empty() {
                continue;
            }
            let at = ocs(&a.normal, a.insertion_point.z).map_or(*xf, |o| o.then(xf));
            let points = text_box(&a.insertion_point, a.height, a.rotation, &a.value);
            let attribute_style = self.style(&a.common, Some(style));
            let text = attribute_text(a, &at);
            self.push_text(points, &at, &attribute_style, handle, text);
        }
        if depth >= MAX_DEPTH {
            self.truncated = true;
            return;
        }
        self.chain.push(handle);
        let doc = self.doc;
        // Styles depend only on the block's entities, not on where each instance is.
        let children: Vec<(&EntityType, Style)> = doc
            .entities_in_block(&e.block_name)
            .filter(|child| !child.common().invisible)
            .map(|child| (child, self.style(child.common(), Some(style))))
            .collect();
        for instance in insert_instances(doc, e, xf) {
            if self.budget == 0 {
                self.truncated = true;
                break;
            }
            self.budget -= 1;
            for (child, child_style) in &children {
                self.entity(child, &instance, child_style, depth + 1);
            }
        }
        self.chain.pop();
    }

    /// Appends a text's closed box, carrying what the text draws.
    fn push_text(
        &mut self,
        points: Vec<[f64; 2]>,
        xf: &Xform,
        style: &Style,
        entity: u64,
        text: FlatText,
    ) {
        self.push_with(points, true, xf, style, entity, Some(Box::new(text)));
    }

    /// Maps `points` through `xf` and appends them as a path, spending one unit of the budget;
    /// a closed path drops a final point that repeats its first.
    fn push(
        &mut self,
        points: Vec<[f64; 2]>,
        closed: bool,
        xf: &Xform,
        style: &Style,
        entity: u64,
    ) {
        self.push_with(points, closed, xf, style, entity, None);
    }

    fn push_with(
        &mut self,
        mut points: Vec<[f64; 2]>,
        closed: bool,
        xf: &Xform,
        style: &Style,
        entity: u64,
        text: Option<Box<FlatText>>,
    ) {
        if points.is_empty() {
            return;
        }
        if self.budget == 0 {
            self.truncated = true;
            return;
        }
        self.budget -= 1;
        if closed && points.len() > 1 && same_point(points[0], points[points.len() - 1]) {
            points.pop();
        }
        self.out.push(FlatPath {
            path: Path {
                points: points.into_iter().map(|p| xf.apply(p)).collect(),
                closed,
            },
            layer: style.layer.clone(),
            color: style.color,
            chain: self.chain.clone(),
            entity,
            xref: None,
            text,
        });
    }
}

/// Block-to-world transform of each instance of `e` (one per array cell), followed by `xf`.
/// Includes the insert's own extrusion normal. Nothing when the block is missing. Lazy, so a
/// caller can stop early on a huge array.
pub fn insert_instances<'a>(
    doc: &CadDocument,
    e: &'a Insert,
    xf: &'a Xform,
) -> impl Iterator<Item = Xform> + 'a {
    let base = doc.block_records.get(&e.block_name).map(|block| {
        Xform::translate(-block.base_point.x, -block.base_point.y)
            .then(&Xform::scale(e.x_scale(), e.y_scale()))
    });
    let to_world = match ocs(&e.normal, e.insert_point.z) {
        Some(o) => o.then(xf),
        None => *xf,
    };
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
                    .then(&to_world)
            })
        })
    })
}

/// The object-to-world transform of an entity stored in object coordinates, projected on plan;
/// `None` when it needs none (world-coordinate entities, and normals along +Z). Inserts apply
/// theirs in `insert_instances`.
fn ocs_of(entity: &EntityType) -> Option<Xform> {
    match entity {
        EntityType::Arc(e) => ocs(&e.normal, e.center.z),
        EntityType::Circle(e) => ocs(&e.normal, e.center.z),
        EntityType::LwPolyline(e) => ocs(&e.normal, e.elevation),
        EntityType::Polyline2D(e) => ocs(&e.normal, e.elevation),
        EntityType::Hatch(e) => ocs(&e.normal, e.elevation),
        EntityType::Text(e) => ocs(&e.normal, e.insertion_point.z),
        _ => None,
    }
}

/// Object (OCS) to world coordinates on plan for an entity with extrusion `normal`, at height `z`
/// in object coordinates: the identity for +Z, a mirror in X for -Z.
pub fn object_to_world(normal: &Vector3, z: f64) -> Xform {
    ocs(normal, z).unwrap_or_else(Xform::identity)
}

/// The plan projection of the AutoCAD arbitrary-axis transform for `normal`, at height `z` in
/// object coordinates. `None` for +Z (identity) or a zero normal.
pub(crate) fn ocs(normal: &Vector3, z: f64) -> Option<Xform> {
    let up = normal.x.abs() < 1e-12 && normal.y.abs() < 1e-12 && normal.z > 0.0;
    if up || normal.length() < 1e-12 {
        return None;
    }
    let m = Matrix3::arbitrary_axis(*normal).m;
    Some(Xform {
        m: [
            [m[0][0], m[0][1], m[0][2] * z],
            [m[1][0], m[1][1], m[1][2] * z],
            [0.0, 0.0, 1.0],
        ],
    })
}

/// An ellipse's minor semi-axis on plan: the normal crossed with the major axis, times the ratio.
fn minor_axis(normal: &Vector3, major: &Vector3, ratio: f64) -> [f64; 2] {
    let n = if normal.length() < 1e-12 {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        normal.normalize()
    };
    let m = n.cross(major);
    [m.x * ratio, m.y * ratio]
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
pub(crate) fn sweep(a0: f64, a1: f64) -> f64 {
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
    minor: [f64; 2],
    t0: f64,
    t1: f64,
    tol: f64,
) -> Vec<[f64; 2]> {
    let sweep = sweep(t0, t1);
    let n = segment_count(major[0].hypot(major[1]), sweep, tol);
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
            let minor = [-major[1] * ratio, major[0] * ratio];
            if a.counter_clockwise {
                ellipse_points(c, major, minor, a.start_angle, a.end_angle, tol)
            } else {
                let mut points = ellipse_points(c, major, minor, -a.end_angle, -a.start_angle, tol);
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

/// What a TEXT draws, placed by `xf` (its normal already in it).
fn line_text(e: &Text, xf: &Xform) -> FlatText {
    use TextHorizontalAlignment as H;
    use TextVerticalAlignment as V;
    FlatText {
        value: e.value.clone(),
        style: e.style.clone(),
        height: e.height,
        layout: TextLayout::Line {
            insertion: xy(&e.insertion_point),
            alignment: e.alignment_point.as_ref().map(xy),
            rotation: e.rotation,
            width_factor: e.width_factor,
            horizontal: match e.horizontal_alignment {
                H::Left => HAlign::Left,
                H::Center => HAlign::Center,
                H::Right => HAlign::Right,
                H::Aligned => HAlign::Aligned,
                H::Middle => HAlign::Middle,
                H::Fit => HAlign::Fit,
            },
            vertical: match e.vertical_alignment {
                V::Baseline => VAlign::Baseline,
                V::Bottom => VAlign::Bottom,
                V::Middle => VAlign::Middle,
                V::Top => VAlign::Top,
            },
            backward: e.generation_flags & 2 != 0,
            upside_down: e.generation_flags & 4 != 0,
        },
        xf: *xf,
    }
}

/// What an attribute draws, placed by `xf` (its normal already in it). A multi-line attribute
/// draws as its MTEXT.
fn attribute_text(a: &AttributeEntity, xf: &Xform) -> FlatText {
    if let Some(mtext) = a.embedded_mtext.as_deref().filter(|_| a.is_multiline) {
        return block_text(mtext, xf);
    }
    use HorizontalAlignment as H;
    use VerticalAlignment as V;
    FlatText {
        value: a.value.clone(),
        style: a.text_style.clone(),
        height: a.height,
        layout: TextLayout::Line {
            insertion: xy(&a.insertion_point),
            alignment: Some(xy(&a.alignment_point)),
            rotation: a.rotation,
            width_factor: a.width_factor,
            horizontal: match a.horizontal_alignment {
                H::Left => HAlign::Left,
                H::Center => HAlign::Center,
                H::Right => HAlign::Right,
                H::Aligned => HAlign::Aligned,
                H::Middle => HAlign::Middle,
                H::Fit => HAlign::Fit,
            },
            vertical: match a.vertical_alignment {
                V::Baseline => VAlign::Baseline,
                V::Bottom => VAlign::Bottom,
                V::Middle => VAlign::Middle,
                V::Top => VAlign::Top,
            },
            backward: a.text_generation_flags & 2 != 0,
            upside_down: a.text_generation_flags & 4 != 0,
        },
        xf: *xf,
    }
}

/// What an MTEXT draws, placed by `xf`.
fn block_text(e: &MText, xf: &Xform) -> FlatText {
    FlatText {
        value: e.value.clone(),
        style: e.style.clone(),
        height: e.height,
        layout: TextLayout::Block {
            insertion: xy(&e.insertion_point),
            rotation: e.rotation,
            width: e.rectangle_width,
            attachment: e.attachment_point as u8,
            line_spacing: e.line_spacing_factor,
        },
        xf: *xf,
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

/// The largest factor by which `xf` stretches a unit length along either axis.
fn max_stretch(xf: &Xform) -> f64 {
    let m = &xf.m;
    m[0][0].hypot(m[1][0]).max(m[0][1].hypot(m[1][1]))
}

fn xy(v: &Vector3) -> [f64; 2] {
    [v.x, v.y]
}

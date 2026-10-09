//! Changing entities already on the sheet on screen (`Command::Modify`): move, copy, rotate,
//! scale, mirror, stretch, align, rectangular and polar arrays, and explode. Each is one undo
//! step. Only the drawing's own entities at the top of the sheet are changed (see
//! `Session::sheet_targets`); a command that can't change every one of them changes nothing.
//!
//! Moves, rotations, scales, mirrors (without keep), stretches and aligns change the entities in
//! place, under their own handles; copies, kept mirrors and arrays add new entities; explode
//! removes the entity and adds its parts.

use std::f64::consts::{PI, TAU};

use opencadcodec::entities::{
    AttachmentPoint, Insert, TextHorizontalAlignment, TextVerticalAlignment,
};
use opencadcodec::types::{Matrix3, Transform};
use opencadcodec::{Circle, EntityType, Handle, Vector3};
use qiyas_geom::{flatten, is_xref_block, Xform};
use serde::{Deserialize, Serialize};

use crate::session::{variant_name, BATCH_TOL};
use crate::{CoreError, Outcome, Session, Target};

/// Most entities one command may add.
const MAX_NEW: usize = 100_000;

/// What `Command::Modify` does, to the entities `targets` (as picked, see `Target`). Points are
/// in drawing coordinates, angles in degrees counter-clockwise.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModifySpec {
    /// Moves them by `delta`.
    Move {
        targets: Vec<Target>,
        delta: [f64; 2],
    },
    /// Adds `count` copies (1 or more), the first `delta` away, each next one `delta` further.
    Copy {
        targets: Vec<Target>,
        delta: [f64; 2],
        count: u32,
    },
    /// Turns them `angle` about `base`; with `copy`, turns a copy and keeps them.
    Rotate {
        targets: Vec<Target>,
        base: [f64; 2],
        angle: f64,
        #[serde(default)]
        copy: bool,
    },
    /// Scales them by `factor` (above 0) about `base`; with `copy`, scales a copy.
    Scale {
        targets: Vec<Target>,
        base: [f64; 2],
        factor: f64,
        #[serde(default)]
        copy: bool,
    },
    /// Mirrors them across the line through `p1` and `p2`; with `keep`, mirrors a copy and keeps
    /// them. Texts still read forwards (AutoCAD's MIRRTEXT 0).
    Mirror {
        targets: Vec<Target>,
        p1: [f64; 2],
        p2: [f64; 2],
        keep: bool,
    },
    /// Moves by `delta` the vertices inside `window` (`[[min_x, min_y], [max_x, max_y]]`) of
    /// lines and polylines; any other entity moves whole when it lies wholly inside the window,
    /// or when its centre or insertion point does (circles, arcs, ellipses, texts, block
    /// references, points).
    Stretch {
        targets: Vec<Target>,
        window: [[f64; 2]; 2],
        delta: [f64; 2],
    },
    /// Moves them so each pair's source point (the first) lands on its destination (the second):
    /// one pair moves; two pairs also turn them so the line between the sources lies along the
    /// line between the destinations (no scaling).
    Align {
        targets: Vec<Target>,
        pairs: Vec<[[f64; 2]; 2]>,
    },
    /// Copies them into `rows` x `cols` (1 or more each, 2 cells at least), `dy` apart up and `dx`
    /// across; they are the first cell.
    ArrayRect {
        targets: Vec<Target>,
        rows: u32,
        cols: u32,
        dy: f64,
        dx: f64,
    },
    /// Copies them round `centre` into `count` items (2 or more, they are the first), each turned
    /// `angle` from the one before (90 with 4 items fills the circle).
    ArrayPolar {
        targets: Vec<Target>,
        centre: [f64; 2],
        count: u32,
        angle: f64,
    },
    /// Breaks block references (one level: a block inside stays a reference; layer "0" and
    /// ByBlock take the reference's layer, colour, linetype and lineweight), polylines and
    /// multilines into their parts. Entities that have no parts are left as they are; nothing to
    /// explode at all is refused.
    Explode { targets: Vec<Target> },
}

impl ModifySpec {
    fn targets(&self) -> &[Target] {
        match self {
            ModifySpec::Move { targets, .. }
            | ModifySpec::Copy { targets, .. }
            | ModifySpec::Rotate { targets, .. }
            | ModifySpec::Scale { targets, .. }
            | ModifySpec::Mirror { targets, .. }
            | ModifySpec::Stretch { targets, .. }
            | ModifySpec::Align { targets, .. }
            | ModifySpec::ArrayRect { targets, .. }
            | ModifySpec::ArrayPolar { targets, .. }
            | ModifySpec::Explode { targets } => targets,
        }
    }
}

/// A transform of the plan and whether it mirrors (then the mirror's line angle, for texts).
#[derive(Clone, Copy)]
struct Placing {
    transform: Transform,
    mirror_line: Option<f64>,
}

impl Placing {
    fn plain(transform: Transform) -> Placing {
        Placing {
            transform,
            mirror_line: None,
        }
    }
}

fn v3([x, y]: [f64; 2]) -> Vector3 {
    Vector3::new(x, y, 0.0)
}

fn translation(d: [f64; 2]) -> Transform {
    Transform::from_translation(v3(d))
}

/// Turns `radians` about `base`.
fn rotation(base: [f64; 2], radians: f64) -> Transform {
    translation([-base[0], -base[1]])
        .then(&Transform::from_rotation(Vector3::UNIT_Z, radians))
        .then(&translation(base))
}

fn invalid(why: impl Into<String>) -> CoreError {
    CoreError::InvalidCommand(why.into())
}

fn finite(values: &[f64]) -> Result<(), CoreError> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(invalid(
            "Points, distances and angles must be finite numbers",
        ))
    }
}

/// Whether `e` can be moved, turned, scaled or mirrored (and with `copying`, copied); why not.
fn check_placeable(e: &EntityType, copying: bool) -> Result<(), CoreError> {
    let h = e.common().handle.value();
    let refuse = |why: &str| Err(invalid(format!("{} {h:X} {why}", variant_name(e))));
    match e {
        // Drawn from a block of its own, which isn't made again yet.
        EntityType::Dimension(_) | EntityType::Table(_) => {
            refuse("can't be changed yet: its block is not drawn again")
        }
        EntityType::Viewport(_)
        | EntityType::Light(_)
        | EntityType::SectionSymbol(_)
        | EntityType::ViewBorder(_)
        | EntityType::Extended(_)
        | EntityType::Unknown(_) => refuse("can't be changed"),
        EntityType::RasterImage(_) | EntityType::Underlay(_) if copying => {
            refuse("can't be copied: attach the file again")
        }
        _ => Ok(()),
    }
}

/// `e` placed by `p`, its stored DWG graphics dropped (they would show it where it was).
fn placed(e: &EntityType, p: &Placing) -> EntityType {
    let mut e = e.clone();
    place(&mut e, p);
    e
}

fn place(e: &mut EntityType, p: &Placing) {
    match p.mirror_line {
        Some(line) => {
            e.apply_mirror(&p.transform);
            keep_readable(e, line);
        }
        None => e.apply_transform(&p.transform),
    }
    e.common_mut().graphic_data = None;
}

/// A text's justification by DXF codes (horizontal 0 left, 1 centre, 2 right, 3 aligned,
/// 4 middle, 5 fit; vertical 0 baseline, 1 bottom, 2 middle, 3 top), its angle and its two points.
struct Justified {
    rotation: f64,
    h: u8,
    v: u8,
    insertion: Vector3,
    alignment: Option<Vector3>,
}

/// A mirrored line of text made to read forwards again, as AutoCAD's MIRRTEXT 0 does. When the
/// mirror line runs more across the text than along it, the text turns half round and its
/// justification swaps left and right; otherwise it swaps top and bottom. Either way it covers
/// about what the mirror image covered.
fn readable(t: Justified, line: f64) -> Justified {
    let d = t.rotation - line;
    let across = d.sin().abs() > d.cos().abs();
    let governs = if t.h == 0 && t.v == 0 {
        t.insertion
    } else {
        t.alignment.unwrap_or(t.insertion)
    };
    let two_points = matches!(t.h, 3 | 5);
    let mut out = Justified { ..t };
    if across {
        out.rotation = (t.rotation + PI).rem_euclid(TAU);
        if two_points {
            // The baseline runs the other way: its ends swap.
            if let Some(a) = t.alignment {
                out.insertion = a;
                out.alignment = Some(t.insertion);
            }
            return out;
        }
        out.h = match t.h {
            0 => 2,
            2 => 0,
            h => h,
        };
    } else {
        if two_points || t.h == 4 {
            return out;
        }
        out.v = match t.v {
            0 | 1 => 3,
            3 => 1,
            v => v,
        };
    }
    out.insertion = governs;
    out.alignment = (out.h != 0 || out.v != 0).then_some(governs);
    out
}

/// Keeps a mirrored text, multiline text, or a block reference's attributes reading forwards.
fn keep_readable(e: &mut EntityType, line: f64) {
    use TextHorizontalAlignment as H;
    use TextVerticalAlignment as V;
    match e {
        EntityType::Text(t) => {
            // The codec slants a mirrored text the other way; a readable one keeps its slant.
            t.oblique_angle = -t.oblique_angle;
            let h = match t.horizontal_alignment {
                H::Left => 0,
                H::Center => 1,
                H::Right => 2,
                H::Aligned => 3,
                H::Middle => 4,
                H::Fit => 5,
            };
            let v = match t.vertical_alignment {
                V::Baseline => 0,
                V::Bottom => 1,
                V::Middle => 2,
                V::Top => 3,
            };
            let out = readable(
                Justified {
                    rotation: t.rotation,
                    h,
                    v,
                    insertion: t.insertion_point,
                    alignment: t.alignment_point,
                },
                line,
            );
            t.rotation = out.rotation;
            t.horizontal_alignment =
                [H::Left, H::Center, H::Right, H::Aligned, H::Middle, H::Fit][usize::from(out.h)];
            t.vertical_alignment = [V::Baseline, V::Bottom, V::Middle, V::Top][usize::from(out.v)];
            t.insertion_point = out.insertion;
            t.alignment_point = out.alignment;
        }
        EntityType::MText(t) => {
            use AttachmentPoint as A;
            let d = t.rotation - line;
            let across = d.sin().abs() > d.cos().abs();
            if across {
                t.rotation = (t.rotation + PI).rem_euclid(TAU);
            }
            let at = t.attachment_point as u8;
            let (row, col) = ((at - 1) / 3, (at - 1) % 3);
            let (row, col) = if across {
                (row, 2 - col)
            } else {
                (2 - row, col)
            };
            t.attachment_point = [
                A::TopLeft,
                A::TopCenter,
                A::TopRight,
                A::MiddleLeft,
                A::MiddleCenter,
                A::MiddleRight,
                A::BottomLeft,
                A::BottomCenter,
                A::BottomRight,
            ][usize::from(row * 3 + col)];
        }
        EntityType::Insert(insert) => {
            for a in &mut insert.attributes {
                let out = readable(
                    Justified {
                        rotation: a.rotation,
                        h: a.horizontal_alignment as u8,
                        v: a.vertical_alignment as u8,
                        insertion: a.insertion_point,
                        alignment: Some(a.alignment_point),
                    },
                    line,
                );
                use opencadcodec::entities::{HorizontalAlignment as AH, VerticalAlignment as AV};
                a.rotation = out.rotation;
                a.horizontal_alignment = [
                    AH::Left,
                    AH::Center,
                    AH::Right,
                    AH::Aligned,
                    AH::Middle,
                    AH::Fit,
                ][usize::from(out.h)];
                a.vertical_alignment =
                    [AV::Baseline, AV::Bottom, AV::Middle, AV::Top][usize::from(out.v)];
                a.insertion_point = out.insertion;
                a.alignment_point = out.alignment.unwrap_or(out.insertion);
            }
        }
        _ => {}
    }
}

/// The world point of `p`, stored in the object coordinates of `normal`.
fn world(normal: Vector3, p: Vector3) -> Vector3 {
    Matrix3::arbitrary_axis(normal) * p
}

/// `delta` (world) in the object coordinates of `normal`.
fn to_object(normal: Vector3, delta: Vector3) -> Vector3 {
    Matrix3::arbitrary_axis(normal).transpose() * delta
}

impl Session {
    /// `Command::Modify`: see `ModifySpec`. `Changed` for a change in place (`Nothing` when a
    /// stretch moved nothing), `Added` with the new handles for copies, arrays and explode.
    pub(crate) fn modify(&mut self, spec: ModifySpec) -> Result<Outcome, CoreError> {
        let handles = self.sheet_targets(spec.targets())?;
        match spec {
            ModifySpec::Move { delta, .. } => {
                finite(&delta)?;
                self.place_all(&handles, Placing::plain(translation(delta)))
            }
            ModifySpec::Copy { delta, count, .. } => {
                finite(&delta)?;
                if count == 0 {
                    return Err(invalid("Make 1 copy or more"));
                }
                let copies = (1..=count)
                    .map(|i| {
                        Placing::plain(translation([
                            delta[0] * f64::from(i),
                            delta[1] * f64::from(i),
                        ]))
                    })
                    .collect();
                self.copy_all(&handles, copies)
            }
            ModifySpec::Rotate {
                base, angle, copy, ..
            } => {
                finite(&[base[0], base[1], angle])?;
                let p = Placing::plain(rotation(base, angle.to_radians()));
                self.place_or_copy(&handles, p, copy)
            }
            ModifySpec::Scale {
                base, factor, copy, ..
            } => {
                finite(&[base[0], base[1], factor])?;
                if factor <= 0.0 {
                    return Err(invalid("The scale factor must be more than 0"));
                }
                let t = Transform::from_scaling_with_origin(
                    Vector3::new(factor, factor, factor),
                    v3(base),
                );
                self.place_or_copy(&handles, Placing::plain(t), copy)
            }
            ModifySpec::Mirror { p1, p2, keep, .. } => {
                finite(&[p1[0], p1[1], p2[0], p2[1]])?;
                if p1 == p2 {
                    return Err(invalid("The mirror line needs two different points"));
                }
                let p = Placing {
                    transform: Transform::from_mirror_line(v3(p1), v3(p2)),
                    mirror_line: Some((p2[1] - p1[1]).atan2(p2[0] - p1[0])),
                };
                self.place_or_copy(&handles, p, keep)
            }
            ModifySpec::Stretch { window, delta, .. } => {
                let [lo, hi] = window;
                finite(&[lo[0], lo[1], hi[0], hi[1], delta[0], delta[1]])?;
                if !(lo[0] < hi[0] && lo[1] < hi[1]) {
                    return Err(invalid("The window's first corner must be its lower left"));
                }
                self.stretch(&handles, window, delta)
            }
            ModifySpec::Align { pairs, .. } => {
                let flat: Vec<f64> = pairs.iter().flatten().flatten().copied().collect();
                finite(&flat)?;
                let t = match pairs.as_slice() {
                    [[s, d]] => translation([d[0] - s[0], d[1] - s[1]]),
                    [[s1, d1], [s2, d2]] => {
                        if s1 == s2 || d1 == d2 {
                            return Err(invalid("Each pair's points must differ from the other's"));
                        }
                        let turn = (d2[1] - d1[1]).atan2(d2[0] - d1[0])
                            - (s2[1] - s1[1]).atan2(s2[0] - s1[0]);
                        translation([d1[0] - s1[0], d1[1] - s1[1]]).then(&rotation(*d1, turn))
                    }
                    _ => return Err(invalid("Align takes one or two pairs of points")),
                };
                self.place_all(&handles, Placing::plain(t))
            }
            ModifySpec::ArrayRect {
                rows, cols, dy, dx, ..
            } => {
                finite(&[dy, dx])?;
                if rows == 0 || cols == 0 || u64::from(rows) * u64::from(cols) < 2 {
                    return Err(invalid("An array needs 2 items or more"));
                }
                self.check_count(&handles, u64::from(rows) * u64::from(cols) - 1)?;
                let copies = (0..rows)
                    .flat_map(|r| (0..cols).map(move |c| (r, c)))
                    .skip(1)
                    .map(|(r, c)| {
                        Placing::plain(translation([dx * f64::from(c), dy * f64::from(r)]))
                    })
                    .collect();
                self.copy_all(&handles, copies)
            }
            ModifySpec::ArrayPolar {
                centre,
                count,
                angle,
                ..
            } => {
                finite(&[centre[0], centre[1], angle])?;
                if count < 2 {
                    return Err(invalid("An array needs 2 items or more"));
                }
                if angle == 0.0 {
                    return Err(invalid("The angle between items must not be 0"));
                }
                self.check_count(&handles, u64::from(count) - 1)?;
                let copies = (1..count)
                    .map(|i| Placing::plain(rotation(centre, (angle * f64::from(i)).to_radians())))
                    .collect();
                self.copy_all(&handles, copies)
            }
            ModifySpec::Explode { .. } => self.explode(&handles),
        }
    }

    /// Refuses `copies` copies of `handles` when that is more than one command may add.
    fn check_count(&self, handles: &[Handle], copies: u64) -> Result<(), CoreError> {
        if copies.saturating_mul(handles.len() as u64) > MAX_NEW as u64 {
            return Err(invalid(format!(
                "That makes more than {MAX_NEW} copies: use fewer items"
            )));
        }
        Ok(())
    }

    fn entity(&self, h: Handle) -> &EntityType {
        self.drawing
            .doc
            .get_entity(h)
            .expect("sheet_targets found it")
    }

    fn place_or_copy(
        &mut self,
        handles: &[Handle],
        p: Placing,
        copy: bool,
    ) -> Result<Outcome, CoreError> {
        if copy {
            self.copy_all(handles, vec![p])
        } else {
            self.place_all(handles, p)
        }
    }

    /// Places every entity `handles` by `p`, in place.
    fn place_all(&mut self, handles: &[Handle], p: Placing) -> Result<Outcome, CoreError> {
        for &h in handles {
            check_placeable(self.entity(h), false)?;
        }
        for &h in handles {
            let e = self.drawing.doc.get_entity_mut(h).expect("checked above");
            place(e, &p);
        }
        Ok(Outcome::Changed)
    }

    /// Adds a copy of every entity `handles` placed by each of `copies`.
    fn copy_all(&mut self, handles: &[Handle], copies: Vec<Placing>) -> Result<Outcome, CoreError> {
        self.check_count(handles, copies.len() as u64)?;
        for &h in handles {
            check_placeable(self.entity(h), true)?;
        }
        let mut added = Vec::with_capacity(handles.len() * copies.len());
        for p in &copies {
            for &h in handles {
                let copy = placed(self.entity(h), p);
                added.push(self.add_fresh(copy)?);
            }
        }
        Ok(Outcome::Added { handles: added })
    }

    /// Adds `e` (a copy of an entity, or a part of one) to the sheet on screen as a new entity:
    /// new handles for it and its attributes, and nothing that ties it to the one it came from
    /// (reactors, extension dictionary, hatch boundary links).
    fn add_fresh(&mut self, mut e: EntityType) -> Result<u64, CoreError> {
        let owner = self.sheet_owner();
        let doc = &mut self.drawing.doc;
        let c = e.common_mut();
        c.handle = Handle::NULL;
        c.reactors.clear();
        c.xdictionary_handle = None;
        c.graphic_data = None;
        if let Some(owner) = owner {
            c.owner_handle = owner;
        }
        match &mut e {
            EntityType::Insert(insert) => {
                let h = doc.allocate_handle();
                insert.common.handle = h;
                insert.seqend_handle = None;
                for a in &mut insert.attributes {
                    a.common.handle = doc.allocate_handle();
                    a.common.owner_handle = h;
                    a.common.reactors.clear();
                    a.common.xdictionary_handle = None;
                }
            }
            EntityType::Hatch(hatch) => {
                hatch.is_associative = false;
                for path in &mut hatch.paths {
                    path.boundary_handles.clear();
                }
            }
            _ => {}
        }
        let h = doc.add_entity(e).map_err(|e| invalid(e.to_string()))?;
        Ok(h.value())
    }

    /// `ModifySpec::Stretch`.
    fn stretch(
        &mut self,
        handles: &[Handle],
        window: [[f64; 2]; 2],
        delta: [f64; 2],
    ) -> Result<Outcome, CoreError> {
        for &h in handles {
            check_placeable(self.entity(h), false)?;
        }
        let [lo, hi] = window;
        let inside = |p: Vector3| lo[0] <= p.x && p.x <= hi[0] && lo[1] <= p.y && p.y <= hi[1];
        let d = v3(delta);
        let whole = Placing::plain(translation(delta));
        let mut moved = false;
        for &h in handles {
            let e = self.entity(h);
            // Vertices inside, by index; `None` for an entity without vertices to stretch.
            let vertices: Option<Vec<usize>> = match e {
                EntityType::Line(l) => Some(
                    [l.start, l.end]
                        .into_iter()
                        .enumerate()
                        .filter(|(_, p)| inside(*p))
                        .map(|(i, _)| i)
                        .collect(),
                ),
                EntityType::LwPolyline(pl) => Some(
                    (0..pl.vertices.len())
                        .filter(|&i| {
                            let at = pl.vertices[i].location;
                            inside(world(pl.normal, Vector3::new(at.x, at.y, pl.elevation)))
                        })
                        .collect(),
                ),
                EntityType::Polyline2D(pl) => Some(
                    (0..pl.vertices.len())
                        .filter(|&i| inside(world(pl.normal, pl.vertices[i].location)))
                        .collect(),
                ),
                EntityType::Polyline(pl) => Some(
                    (0..pl.vertices.len())
                        .filter(|&i| inside(pl.vertices[i].location))
                        .collect(),
                ),
                EntityType::Polyline3D(pl) => Some(
                    (0..pl.vertices.len())
                        .filter(|&i| inside(pl.vertices[i].position))
                        .collect(),
                ),
                _ => None,
            };
            match vertices {
                Some(at) if at.is_empty() => {}
                Some(at) => {
                    moved = true;
                    let e = self.drawing.doc.get_entity_mut(h).expect("found above");
                    e.common_mut().graphic_data = None;
                    match e {
                        EntityType::Line(l) => {
                            for i in at {
                                if i == 0 {
                                    l.start = l.start + d;
                                } else {
                                    l.end = l.end + d;
                                }
                            }
                        }
                        EntityType::LwPolyline(pl) => {
                            let o = to_object(pl.normal, d);
                            for i in at {
                                let v = &mut pl.vertices[i].location;
                                v.x += o.x;
                                v.y += o.y;
                            }
                        }
                        EntityType::Polyline2D(pl) => {
                            let o = to_object(pl.normal, d);
                            for i in at {
                                let v = &mut pl.vertices[i].location;
                                v.x += o.x;
                                v.y += o.y;
                            }
                        }
                        EntityType::Polyline(pl) => {
                            for i in at {
                                pl.vertices[i].location = pl.vertices[i].location + d;
                            }
                        }
                        EntityType::Polyline3D(pl) => {
                            for i in at {
                                pl.vertices[i].position = pl.vertices[i].position + d;
                            }
                        }
                        _ => unreachable!("only entities with vertices"),
                    }
                }
                None => {
                    let anchor = match e {
                        EntityType::Circle(c) => Some(world(c.normal, c.center)),
                        EntityType::Arc(a) => Some(world(a.normal, a.center)),
                        EntityType::Ellipse(el) => Some(el.center),
                        EntityType::Text(t) => Some(world(t.normal, t.insertion_point)),
                        EntityType::MText(t) => Some(t.insertion_point),
                        EntityType::Insert(i) => Some(world(i.normal, i.insert_point)),
                        EntityType::Point(p) => Some(p.location),
                        _ => None,
                    };
                    let goes = match anchor {
                        Some(p) => inside(p),
                        None => self.wholly_inside(e, window),
                    };
                    if goes {
                        moved = true;
                        let e = self.drawing.doc.get_entity_mut(h).expect("found above");
                        place(e, &whole);
                    }
                }
            }
        }
        Ok(if moved {
            Outcome::Changed
        } else {
            Outcome::Nothing
        })
    }

    /// Every point of `e` as drawn lies in `window`.
    fn wholly_inside(&self, e: &EntityType, [lo, hi]: [[f64; 2]; 2]) -> bool {
        let paths = flatten(&self.drawing.doc, e, &Xform::identity(), BATCH_TOL).into_paths();
        !paths.is_empty()
            && paths
                .iter()
                .flat_map(|p| &p.points)
                .all(|p| lo[0] <= p[0] && p[0] <= hi[0] && lo[1] <= p[1] && p[1] <= hi[1])
    }

    /// `ModifySpec::Explode`.
    fn explode(&mut self, handles: &[Handle]) -> Result<Outcome, CoreError> {
        let doc = &self.drawing.doc;
        let mut parts: Vec<(Handle, Vec<EntityType>)> = Vec::new();
        for &h in handles {
            let e = self.entity(h);
            let pieces = match e {
                EntityType::Insert(insert) => {
                    if is_xref_block(doc, &insert.block_name) {
                        return Err(invalid(format!(
                            "Entity {:X} is the xref {}: an xref can't be exploded",
                            h.value(),
                            insert.block_name
                        )));
                    }
                    explode_insert(doc, insert)
                }
                EntityType::LwPolyline(_)
                | EntityType::Polyline2D(_)
                | EntityType::Polyline(_)
                | EntityType::Polyline3D(_)
                | EntityType::MLine(_) => e.explode(),
                _ => Vec::new(),
            };
            if !pieces.is_empty() {
                parts.push((h, pieces));
            }
        }
        if parts.is_empty() {
            return Err(invalid(
                "Nothing picked can explode: only block references, polylines and multilines do",
            ));
        }
        let mut added = Vec::new();
        for (h, pieces) in parts {
            self.drawing.doc.remove_entity_arc(h);
            for piece in pieces {
                added.push(self.add_fresh(piece)?);
            }
        }
        Ok(Outcome::Added { handles: added })
    }
}

/// The entities of `insert`'s block placed where it draws them (every cell of an array), one
/// level deep, layer "0" and ByBlock resolved to the reference's. A circle stays a circle under
/// an even scale (the codec makes every circle an ellipse).
fn explode_insert(doc: &opencadcodec::CadDocument, insert: &Insert) -> Vec<EntityType> {
    let even = (insert.x_scale().abs() - insert.y_scale().abs()).abs() < 1e-10;
    let mut out = Vec::new();
    for child in doc.entities_in_block(&insert.block_name) {
        for mut piece in insert.explode(std::slice::from_ref(child)) {
            if let (true, EntityType::Circle(_), EntityType::Ellipse(el)) = (even, child, &piece) {
                let mut circle = Circle::from_center_radius(Vector3::ZERO, el.major_axis.length());
                circle.common = el.common.clone();
                circle.normal = el.normal;
                circle.center = to_object(el.normal, el.center);
                if let EntityType::Circle(c) = child {
                    circle.thickness = c.thickness;
                }
                piece = EntityType::Circle(circle);
            }
            let c = piece.common_mut();
            if c.linetype.eq_ignore_ascii_case("ByBlock") {
                c.linetype = insert.common.linetype.clone();
            }
            out.push(piece);
        }
    }
    out
}

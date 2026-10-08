//! A picture of the drawing for the model to look at: the sheet's lines on white, with the
//! take-off drawn over them in violet.

use std::collections::HashMap;

use qiyas_core::{Kind, Measurement};
use qiyas_render::Batch;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Largest picture side, in pixels.
pub(crate) const MAX_SIDE: u32 = 1600;
/// The take-off overlay: Qiyas violet `#6E4A9E`.
const VIOLET: [u8; 3] = [0x6E, 0x4A, 0x9E];
/// Radius of the dot marking a counted point or a one-point markup, in pixels.
const DOT: f32 = 4.0;

/// Renders `bounds` (`[[min_x, min_y], [max_x, max_y]]`, drawing coordinates) of `batch` as a
/// PNG `width` pixels wide (1 to 1600), its height following the bounds' aspect (1 to 1600).
/// White background; each segment 1 px in its colour, white drawn black; then every
/// measurement in violet, 2 px: its points as a ring or polyline (dots for counts), or, with no
/// points, the segments of its handles.
pub fn render_png(
    batch: &Batch,
    measurements: &[Measurement],
    bounds: [[f64; 2]; 2],
    width: u32,
) -> Vec<u8> {
    let width = width.clamp(1, MAX_SIDE);
    let [lo, hi] = normalized(bounds);
    let dx = (hi[0] - lo[0]).max(f64::MIN_POSITIVE);
    let dy = (hi[1] - lo[1]).max(f64::MIN_POSITIVE);
    let height = height_for(dx, dy, width);
    let (sx, sy) = (f64::from(width) / dx, f64::from(height) / dy);
    // Pixels have y down; the drawing has y up.
    let to_px = |x: f64, y: f64| (((x - lo[0]) * sx) as f32, ((hi[1] - y) * sy) as f32);
    let segment = |i: usize| {
        let s = &batch.segments[i * 4..i * 4 + 4];
        let [ox, oy] = batch.origin;
        [
            f64::from(s[0]) + ox,
            f64::from(s[1]) + oy,
            f64::from(s[2]) + ox,
            f64::from(s[3]) + oy,
        ]
    };
    let outside = |s: [f64; 4]| {
        s[0].max(s[2]) < lo[0]
            || s[0].min(s[2]) > hi[0]
            || s[1].max(s[3]) < lo[1]
            || s[1].min(s[3]) > hi[1]
    };

    let mut pixmap = Pixmap::new(width, height).expect("1 to 1600 px each way");
    pixmap.fill(Color::WHITE);

    // One path per colour, so a big drawing is a handful of strokes.
    let mut by_color: HashMap<u32, PathBuilder> = HashMap::new();
    for (i, rgb) in batch.colors.iter().enumerate() {
        let s = segment(i);
        if outside(s) {
            continue;
        }
        let rgb = if *rgb == 0xFF_FFFF { 0 } else { *rgb };
        let path = by_color.entry(rgb).or_default();
        let (x0, y0) = to_px(s[0], s[1]);
        let (x1, y1) = to_px(s[2], s[3]);
        path.move_to(x0, y0);
        path.line_to(x1, y1);
    }
    for (rgb, path) in by_color {
        let [_, r, g, b] = rgb.to_be_bytes();
        stroke(&mut pixmap, path, [r, g, b], 1.0);
    }

    let ranges: HashMap<u64, (u32, u32)> = batch
        .ranges
        .iter()
        .map(|&(handle, first, count)| (handle, (first, count)))
        .collect();
    let mut overlay = PathBuilder::new();
    for m in measurements {
        let points: Vec<(f32, f32)> = m.points.iter().map(|p| to_px(p[0], p[1])).collect();
        match points.as_slice() {
            [] => {
                for (first, count) in m.handles.iter().filter_map(|h| ranges.get(h)) {
                    for i in *first as usize..(*first + *count) as usize {
                        let s = segment(i);
                        if !outside(s) {
                            let (x0, y0) = to_px(s[0], s[1]);
                            let (x1, y1) = to_px(s[2], s[3]);
                            overlay.move_to(x0, y0);
                            overlay.line_to(x1, y1);
                        }
                    }
                }
            }
            [only] => overlay.push_circle(only.0, only.1, DOT),
            _ if m.kind == Kind::Count => {
                for (x, y) in &points {
                    overlay.push_circle(*x, *y, DOT);
                }
            }
            [first, rest @ ..] => {
                overlay.move_to(first.0, first.1);
                for (x, y) in rest {
                    overlay.line_to(*x, *y);
                }
                if matches!(m.kind, Kind::Area | Kind::Volume) || m.unit == "markup" {
                    overlay.close();
                }
            }
        }
    }
    stroke(&mut pixmap, overlay, VIOLET, 2.0);

    pixmap
        .encode_png()
        .expect("a pixmap in memory always encodes")
}

/// The height of a picture `width` pixels wide of a `dx` by `dy` box: 1 to 1600 pixels.
pub(crate) fn height_for(dx: f64, dy: f64, width: u32) -> u32 {
    let height = (f64::from(width) * dy / dx).round();
    if height.is_finite() {
        (height as u32).clamp(1, MAX_SIDE)
    } else {
        MAX_SIDE
    }
}

/// The box with its corners in order, whichever way round they were given.
pub(crate) fn normalized([a, b]: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    [
        [a[0].min(b[0]), a[1].min(b[1])],
        [a[0].max(b[0]), a[1].max(b[1])],
    ]
}

fn stroke(pixmap: &mut Pixmap, path: PathBuilder, [r, g, b]: [u8; 3], width: f32) {
    let Some(path) = path.finish() else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, 255);
    paint.anti_alias = true;
    let stroke = Stroke {
        width,
        ..Stroke::default()
    };
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

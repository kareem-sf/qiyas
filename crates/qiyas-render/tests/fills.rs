//! Fills in the batch: solid hatches, 2D solids and wipeouts become triangles under the lines
//! (sections 12 and 13), pattern hatches pattern lines among the segments, and rays and
//! construction lines run across the sheet.

use std::collections::HashSet;

use opencadcodec::entities::{
    BoundaryEdge, BoundaryPath, Hatch, HatchPattern, HatchPatternLine, Point, PolylineEdge, Ray,
    Solid, Wipeout, XLine,
};
use opencadcodec::{CadDocument, Color, EntityType, Line, Vector2, Vector3};
use qiyas_render::{
    add_images, build_batch, section, Batch, ImagePlacement, PickIndex, Range, Visibility,
    FILL_BACKGROUND, NO_CHAIN, PATTERN,
};

const MODEL: &str = "*Model_Space";

fn batch_of(entities: Vec<EntityType>) -> (Batch, Vec<u64>) {
    let mut doc = CadDocument::new();
    let handles = entities
        .into_iter()
        .map(|e| doc.add_entity(e).unwrap().value())
        .collect();
    let vis = Visibility {
        hidden_layers: HashSet::new(),
    };
    (build_batch(&doc, MODEL, &vis, 1e-3), handles)
}

fn ring(points: &[[f64; 2]]) -> BoundaryPath {
    let mut path = BoundaryPath::external();
    path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
        points.iter().map(|p| Vector2::new(p[0], p[1])).collect(),
        true,
    )));
    path
}

/// The square 0..10 with the island 4..6, on red.
fn hatch(pattern: Option<HatchPattern>) -> EntityType {
    let mut h = match pattern {
        Some(p) => Hatch::with_pattern(p),
        None => Hatch::solid(),
    };
    h.add_path(ring(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]));
    h.add_path(ring(&[[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]]));
    h.common.color = Color::Index(1);
    EntityType::Hatch(h)
}

/// Absolute `x0 y0 x1 y1` of segment `i`.
fn segment(b: &Batch, i: usize) -> [f64; 4] {
    let s = &b.segments[4 * i..4 * i + 4];
    [
        f64::from(s[0]) + b.origin[0],
        f64::from(s[1]) + b.origin[1],
        f64::from(s[2]) + b.origin[0],
        f64::from(s[3]) + b.origin[1],
    ]
}

/// Area of fill triangle `i`, in drawing units.
fn triangle_area(b: &Batch, i: usize) -> f64 {
    let t: Vec<f64> = b.fills[6 * i..6 * i + 6]
        .iter()
        .map(|v| f64::from(*v))
        .collect();
    ((t[2] - t[0]) * (t[5] - t[1]) - (t[4] - t[0]) * (t[3] - t[1])).abs() / 2.0
}

#[test]
fn a_solid_hatch_is_triangles_under_its_outline() {
    let (b, handles) = batch_of(vec![hatch(None)]);
    let n = b.fill_colors.len();
    assert!(n >= 8, "{n} triangles");
    assert_eq!(b.fills.len(), 6 * n);
    let area: f64 = (0..n).map(|i| triangle_area(&b, i)).sum();
    assert!((area - 96.0).abs() < 1e-3, "the island stays empty: {area}");
    assert!(b.fill_colors.iter().all(|&c| c == 0xFF0000));
    assert_eq!(
        b.fill_ranges,
        vec![Range {
            top: handles[0],
            entity: handles[0],
            chain: NO_CHAIN,
            first: 0,
            count: n as u32,
            flags: 0,
        }]
    );
    // The outline is still drawn and picked.
    assert!(b
        .ranges
        .iter()
        .any(|r| r.entity == handles[0] && r.flags == 0));
    // Fills count in the bounds; the origin is their middle.
    assert_eq!(b.bounds, [[0.0, 0.0], [10.0, 10.0]]);
}

#[test]
fn solids_fill_and_wipeouts_fill_with_the_background() {
    let v = |x, y| Vector3::new(x, y, 0.0);
    let mut solid = Solid::new(v(0.0, 0.0), v(2.0, 0.0), v(0.0, 1.0), v(2.0, 1.0));
    solid.common.color = Color::Index(3);
    let wipeout = Wipeout::from_corners(v(5.0, 5.0), v(7.0, 6.0));
    let (b, handles) = batch_of(vec![EntityType::Solid(solid), EntityType::Wipeout(wipeout)]);
    let colors_of = |h: u64| -> Vec<u32> {
        b.fill_ranges
            .iter()
            .filter(|r| r.entity == h)
            .flat_map(|r| b.fill_colors[r.first as usize..(r.first + r.count) as usize].to_vec())
            .collect()
    };
    let solid_colors = colors_of(handles[0]);
    assert_eq!(solid_colors, [0x00FF00, 0x00FF00]);
    let wipe = colors_of(handles[1]);
    assert_eq!(wipe.len(), 2);
    assert!(wipe.iter().all(|c| c & FILL_BACKGROUND != 0));
    let area: f64 = (0..b.fill_colors.len()).map(|i| triangle_area(&b, i)).sum();
    assert!((area - 4.0).abs() < 1e-6, "{area}");
}

#[test]
fn pattern_lines_are_flagged_and_left_out_of_boundaries() {
    let mut p = HatchPattern::new("LINE");
    p.add_line(HatchPatternLine {
        angle: 0.0,
        base_point: Vector2::new(0.0, 0.5),
        offset: Vector2::new(0.0, 1.0),
        dash_lengths: Vec::new(),
    });
    let (b, handles) = batch_of(vec![hatch(Some(p))]);
    assert!(b.fills.is_empty());
    let pattern: Vec<&Range> = b.ranges.iter().filter(|r| r.flags & PATTERN != 0).collect();
    assert!(!pattern.is_empty());
    assert!(pattern.iter().all(|r| r.entity == handles[0]));
    let spans: u32 = pattern.iter().map(|r| r.count).sum();
    // Ten lines, the two through the island cut in two.
    assert_eq!(spans, 12);
    // Picking a pattern line picks the hatch.
    let index = PickIndex::new(&b);
    let i = index.pick([1.0, 0.5], 0.01).unwrap();
    assert_eq!(b.range_of(i).unwrap().entity, handles[0]);
}

#[test]
fn rays_and_construction_lines_cross_the_sheet_without_growing_it() {
    let line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 10.0, 10.0, 0.0));
    let ray = EntityType::Ray(Ray::new(
        Vector3::new(5.0, 5.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
    ));
    let xline = EntityType::XLine(XLine::new(
        Vector3::new(5.0, 5.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    ));
    let (b, handles) = batch_of(vec![line, ray, xline]);
    assert_eq!(b.bounds, [[0.0, 0.0], [10.0, 10.0]], "bounds of the rest");
    let segment_of = |h: u64| -> [f64; 4] {
        let r = b.ranges.iter().find(|r| r.entity == h).unwrap();
        assert_eq!(r.count, 1);
        let s = &b.segments[4 * r.first as usize..4 * r.first as usize + 4];
        [
            f64::from(s[0]) + b.origin[0],
            f64::from(s[1]) + b.origin[1],
            f64::from(s[2]) + b.origin[0],
            f64::from(s[3]) + b.origin[1],
        ]
    };
    // The ray from its base point to well past the sheet; the xline both ways.
    let [x0, y0, x1, y1] = segment_of(handles[1]);
    assert_eq!([x0, y0], [5.0, 5.0]);
    assert!(x1 >= 20.0 && (y1 - 5.0).abs() < 1e-6, "{x1} {y1}");
    let [x0, y0, x1, y1] = segment_of(handles[2]);
    assert!((x0 - 5.0).abs() < 1e-6 && (x1 - 5.0).abs() < 1e-6);
    assert!(y0.min(y1) <= -10.0 && y0.max(y1) >= 20.0, "{y0} {y1}");
}

#[test]
fn a_construction_line_alone_still_draws() {
    let xline = EntityType::XLine(XLine::new(
        Vector3::new(100.0, 0.0, 0.0),
        Vector3::new(1.0, 1.0, 0.0).normalize(),
    ));
    let (b, _) = batch_of(vec![xline]);
    assert_eq!(b.ranges.len(), 1);
    assert_eq!(b.segments.len(), 4);
    assert!(b.segments.iter().all(|v| v.is_finite()));
}

#[test]
fn images_keep_the_origin_of_a_sheet_with_fills_only() {
    let (mut b, _) = batch_of(vec![hatch(None)]);
    assert_eq!(b.origin, [5.0, 5.0]);
    add_images(
        &mut b,
        &[ImagePlacement {
            id: 1,
            version: 1,
            corners: [[20.0, 30.0], [40.0, 30.0], [40.0, 20.0], [20.0, 20.0]],
            flags: 0,
        }],
    );
    assert_eq!(b.origin, [5.0, 5.0]);
}

#[test]
fn a_point_is_a_cross_sized_from_the_sheet() {
    let line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 100.0, 0.0, 0.0));
    let point = EntityType::Point(Point::at(Vector3::new(30.0, 20.0, 0.0)));
    let (b, handles) = batch_of(vec![line, point]);
    let range = b.ranges.iter().find(|r| r.entity == handles[1]).unwrap();
    assert_eq!(range.count, 2);
    assert_eq!(range.flags, 0);
    // One percent of the sheet's larger side (100) across, centred on the point.
    let close = |a: [f64; 4], e: [f64; 4]| a.iter().zip(e).all(|(a, e)| (a - e).abs() < 1e-4);
    let first = range.first as usize;
    assert!(close(segment(&b, first), [29.5, 20.0, 30.5, 20.0]));
    assert!(close(segment(&b, first + 1), [30.0, 19.5, 30.0, 20.5]));
    // The point (not its cross) grows the sheet; picking near it finds it.
    assert_eq!(b.bounds, [[0.0, 0.0], [100.0, 20.0]]);
    let index = PickIndex::new(&b);
    let picked = index.pick([30.1, 20.1], 0.5).unwrap();
    assert_eq!(b.range_of(picked).unwrap().entity, handles[1]);
    // A point alone still shows.
    let (alone, _) = batch_of(vec![EntityType::Point(Point::at(Vector3::new(1.0, 1.0, 0.0)))]);
    assert_eq!(alone.segments.len(), 8);
}

fn section_table(bytes: &[u8]) -> Vec<(u32, usize, usize)> {
    let k = u32::from_le_bytes(bytes[56..60].try_into().unwrap()) as usize;
    (0..k)
        .map(|i| {
            let at = 60 + 12 * i;
            let word = |o: usize| u32::from_le_bytes(bytes[at + o..at + o + 4].try_into().unwrap());
            (word(0), word(4) as usize, word(8) as usize)
        })
        .collect()
}

/// The batch `src/viewport/batch.test.ts` decodes from the same bytes: one line (handle 5) and
/// one fill triangle (handle 6, green), plus a wipeout triangle (handle 7) on the background.
#[test]
fn fill_sections_match_the_frontend_fixture() {
    let batch = Batch {
        segments: vec![-1.0, -1.0, 1.0, 1.0],
        colors: vec![0xFF0000],
        ranges: vec![Range {
            top: 5,
            entity: 5,
            chain: NO_CHAIN,
            first: 0,
            count: 1,
            flags: 0,
        }],
        bounds: [[0.0, 0.0], [2.0, 2.0]],
        origin: [1.0, 1.0],
        fills: vec![
            -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, //
            -1.0, -1.0, 1.0, 1.0, -1.0, 1.0,
        ],
        fill_colors: vec![0x00FF00, FILL_BACKGROUND],
        fill_ranges: vec![
            Range {
                top: 6,
                entity: 6,
                chain: NO_CHAIN,
                first: 0,
                count: 1,
                flags: 0,
            },
            Range {
                top: 7,
                entity: 7,
                chain: NO_CHAIN,
                first: 1,
                count: 1,
                flags: 0,
            },
        ],
        ..Batch::default()
    };
    let bytes = batch.to_bytes();
    let table = section_table(&bytes);
    assert_eq!(table.len(), 9);
    assert_eq!((table[7].0, table[7].2), (section::FILLS, 2 * 28));
    assert_eq!((table[8].0, table[8].2), (section::FILL_RANGES, 2 * 32));
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let lines: Vec<&str> = (0..hex.len())
        .step_by(80)
        .map(|i| &hex[i..(i + 80).min(hex.len())])
        .collect();
    assert_eq!(hex, FILLS_FIXTURE, "new fixture:\n{}", lines.join("\n"));
}

/// Also in `src/viewport/batch.test.ts`.
const FILLS_FIXTURE: &str = concat!(
    "42594951020000000000000000000000000000000000000000000000000000400000000000000040",
    "000000000000f03f000000000000f03f0900000003000000a80000002000000004000000c8000000",
    "0000000005000000c80000001000000006000000d80000000400000007000000e000000000000000",
    "08000000e00000001800000009000000f8000000000000000c000000f8000000380000000d000000",
    "300100004000000005000000000000000500000000000000ffffffff000000000100000000000000",
    "000080bf000080bf0000803f0000803f0000ff000000000000000000000000000000000000000000",
    "0000000000000000000080bf000080bf0000803f000080bf0000803f0000803f00ff0000000080bf",
    "000080bf0000803f0000803f000080bf0000803f0000000106000000000000000600000000000000",
    "ffffffff00000000010000000000000007000000000000000700000000000000ffffffff01000000",
    "0100000000000000",
);

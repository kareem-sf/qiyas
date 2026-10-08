use std::collections::HashSet;

use opencadcodec::{CadDocument, Color, EntityType, Line};
use qiyas_core::fixtures::sample_doc;
use qiyas_render::{build_batch, resolve_color, Batch, PickIndex, Visibility};

const MODEL: &str = "*Model_Space";
const WHITE: [u8; 3] = [255, 255, 255];

fn visibility(hidden: &[&str]) -> Visibility {
    Visibility {
        hidden_layers: hidden.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
    }
}

fn sample_batch(hidden: &[&str]) -> Batch {
    build_batch(&sample_doc(), MODEL, &visibility(hidden), 1e-3)
}

/// A fresh document whose model space holds `lines`, each `(x0, y0, x1, y1)`.
fn doc_with_lines(lines: &[(f64, f64, f64, f64)]) -> (CadDocument, Vec<u64>) {
    let mut doc = CadDocument::new();
    let handles = lines
        .iter()
        .map(|&(x0, y0, x1, y1)| {
            let line = Line::from_coords(x0, y0, 0.0, x1, y1, 0.0);
            doc.add_entity(EntityType::Line(line)).unwrap().value()
        })
        .collect();
    (doc, handles)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}
fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn f64_at(bytes: &[u8], at: usize) -> f64 {
    f64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

#[test]
fn sample_batch_counts_and_bounds() {
    let batch = sample_batch(&[]);
    // The Text yields a rectangle, so it counts.
    assert_eq!(batch.ranges.len(), 7);
    assert_eq!(batch.bounds[0][0], 0.0);
    // The arc reaches x = 22.
    assert!(batch.bounds[1][0] >= 22.0 - 1e-9, "{:?}", batch.bounds);
    let [[x0, y0], [x1, y1]] = batch.bounds;
    assert_eq!(batch.origin, [(x0 + x1) / 2.0, (y0 + y1) / 2.0]);
    let segments = batch.segments.len() / 4;
    assert_eq!(batch.colors.len(), segments);
    assert_eq!(batch.handles.len(), segments);
    let counted: u32 = batch.ranges.iter().map(|r| r.2).sum();
    assert_eq!(counted as usize, segments);
}

#[test]
fn hidden_layer_removes_segments() {
    assert_eq!(sample_batch(&["A-DOOR"]).ranges.len(), 4);
}

#[test]
fn hidden_layer_names_ignore_case() {
    assert_eq!(sample_batch(&["a-door"]).ranges.len(), 4);
}

#[test]
fn bylayer_color_comes_from_layer() {
    let doc = sample_doc();
    let wall = doc.layers.get("A-WALL").unwrap();
    assert_eq!(resolve_color(&Color::ByLayer, &wall.color), [255, 0, 0]);
    // In the batch, every segment of the A-WALL line is red.
    let batch = sample_batch(&[]);
    let line = line_handle(&doc);
    let (_, first, count) = *batch.ranges.iter().find(|r| r.0 == line).unwrap();
    for c in &batch.colors[first as usize..(first + count) as usize] {
        assert_eq!(*c, 0xFF0000);
    }
}

#[test]
fn other_colors_resolve() {
    let red = Color::Index(1);
    assert_eq!(resolve_color(&Color::ByBlock, &red), WHITE);
    assert_eq!(resolve_color(&Color::None, &red), WHITE);
    assert_eq!(resolve_color(&Color::Index(7), &red), WHITE);
    assert_eq!(
        resolve_color(
            &Color::Rgb {
                r: 12,
                g: 34,
                b: 56
            },
            &red
        ),
        [12, 34, 56]
    );
}

#[test]
fn bylayer_on_a_missing_layer_is_white() {
    let (mut doc, _) = doc_with_lines(&[]);
    let mut line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    line.common_mut().layer = "NOT-IN-TABLE".to_string();
    line.common_mut().color = Color::ByLayer;
    doc.add_entity(line).unwrap();
    assert!(doc.layers.get("NOT-IN-TABLE").is_none());
    let batch = build_batch(&doc, MODEL, &Visibility::default(), 1e-3);
    assert_eq!(batch.colors, vec![0xFFFFFF]);
}

#[test]
fn bytes_decode_every_section() {
    // The sample has an even segment count; one more line makes it odd, so a section
    // after the handles would be misaligned if the layout were wrong.
    let mut doc = sample_doc();
    let line = Line::from_coords(0.0, 5.0, 0.0, 1.0, 5.0, 0.0);
    doc.add_entity(EntityType::Line(line)).unwrap();
    let batch = build_batch(&doc, MODEL, &Visibility::default(), 1e-3);
    let bytes = batch.to_bytes();
    let n = batch.handles.len();
    let r = batch.ranges.len();
    assert_eq!(n % 2, 1, "segment count should be odd");
    assert_eq!(u32_at(&bytes, 0) as usize, n);
    assert_eq!(u32_at(&bytes, 4) as usize, r);
    let header: Vec<f64> = (0..6).map(|i| f64_at(&bytes, 8 + 8 * i)).collect();
    assert_eq!(
        header,
        [
            batch.bounds[0][0],
            batch.bounds[0][1],
            batch.bounds[1][0],
            batch.bounds[1][1],
            batch.origin[0],
            batch.origin[1]
        ]
    );

    let handles = 56;
    let ranges = handles + 8 * n;
    let segments = ranges + 16 * r;
    let colors = segments + 16 * n;
    assert_eq!(handles % 8, 0);
    assert_eq!(ranges % 8, 0);
    assert_eq!(segments % 4, 0);
    assert_eq!(colors % 4, 0);
    assert_eq!(bytes.len(), colors + 4 * n);

    assert_eq!(u64_at(&bytes, handles), batch.handles[0]);
    let range = (
        u64_at(&bytes, ranges),
        u32_at(&bytes, ranges + 8),
        u32_at(&bytes, ranges + 12),
    );
    assert_eq!(range, batch.ranges[0]);
    let segment: Vec<f32> = (0..4).map(|i| f32_at(&bytes, segments + 4 * i)).collect();
    assert_eq!(segment, batch.segments[..4]);
    assert_eq!(u32_at(&bytes, colors), batch.colors[0]);
}

#[test]
fn far_coordinates_keep_precision() {
    let (doc, _) = doc_with_lines(&[(500_000_000.0, 0.0, 500_000_010.0, 0.0)]);
    let bytes = build_batch(&doc, MODEL, &Visibility::default(), 1e-3).to_bytes();
    assert_eq!(u32_at(&bytes, 0), 1);
    let origin = [f64_at(&bytes, 40), f64_at(&bytes, 48)];
    let segments = 56 + 8 + 16;
    let v: Vec<f64> = (0..4)
        .map(|i| f64::from(f32_at(&bytes, segments + 4 * i)) + origin[i % 2])
        .collect();
    let length = (v[2] - v[0]).hypot(v[3] - v[1]);
    assert!((length - 10.0).abs() < 1e-3, "length {length}");
    assert!((v[0] - 500_000_000.0).abs() < 1e-3, "x0 {}", v[0]);
}

#[test]
fn empty_sheet_is_empty() {
    let batch = build_batch(&sample_doc(), "*NoSuchSheet", &Visibility::default(), 1e-3);
    assert!(batch.ranges.is_empty());
    assert_eq!(batch.bounds, [[0.0; 2]; 2]);
    assert_eq!(batch.origin, [0.0; 2]);
    assert_eq!(batch.to_bytes().len(), 56);
}

#[test]
fn pick_finds_the_line() {
    let doc = sample_doc();
    let index = PickIndex::new(&sample_batch(&[]));
    assert_eq!(index.pick([5.0, 0.05], 0.1), Some(line_handle(&doc)));
    assert_eq!(index.pick([50.0, 50.0], 0.1), None);
}

#[test]
fn pick_measures_to_the_segment_not_its_box() {
    let (doc, h) = doc_with_lines(&[(0.0, 0.0, 10.0, 10.0)]);
    let index = PickIndex::new(&build_batch(&doc, MODEL, &Visibility::default(), 1e-3));
    // Near the diagonal, far from the box corners.
    assert_eq!(index.pick([5.0, 5.05], 0.1), Some(h[0]));
    // Inside the box but far from the diagonal.
    assert_eq!(index.pick([9.0, 1.0], 0.1), None);
}

#[test]
fn pick_returns_the_nearest_entity() {
    let (doc, h) = doc_with_lines(&[(0.0, 0.0, 10.0, 0.0), (0.0, 0.3, 10.0, 0.3)]);
    let index = PickIndex::new(&build_batch(&doc, MODEL, &Visibility::default(), 1e-3));
    assert_eq!(index.pick([5.0, 0.1], 0.5), Some(h[0]));
    assert_eq!(index.pick([5.0, 0.2], 0.5), Some(h[1]));
}

/// Handle of the model-space Line from (0,0) to (10,0).
fn line_handle(doc: &CadDocument) -> u64 {
    doc.entities_in_block(MODEL)
        .find(|e| matches!(e, EntityType::Line(_)))
        .unwrap()
        .common()
        .handle
        .value()
}

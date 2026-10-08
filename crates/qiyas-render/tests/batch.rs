use std::collections::HashSet;

use opencadcodec::EntityType;
use qiyas_core::fixtures::sample_doc;
use qiyas_render::{build_batch, resolve_color, Batch, PickIndex, Visibility};

const MODEL: &str = "*Model_Space";

fn sample_batch(hidden: &[&str]) -> Batch {
    let vis = Visibility {
        hidden_layers: hidden.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
    };
    build_batch(&sample_doc(), MODEL, &vis, 1e-3)
}

#[test]
fn sample_batch_counts_and_bounds() {
    let batch = sample_batch(&[]);
    // The Text yields a rectangle, so it counts.
    assert_eq!(batch.ranges.len(), 7);
    assert_eq!(batch.bounds[0][0], 0.0);
    // The arc reaches x = 22.
    assert!(batch.bounds[1][0] >= 22.0 - 1e-9, "{:?}", batch.bounds);
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
fn bylayer_color_comes_from_layer() {
    let doc = sample_doc();
    let wall = doc.layers.get("A-WALL").unwrap();
    assert_eq!(
        resolve_color(&opencadcodec::Color::ByLayer, &wall.color),
        [255, 0, 0]
    );
    // In the batch, every segment of the A-WALL line is red.
    let batch = sample_batch(&[]);
    let line = line_handle(&doc);
    let (_, first, count) = *batch.ranges.iter().find(|r| r.0 == line).unwrap();
    for c in &batch.colors[first as usize..(first + count) as usize] {
        assert_eq!(*c, 0xFF0000);
    }
}

#[test]
fn bytes_round_trip_header() {
    let batch = sample_batch(&[]);
    let bytes = batch.to_bytes();
    let segment_count = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let range_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(segment_count as usize, batch.segments.len() / 4);
    assert_eq!(range_count as usize, batch.ranges.len());
    let min_x = f64::from_le_bytes(bytes[8..16].try_into().unwrap());
    assert_eq!(min_x, batch.bounds[0][0]);
    let n = segment_count as usize;
    let expected = 8 + 32 + n * 16 + n * 4 + n * 8 + batch.ranges.len() * 16;
    assert_eq!(bytes.len(), expected);
}

#[test]
fn pick_finds_the_line() {
    let doc = sample_doc();
    let index = PickIndex::new(&sample_batch(&[]));
    assert_eq!(index.pick([5.0, 0.05], 0.1), Some(line_handle(&doc)));
    assert_eq!(index.pick([50.0, 50.0], 0.1), None);
}

/// Handle of the model-space Line from (0,0) to (10,0).
fn line_handle(doc: &opencadcodec::CadDocument) -> u64 {
    doc.entities_in_block(MODEL)
        .find(|e| matches!(e, EntityType::Line(_)))
        .unwrap()
        .common()
        .handle
        .value()
}

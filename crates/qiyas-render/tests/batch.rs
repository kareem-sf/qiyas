use std::collections::HashSet;

use opencadcodec::entities::Insert;
use opencadcodec::{BlockRecord, CadDocument, Color, EntityType, Layer, Line, Vector3};
use qiyas_core::fixtures::sample_doc;
use qiyas_render::{
    build_batch, resolve_color, section, Batch, PickIndex, Range, Visibility, MAGIC, NO_CHAIN,
    VERSION,
};

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
    assert_eq!(batch.entity_handles.len(), segments);
    let counted: u32 = batch.ranges.iter().map(|r| r.count).sum();
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
    let r = *batch.ranges.iter().find(|r| r.top == line).unwrap();
    for c in &batch.colors[r.first as usize..(r.first + r.count) as usize] {
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

/// `(kind, offset, length)` of every section in the table of `bytes`.
fn section_table(bytes: &[u8]) -> Vec<(u32, usize, usize)> {
    let k = u32_at(bytes, 56) as usize;
    (0..k)
        .map(|i| {
            let at = 60 + 12 * i;
            (
                u32_at(bytes, at),
                u32_at(bytes, at + 4) as usize,
                u32_at(bytes, at + 8) as usize,
            )
        })
        .collect()
}

/// Offset and length of the section `kind`.
fn section_at(bytes: &[u8], kind: u32) -> (usize, usize) {
    let (_, at, len) = *section_table(bytes)
        .iter()
        .find(|s| s.0 == kind)
        .unwrap_or_else(|| panic!("no section {kind}"));
    (at, len)
}

#[test]
fn bytes_decode_every_section() {
    // The sample has an even segment count; one more line makes it odd, so a section after the
    // colours or segments would be misaligned without the padding.
    let mut doc = sample_doc();
    let line = Line::from_coords(0.0, 5.0, 0.0, 1.0, 5.0, 0.0);
    doc.add_entity(EntityType::Line(line)).unwrap();
    let batch = build_batch(&doc, MODEL, &Visibility::default(), 1e-3);
    let bytes = batch.to_bytes();
    let n = batch.handles.len();
    let r = batch.ranges.len();
    assert_eq!(n % 2, 1, "segment count should be odd");
    assert_eq!(u32_at(&bytes, 0), MAGIC);
    assert_eq!(&bytes[..4], b"BYIQ");
    assert_eq!(u32_at(&bytes, 4), VERSION);
    assert_eq!(VERSION, 2);
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

    let table = section_table(&bytes);
    assert_eq!(
        table.iter().map(|s| s.0).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6]
    );
    for (_, at, len) in &table {
        assert_eq!(at % 8, 0, "{table:?}");
        assert!(at + len <= bytes.len());
    }
    let (handles, len) = section_at(&bytes, section::HANDLES);
    assert_eq!(len, 8 * n);
    assert_eq!(u64_at(&bytes, handles), batch.handles[0]);
    let (entities, len) = section_at(&bytes, section::ENTITY_HANDLES);
    assert_eq!(len, 8 * n);
    // The scaled DOOR's leaf line: top is the insert, the entity is the line in the block.
    let door = batch
        .ranges
        .iter()
        .position(|r| r.chain != NO_CHAIN)
        .unwrap();
    let first = batch.ranges[door].first as usize;
    assert_eq!(
        u64_at(&bytes, entities + 8 * first),
        batch.entity_handles[first]
    );
    assert_ne!(batch.entity_handles[first], batch.handles[first]);

    let (ranges, len) = section_at(&bytes, section::RANGES);
    assert_eq!(len, 32 * r);
    let at = ranges + 32 * door;
    let range = Range {
        top: u64_at(&bytes, at),
        entity: u64_at(&bytes, at + 8),
        chain: u32_at(&bytes, at + 16),
        first: u32_at(&bytes, at + 20),
        count: u32_at(&bytes, at + 24),
    };
    assert_eq!(range, batch.ranges[door]);
    // A top-level range has no chain.
    assert_eq!(u32_at(&bytes, ranges + 16), NO_CHAIN);

    let (chains, len) = section_at(&bytes, section::CHAINS);
    // Three DOORs, one chain each: (u32 1, u64 handle).
    assert_eq!(batch.chains.len(), 3);
    assert_eq!(len, 3 * 12);
    let chain = &batch.chains[range.chain as usize];
    let at = chains + 12 * range.chain as usize;
    assert_eq!(u32_at(&bytes, at), 1);
    assert_eq!(u64_at(&bytes, at + 4), chain[0]);
    assert_eq!(chain[0], range.top);

    let (segments, len) = section_at(&bytes, section::SEGMENTS);
    assert_eq!(len, 16 * n);
    let segment: Vec<f32> = (0..4).map(|i| f32_at(&bytes, segments + 4 * i)).collect();
    assert_eq!(segment, batch.segments[..4]);
    let (colors, len) = section_at(&bytes, section::COLORS);
    assert_eq!(len, 4 * n);
    assert_eq!(u32_at(&bytes, colors), batch.colors[0]);
    assert_eq!(bytes.len(), colors + len);
}

#[test]
fn far_coordinates_keep_precision() {
    let (doc, _) = doc_with_lines(&[(500_000_000.0, 0.0, 500_000_010.0, 0.0)]);
    let bytes = build_batch(&doc, MODEL, &Visibility::default(), 1e-3).to_bytes();
    let (segments, len) = section_at(&bytes, section::SEGMENTS);
    assert_eq!(len, 16);
    let origin = [f64_at(&bytes, 40), f64_at(&bytes, 48)];
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
    let bytes = batch.to_bytes();
    // Header and a table of six empty sections, padded to 8 bytes.
    assert_eq!(bytes.len(), 136);
    assert!(section_table(&bytes).iter().all(|s| s.2 == 0));
}

#[test]
fn hidden_inner_layer_removes_only_those_segments() {
    // A block with a frame on A-FRAME and a leaf on layer 0, inserted on A-DOOR.
    let mut doc = sample_doc();
    let mut frame = Layer::new("A-FRAME");
    frame.color = Color::Index(5);
    doc.layers.add(frame).unwrap();
    let mut block = BlockRecord::new("FRAMED");
    block.handle = doc.allocate_handle();
    let owner = block.handle;
    doc.block_records.add(block).unwrap();
    let mut inner = Vec::new();
    for (layer, x) in [("A-FRAME", 0.0), ("0", 1.0)] {
        let mut e = EntityType::Line(Line::from_coords(x, 0.0, 0.0, x, 1.0, 0.0));
        e.common_mut().layer = layer.to_string();
        e.common_mut().owner_handle = owner;
        inner.push(doc.add_entity(e).unwrap().value());
    }
    let mut insert = EntityType::Insert(Insert::new("FRAMED", Vector3::new(50.0, 0.0, 0.0)));
    insert.common_mut().layer = "A-DOOR".to_string();
    let top = doc.add_entity(insert).unwrap().value();

    let all = build_batch(&doc, MODEL, &Visibility::default(), 1e-3);
    let mine: Vec<&Range> = all.ranges.iter().filter(|r| r.top == top).collect();
    assert_eq!(mine.iter().map(|r| r.entity).collect::<Vec<_>>(), inner);
    // The frame keeps its own layer's colour; the leaf on 0 takes A-DOOR's green.
    assert_eq!(all.colors[mine[0].first as usize], 0x0000FF);
    assert_eq!(all.colors[mine[1].first as usize], 0x00FF00);
    assert_eq!(all.chain_of(mine[0]), [top]);

    let hidden = build_batch(&doc, MODEL, &visibility(&["a-frame"]), 1e-3);
    let left: Vec<u64> = hidden
        .ranges
        .iter()
        .filter(|r| r.top == top)
        .map(|r| r.entity)
        .collect();
    assert_eq!(left, [inner[1]]);
    // Nothing else changed.
    assert_eq!(hidden.ranges.len(), all.ranges.len() - 1);

    // Off in the drawing works the same way.
    doc.layers.get_mut("A-FRAME").unwrap().flags.off = true;
    let off = build_batch(&doc, MODEL, &Visibility::default(), 1e-3);
    assert_eq!(off.ranges.len(), all.ranges.len() - 1);
}

#[test]
fn pick_finds_the_line() {
    let doc = sample_doc();
    let batch = sample_batch(&[]);
    let index = PickIndex::new(&batch);
    let hit = index.pick([5.0, 0.05], 0.1).unwrap();
    assert_eq!(batch.handles[hit], line_handle(&doc));
    assert_eq!(batch.range_of(hit).unwrap().top, line_handle(&doc));
    assert_eq!(index.pick([50.0, 50.0], 0.1), None);
}

#[test]
fn pick_measures_to_the_segment_not_its_box() {
    let (doc, h) = doc_with_lines(&[(0.0, 0.0, 10.0, 10.0)]);
    let batch = build_batch(&doc, MODEL, &Visibility::default(), 1e-3);
    let index = PickIndex::new(&batch);
    // Near the diagonal, far from the box corners.
    assert_eq!(
        index.pick([5.0, 5.05], 0.1).map(|i| batch.handles[i]),
        Some(h[0])
    );
    // Inside the box but far from the diagonal.
    assert_eq!(index.pick([9.0, 1.0], 0.1), None);
}

#[test]
fn pick_returns_the_nearest_entity() {
    let (doc, h) = doc_with_lines(&[(0.0, 0.0, 10.0, 0.0), (0.0, 0.3, 10.0, 0.3)]);
    let batch = build_batch(&doc, MODEL, &Visibility::default(), 1e-3);
    let index = PickIndex::new(&batch);
    assert_eq!(
        index.pick([5.0, 0.1], 0.5).map(|i| batch.handles[i]),
        Some(h[0])
    );
    assert_eq!(
        index.pick([5.0, 0.2], 0.5).map(|i| batch.handles[i]),
        Some(h[1])
    );
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

/// The batch `src/viewport/batch.test.ts` decodes from the same bytes, so both sides agree on
/// layout version 2.
#[test]
fn bytes_match_the_frontend_fixture() {
    let batch = Batch {
        segments: vec![-2.0, -1.5, 2.0, 1.5, 0.0, 0.0, 1.0, 1.0],
        colors: vec![0xFF0000, 0x00FF00],
        handles: vec![7, 8],
        entity_handles: vec![7, 9],
        ranges: vec![
            Range {
                top: 7,
                entity: 7,
                chain: NO_CHAIN,
                first: 0,
                count: 1,
            },
            Range {
                top: 8,
                entity: 9,
                chain: 0,
                first: 1,
                count: 1,
            },
        ],
        chains: vec![vec![8]],
        bounds: [[100.0, 200.0], [104.0, 203.0]],
        origin: [102.0, 201.5],
        truncated: false,
    };
    let hex: String = batch
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hex, FRONTEND_FIXTURE);
}

/// Also in `src/viewport/batch.test.ts`.
const FRONTEND_FIXTURE: &str = concat!(
    "4259495102000000000000000000594000000000000069400000000000005a400000000000606940",
    "00000000008059400000000000306940060000000100000088000000100000000200000098000000",
    "1000000003000000a80000004000000004000000e80000000c00000005000000f800000020000000",
    "06000000180100000800000000000000070000000000000008000000000000000700000000000000",
    "090000000000000007000000000000000700000000000000ffffffff000000000100000000000000",
    "08000000000000000900000000000000000000000100000001000000000000000100000008000000",
    "0000000000000000000000c00000c0bf000000400000c03f00000000000000000000803f0000803f",
    "0000ff0000ff0000",
);

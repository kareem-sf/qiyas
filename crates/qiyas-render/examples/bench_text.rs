//! Times drawing a sheet of 10,000 texts: shaping, layout, glyph quads and the atlas.
//!
//! `cargo run --release -p qiyas-render --example bench_text`

use std::time::Instant;

use opencadcodec::{CadDocument, EntityType, Text, Vector3};
use qiyas_render::{
    build_batch, build_batch_with_text, place_text, TextEngine, TextOptions, Visibility,
};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    // A grid of room labels and door marks, one in four in Arabic, every string different.
    let mut doc = CadDocument::new();
    for i in 0..10_000 {
        let value = match i % 4 {
            0 => format!("ROOM {i}"),
            1 => format!("D{i} 900x2100"),
            2 => format!("غرفة {i}"),
            _ => format!("Bedroom {i} - 12.5 m2"),
        };
        let at = Vector3::new(f64::from(i % 100) * 20.0, f64::from(i / 100) * 5.0, 0.0);
        let text = Text::with_value(value, at).with_height(0.25 + f64::from(i % 3) * 0.25);
        doc.add_entity(EntityType::Text(text)).unwrap();
    }
    let sheet = "*Model_Space";
    let vis = Visibility::default();

    let t = Instant::now();
    let lines = build_batch(&doc, sheet, &vis, 0.05);
    let lines_ms = ms(t);

    let mut engine = TextEngine::new();
    let t = Instant::now();
    let batch = build_batch_with_text(&doc, sheet, &vis, 0.05, &mut engine, TextOptions::default());
    let first_ms = ms(t);

    let t = Instant::now();
    let again = build_batch_with_text(&doc, sheet, &vis, 0.05, &mut engine, TextOptions::default());
    let again_ms = ms(t);

    let mut zoomed = again.clone();
    let t = Instant::now();
    place_text(&mut zoomed, &mut engine, batch.px_per_unit * 8.0);
    let zoom_ms = ms(t);

    println!("texts          {}", batch.texts.len());
    println!("quads          {}", batch.quads.len());
    println!("lines only ms  {lines_ms:.1} (boxes, no text)");
    println!("with text ms   {first_ms:.1} (first build: shaping, layout, quads, atlas)");
    println!("again ms       {again_ms:.1} (glyphs already in the atlas)");
    println!("zoom 8x ms     {zoom_ms:.1} (quads again at another size)");
    println!(
        "atlas pages    {} (version {})",
        engine.atlas().page_count(),
        engine.atlas().version()
    );
    println!("batch bytes    {}", batch.to_bytes().len());
    let _ = lines;
}

//! Times opening a real drawing, building its first batch and picking.
//!
//! `cargo run --release -p qiyas-core --example bench_open -- <drawing.dwg>`
//! Read only: it never writes to the drawing.

use std::path::PathBuf;
use std::time::Instant;

use opencadcodec::EntityType;
use qiyas_core::Session;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let arg = std::env::args()
        .nth(1)
        .expect("usage: bench_open <drawing>");
    let path = PathBuf::from(arg);

    let t = Instant::now();
    let session = Session::open(&path).expect("open");
    let open_ms = ms(t);

    let t = Instant::now();
    let batch = session.batch();
    let batch_ms = ms(t);
    let bytes = batch.to_bytes().len();

    let [[x0, y0], [x1, y1]] = batch.bounds;
    let centre = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
    let t = Instant::now();
    let hits: Vec<_> = (0..20).map(|_| session.pick(centre, 0.01)).collect();
    let pick_ms = ms(t) / 20.0;

    let info = session.info();
    println!("file      {}", path.display());
    println!("open ms   {open_ms:.0}");
    println!("batch ms  {batch_ms:.0}");
    println!("pick ms   {pick_ms:.3} (avg of 20, hit {:?})", hits[0]);
    println!("segments  {}", batch.segments.len() / 4);
    println!("ranges    {}", batch.ranges.len());
    println!("bytes     {bytes}");
    println!("problems  {}", info.problems.len());
    println!("layers    {}", info.layers.len());
    println!("sheets    {}", info.sheets.len());
    let is_insert = |e: &&EntityType| matches!(e, EntityType::Insert(_));
    let inserts = session
        .drawing
        .entities_in(&session.sheet)
        .filter(is_insert);
    let inserts = inserts.count();
    println!("inserts   {inserts} (block references on the first sheet)");
    println!("units     {}", info.units);
    for p in info.problems.iter().take(5) {
        println!("problem   {p}");
    }
}

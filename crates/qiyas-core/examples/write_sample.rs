//! Writes the synthetic test drawing for trying the app by hand.
//!
//! `cargo run -p qiyas-core --example write_sample -- target` writes `target/sample.dxf` and
//! `target/sample.dwg`.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    for ext in ["dxf", "dwg"] {
        println!(
            "{}",
            qiyas_core::fixtures::write_sample(&dir, ext).display()
        );
    }
}

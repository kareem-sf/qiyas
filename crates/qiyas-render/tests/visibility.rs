use std::collections::HashSet;

use opencadcodec::{CadDocument, Layer};
use qiyas_render::{layer_on_in_file, layer_shown, Visibility};

fn doc() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.layers.add(Layer::new("ON")).unwrap();
    let mut off = Layer::new("OFF");
    off.flags.off = true;
    doc.layers.add(off).unwrap();
    let mut frozen = Layer::new("FROZEN");
    frozen.flags.frozen = true;
    doc.layers.add(frozen).unwrap();
    doc
}

fn hiding(names: &[&str]) -> Visibility {
    Visibility {
        hidden_layers: names.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
    }
}

#[test]
fn in_file_means_not_off_and_not_frozen() {
    let doc = doc();
    assert!(layer_on_in_file(&doc, "ON"));
    assert!(layer_on_in_file(&doc, "on"));
    assert!(!layer_on_in_file(&doc, "OFF"));
    assert!(!layer_on_in_file(&doc, "FROZEN"));
    // A layer the drawing doesn't have is on.
    assert!(layer_on_in_file(&doc, "MISSING"));
}

#[test]
fn shown_also_needs_the_user_not_to_have_hidden_it() {
    let doc = doc();
    let vis = hiding(&["On", "missing"]);
    assert!(!layer_shown(&doc, &vis, "ON"));
    assert!(!layer_shown(&doc, &vis, "Missing"));
    assert!(!layer_shown(&doc, &Visibility::default(), "OFF"));
    assert!(layer_shown(&doc, &Visibility::default(), "ON"));
    assert!(layer_shown(&doc, &vis, "0"));
}

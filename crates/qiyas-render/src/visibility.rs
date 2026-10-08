//! The one rule for which layers are drawn, counted and measured.

use opencadcodec::tables::normalize_name;
use opencadcodec::CadDocument;

use crate::Visibility;

/// True unless the drawing itself has the layer off or frozen. A layer the drawing doesn't have
/// counts as on. The user's hidden layers are not looked at.
pub fn layer_on_in_file(doc: &CadDocument, layer: &str) -> bool {
    !doc.layers
        .get(layer)
        .is_some_and(|l| l.flags.off || l.flags.frozen)
}

/// True when the layer is on in the file and the user has not hidden it. Names match
/// case-insensitively, as in DXF.
pub fn layer_shown(doc: &CadDocument, vis: &Visibility, layer: &str) -> bool {
    let key = normalize_name(layer);
    let hidden = vis.hidden_layers.iter().any(|n| normalize_name(n) == key);
    !hidden && layer_on_in_file(doc, layer)
}

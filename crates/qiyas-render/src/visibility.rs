//! The one rule for which layers are drawn, counted and measured.

use opencadcodec::tables::normalize_name;
use opencadcodec::CadDocument;
use qiyas_geom::{find_layer, XrefDocs};

use crate::Visibility;

/// True unless the drawing itself has the layer off or frozen. A layer the drawing doesn't have
/// counts as on. The user's hidden layers are not looked at.
pub fn layer_on_in_file(doc: &CadDocument, layer: &str) -> bool {
    layer_on_in_file_with(doc, &XrefDocs::new(), layer)
}

/// True when the layer is on in the file and the user has not hidden it. Names match
/// case-insensitively, as in DXF.
pub fn layer_shown(doc: &CadDocument, vis: &Visibility, layer: &str) -> bool {
    layer_shown_with(doc, &XrefDocs::new(), vis, layer)
}

/// `layer_on_in_file`, where an xref layer `XREF|LAYER` the host does not keep is on unless that
/// xref has `LAYER` off or frozen.
pub fn layer_on_in_file_with(doc: &CadDocument, xrefs: &XrefDocs, layer: &str) -> bool {
    !find_layer(doc, xrefs, layer).is_some_and(|l| l.flags.off || l.flags.frozen)
}

/// `layer_shown`, with xref layers as `layer_on_in_file_with` has them. The user hides an xref
/// layer by its full `XREF|LAYER` name.
pub fn layer_shown_with(
    doc: &CadDocument,
    xrefs: &XrefDocs,
    vis: &Visibility,
    layer: &str,
) -> bool {
    let key = normalize_name(layer);
    let hidden = vis.hidden_layers.iter().any(|n| normalize_name(n) == key);
    !hidden && layer_on_in_file_with(doc, xrefs, layer)
}

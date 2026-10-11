//! What the batch doesn't carry about a drawn entity and a plot needs: its lineweight (ByLayer
//! and ByBlock resolved as AutoCAD does inside blocks), whether its layer plots, and its layer's
//! plot style name (for an STB).

use std::collections::HashMap;

use opencadcodec::tables::normalize_name;
use opencadcodec::{CadDocument, Handle, LineWeight};
use qiyas_geom::{find_layer, is_xref_block, XrefDocs};

/// LWDEFAULT: what "Default" plots at, in millimetres.
pub const DEFAULT_LINEWEIGHT_MM: f64 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// Millimetres.
    pub weight_mm: f64,
    /// Its layer plots.
    pub plot: bool,
    /// Its layer's plot style (STB), "" for none.
    pub style: String,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            weight_mm: DEFAULT_LINEWEIGHT_MM,
            plot: true,
            style: String::new(),
        }
    }
}

fn millimetres(w: &LineWeight) -> Option<f64> {
    match w {
        LineWeight::Value(v) if *v >= 0 => Some(f64::from(*v) / 100.0),
        _ => None,
    }
}

pub struct Looks<'a> {
    doc: &'a CadDocument,
    xrefs: XrefDocs<'a>,
    memo: HashMap<(u64, Vec<u64>), Look>,
}

impl<'a> Looks<'a> {
    pub fn new(doc: &'a CadDocument, xrefs: XrefDocs<'a>) -> Looks<'a> {
        Looks {
            doc,
            xrefs,
            memo: HashMap::new(),
        }
    }

    /// The look of `entity` drawn inside the references `chain` (outermost first).
    pub fn of(&mut self, entity: u64, chain: &[u64]) -> Look {
        let key = (entity, chain.to_vec());
        if let Some(look) = self.memo.get(&key) {
            return look.clone();
        }
        let look = self.find(entity, chain).unwrap_or_default();
        self.memo.insert(key, look.clone());
        look
    }

    fn weight(&self, own: &LineWeight, layer: &str, outer: Option<f64>) -> f64 {
        match own {
            LineWeight::ByLayer => find_layer(self.doc, &self.xrefs, layer)
                .and_then(|l| millimetres(&l.line_weight))
                .unwrap_or(DEFAULT_LINEWEIGHT_MM),
            LineWeight::ByBlock => outer.unwrap_or(DEFAULT_LINEWEIGHT_MM),
            w => millimetres(w).unwrap_or(DEFAULT_LINEWEIGHT_MM),
        }
    }

    fn find(&self, entity: u64, chain: &[u64]) -> Option<Look> {
        let mut doc = self.doc;
        let mut xref: Option<&str> = None;
        let mut outer_layer: Option<String> = None;
        let mut outer_weight: Option<f64> = None;
        let drawn = |own: &str, outer: &Option<String>, xref: Option<&str>| match (outer, xref) {
            (Some(o), _) if own == "0" => o.clone(),
            (_, Some(x)) => format!("{x}|{own}"),
            _ => own.to_string(),
        };
        for &h in chain {
            if xref.is_some() {
                return None;
            }
            let e = doc.get_entity(Handle::new(h))?;
            let common = e.common();
            let layer = drawn(&common.layer, &outer_layer, xref);
            outer_weight = Some(self.weight(&common.line_weight, &layer, outer_weight));
            outer_layer = Some(layer);
            if let opencadcodec::EntityType::Insert(insert) = e {
                if is_xref_block(doc, &insert.block_name) {
                    doc = self
                        .xrefs
                        .get(&normalize_name(&insert.block_name))
                        .copied()?;
                    xref = Some(insert.block_name.as_str());
                }
            }
        }
        let common = doc.get_entity(Handle::new(entity))?.common();
        let layer = drawn(&common.layer, &outer_layer, xref);
        let found = find_layer(self.doc, &self.xrefs, &layer);
        Some(Look {
            weight_mm: self.weight(&common.line_weight, &layer, outer_weight),
            plot: found.is_none_or(|l| l.is_plottable),
            style: found.map(|l| l.plot_style.clone()).unwrap_or_default(),
        })
    }
}

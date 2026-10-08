use std::collections::HashMap;
use std::path::{Path, PathBuf};

use opencadcodec::objects::ObjectType;
use opencadcodec::{CadDocument, DwgReadOptions, DwgReader, DxfReader, EntityType};

use crate::{CoreError, Units};

/// An open DWG or DXF drawing.
pub struct Drawing {
    pub path: PathBuf,
    pub doc: CadDocument,
    /// What the reader had to skip or repair, in its own words.
    pub problems: Vec<String>,
    pub units: Units,
}

/// Model space or one paper layout.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Sheet {
    pub name: String,
    pub block: String,
    pub is_model: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LayerInfo {
    pub name: String,
    pub color: [u8; 3],
    pub off: bool,
    pub frozen: bool,
    pub locked: bool,
}

impl Drawing {
    /// Opens a DXF (by extension) or DWG file. A file the reader can't make sense of is `Unreadable`.
    pub fn open(path: &Path) -> Result<Drawing, CoreError> {
        // A missing or locked file is an Io error; anything the reader rejects is Unreadable.
        std::fs::metadata(path)?;
        let is_dxf = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("dxf"));
        let outcome = if is_dxf {
            DxfReader::from_file(path).and_then(|reader| reader.read_with_stats())
        } else {
            DwgReader::from_file_with_options(path, DwgReadOptions::failsafe())
                .and_then(|mut reader| reader.read_with_stats())
        }
        .map_err(|e| CoreError::Unreadable(e.to_string()))?;
        if !outcome.stats.has_usable_drawing_data() {
            return Err(CoreError::Unreadable(
                "no usable drawing data found".to_string(),
            ));
        }
        let problems = outcome
            .stats
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect();
        let units = Units::from_header(outcome.document.header.insertion_units);
        Ok(Drawing {
            path: path.to_path_buf(),
            doc: outcome.document,
            problems,
            units,
        })
    }

    /// Model first, then each paper layout in tab order.
    pub fn sheets(&self) -> Vec<Sheet> {
        let blocks: HashMap<u64, &str> = self
            .doc
            .block_records
            .iter()
            .map(|b| (b.handle.value(), b.name.as_str()))
            .collect();
        let mut layouts: Vec<(i16, String, String)> = self
            .doc
            .objects
            .values()
            .filter_map(|o| match o {
                ObjectType::Layout(l) if !l.name.eq_ignore_ascii_case("model") => blocks
                    .get(&l.block_record.value())
                    .map(|block| (l.tab_order, l.name.clone(), block.to_string())),
                _ => None,
            })
            .collect();
        layouts.sort();
        let mut sheets = vec![Sheet {
            name: "Model".to_string(),
            block: "*Model_Space".to_string(),
            is_model: true,
        }];
        sheets.extend(layouts.into_iter().map(|(_, name, block)| Sheet {
            name,
            block,
            is_model: false,
        }));
        sheets
    }

    /// Every layer, sorted by name.
    pub fn layers(&self) -> Vec<LayerInfo> {
        let mut layers: Vec<LayerInfo> = self
            .doc
            .layers
            .iter()
            .map(|l| LayerInfo {
                name: l.name.clone(),
                color: l
                    .color
                    .rgb()
                    .map(|(r, g, b)| [r, g, b])
                    .unwrap_or([255, 255, 255]),
                off: l.flags.off,
                frozen: l.flags.frozen,
                locked: l.flags.locked,
            })
            .collect();
        layers.sort_by(|a, b| a.name.cmp(&b.name));
        layers
    }

    /// The entities of one sheet's block (or any block), by block name.
    pub fn entities_in(&self, sheet_block: &str) -> impl Iterator<Item = &EntityType> {
        self.doc.entities_in_block(sheet_block)
    }
}

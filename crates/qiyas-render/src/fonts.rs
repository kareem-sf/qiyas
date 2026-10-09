//! Fonts for drawing text: Noto Sans and Noto Sans Arabic (OFL, bundled) behind every drawing
//! font, and the drawing's own TrueType files when the computer has them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use ab_glyph::FontRef;

/// Noto Sans Regular, unhinted (OFL, `fonts/NotoSans-OFL.txt`).
static NOTO_SANS_TTF: &[u8] = include_bytes!("../fonts/NotoSans-Regular.ttf");
/// Noto Sans Arabic Regular, unhinted (OFL, `fonts/NotoSansArabic-OFL.txt`).
static NOTO_ARABIC_TTF: &[u8] = include_bytes!("../fonts/NotoSansArabic-Regular.ttf");

/// Font ids of the bundled fonts, always the first two.
pub const NOTO_SANS: u16 = 0;
pub const NOTO_ARABIC: u16 = 1;
/// The fallback chain when the drawing's font is an SHX font or is not on this computer.
pub const DEFAULT_CHAIN: [u16; 2] = [NOTO_SANS, NOTO_ARABIC];

/// One loaded font: the shaping face and the outline face over the same bytes.
pub struct Font {
    pub face: rustybuzz::Face<'static>,
    pub raster: FontRef<'static>,
    /// Font units per em.
    pub upem: f32,
}

impl Font {
    fn parse(data: &'static [u8]) -> Option<Font> {
        let face = rustybuzz::Face::from_slice(data, 0)?;
        let raster = FontRef::try_from_slice(data).ok()?;
        let upem = face.units_per_em() as f32;
        Some(Font { face, raster, upem })
    }

    /// Whether the font draws `c` (not the missing-glyph box).
    pub fn has(&self, c: char) -> bool {
        self.face.glyph_index(c).is_some_and(|g| g.0 != 0)
    }
}

/// The fonts of one session. Chains are per font file name of a text style.
pub struct Fonts {
    list: Vec<Font>,
    /// Font file name (lower case) to its font id, or `None` when not found or not a TrueType.
    by_file: HashMap<String, Option<u16>>,
    /// Look in the computer's font folders; off in tests, so they don't depend on the machine.
    system: bool,
}

impl Fonts {
    /// The bundled fonts, and the computer's when `system` is true.
    pub fn new(system: bool) -> Fonts {
        let list = [NOTO_SANS_TTF, NOTO_ARABIC_TTF]
            .map(|data| Font::parse(data).expect("bundled fonts parse"))
            .into();
        Fonts {
            list,
            by_file: HashMap::new(),
            system,
        }
    }

    pub fn get(&self, id: u16) -> &Font {
        &self.list[usize::from(id)]
    }

    /// The fonts to try for a style whose font file is `file` (e.g. `arial.ttf`, `romans.shx`,
    /// `txt`): that file when it is a TrueType or OpenType font found in the computer's font
    /// folders, then Noto Sans, then Noto Sans Arabic. SHX fonts map to the Noto pair.
    pub fn chain(&mut self, file: &str) -> Vec<u16> {
        let mut chain = Vec::with_capacity(3);
        if let Some(id) = self.load(file) {
            chain.push(id);
        }
        chain.extend(DEFAULT_CHAIN);
        chain
    }

    fn load(&mut self, file: &str) -> Option<u16> {
        if !self.system {
            return None;
        }
        let name = file_name(file)?;
        if let Some(found) = self.by_file.get(&name) {
            return *found;
        }
        let found = system_font(&name).and_then(Font::parse).map(|font| {
            self.list.push(font);
            (self.list.len() - 1) as u16
        });
        self.by_file.insert(name, found);
        found
    }
}

/// The lower-case file name of a TrueType or OpenType font file, without any folder (Windows or
/// not); `None` for SHX fonts (which include names without an extension, as AutoCAD reads them).
fn file_name(file: &str) -> Option<String> {
    let name = file.trim().rsplit(['/', '\\']).next()?.to_lowercase();
    [".ttf", ".otf", ".ttc"]
        .iter()
        .any(|ext| name.ends_with(ext))
        .then_some(name)
}

/// The bytes of the font file `name` (lower case) in the computer's font folders. Read once per
/// process and kept for its life, so the faces built on them can live in any session.
fn system_font(name: &str) -> Option<&'static [u8]> {
    static LOADED: OnceLock<Mutex<HashMap<PathBuf, &'static [u8]>>> = OnceLock::new();
    let path = font_index().get(name)?;
    let mut loaded = LOADED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(bytes) = loaded.get(path) {
        return Some(bytes);
    }
    let bytes: &'static [u8] = Box::leak(std::fs::read(path).ok()?.into_boxed_slice());
    loaded.insert(path.clone(), bytes);
    Some(bytes)
}

/// Every font file in the computer's font folders by lower-case file name; built once.
fn font_index() -> &'static HashMap<String, PathBuf> {
    static INDEX: OnceLock<HashMap<String, PathBuf>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = HashMap::new();
        for dir in font_dirs() {
            walk(&dir, 0, &mut index);
        }
        index
    })
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/System/Library/Fonts/Supplemental"),
    ];
    let windows = std::env::var_os("WINDIR").map_or(PathBuf::from("C:\\Windows"), PathBuf::from);
    dirs.push(windows.join("Fonts"));
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/share/fonts"));
        dirs.push(home.join(".fonts"));
        dirs.push(home.join("Library/Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs.push(local.join("Microsoft\\Windows\\Fonts"));
    }
    dirs
}

/// Adds the font files under `dir` to `index`, the first found winning, down to a few folders deep.
fn walk(dir: &Path, depth: usize, index: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 4 {
                walk(&path, depth + 1, index);
            }
        } else if let Some(name) = path.to_str().and_then(file_name) {
            index.entry(name).or_insert(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shx_and_missing_fonts_fall_back_to_noto() {
        let mut fonts = Fonts::new(true);
        assert_eq!(fonts.chain("romans.shx"), DEFAULT_CHAIN);
        assert_eq!(fonts.chain("txt"), DEFAULT_CHAIN);
        assert_eq!(fonts.chain(""), DEFAULT_CHAIN);
        assert_eq!(fonts.chain("no-such-font-anywhere.ttf"), DEFAULT_CHAIN);
    }

    #[test]
    fn bundled_fonts_cover_latin_and_arabic() {
        let fonts = Fonts::new(false);
        assert!(fonts.get(NOTO_SANS).has('A'));
        assert!(!fonts.get(NOTO_SANS).has('ق'));
        assert!(fonts.get(NOTO_ARABIC).has('ق'));
        assert_eq!(
            file_name("C:\\Fonts\\Arial.TTF").as_deref(),
            Some("arial.ttf")
        );
        assert_eq!(file_name("Arial.TTF").as_deref(), Some("arial.ttf"));
    }
}

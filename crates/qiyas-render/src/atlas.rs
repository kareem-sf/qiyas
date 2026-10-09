//! The glyph atlas: one-byte (R8) coverage pages of 2048 x 2048 texels, shelf-packed, grown a
//! page at a time. Pages are never cleared, so a glyph once drawn keeps its place across batches.

/// Width and height of every page, in texels.
pub const PAGE_SIZE: u32 = 2048;
/// Empty texels kept around every glyph, so linear filtering never reads a neighbour.
pub const PAD: u32 = 1;

/// Where a glyph's bitmap sits: its page and the texel rectangle `x0 y0 x1 y1` around it, the
/// padding included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub page: u16,
    pub rect: [u16; 4],
}

/// A row of glyphs of about the same height.
#[derive(Debug, Clone, Copy)]
struct Shelf {
    y: u32,
    height: u32,
    /// Where the next glyph goes on this shelf.
    x: u32,
}

#[derive(Debug, Clone)]
struct Page {
    data: Vec<u8>,
    shelves: Vec<Shelf>,
    /// Top of the space below the last shelf.
    next_y: u32,
    /// The atlas version at which this page last changed.
    changed: u64,
    /// Changed since the last `commit`.
    dirty: bool,
}

impl Page {
    fn new() -> Self {
        Page {
            data: vec![0; (PAGE_SIZE * PAGE_SIZE) as usize],
            shelves: Vec::new(),
            next_y: 0,
            changed: 0,
            dirty: false,
        }
    }

    /// The top-left corner of a free `w` x `h` area, if the page has one. Takes the lowest shelf
    /// that fits the height without wasting more than half of it, else opens a new shelf.
    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let best = self
            .shelves
            .iter_mut()
            .filter(|s| s.height >= h && s.height <= h + h / 2 && s.x + w <= PAGE_SIZE)
            .min_by_key(|s| s.height);
        if let Some(shelf) = best {
            let at = (shelf.x, shelf.y);
            shelf.x += w;
            return Some(at);
        }
        if self.next_y + h > PAGE_SIZE || w > PAGE_SIZE {
            return None;
        }
        let y = self.next_y;
        self.next_y += h;
        self.shelves.push(Shelf { y, height: h, x: w });
        Some((0, y))
    }
}

/// The pages and their version. The version goes up once per `commit` that follows a change, so
/// the window asks only for the pages changed since the version it holds.
#[derive(Debug, Clone, Default)]
pub struct Atlas {
    pages: Vec<Page>,
    version: u64,
}

/// One page as sent to the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageView<'a> {
    pub index: u32,
    pub width: u32,
    pub height: u32,
    /// `width * height` coverage bytes, row by row from the top.
    pub data: &'a [u8],
}

impl Atlas {
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies a `w` x `h` coverage bitmap (row by row) into the first page with room, a new page
    /// when none has, and returns where it went. `w` and `h` must leave room for the padding.
    pub fn insert(&mut self, w: u32, h: u32, pixels: &[u8]) -> Slot {
        assert!(w + 2 * PAD <= PAGE_SIZE && h + 2 * PAD <= PAGE_SIZE);
        assert_eq!(pixels.len(), (w * h) as usize);
        let (pw, ph) = (w + 2 * PAD, h + 2 * PAD);
        let found = self
            .pages
            .iter_mut()
            .enumerate()
            .find_map(|(i, page)| page.allocate(pw, ph).map(|at| (i, at)));
        let (index, (x, y)) = found.unwrap_or_else(|| {
            let mut page = Page::new();
            let at = page.allocate(pw, ph).expect("a new page fits one glyph");
            self.pages.push(page);
            (self.pages.len() - 1, at)
        });
        let page = &mut self.pages[index];
        for row in 0..h {
            let at = ((y + PAD + row) * PAGE_SIZE + x + PAD) as usize;
            let from = (row * w) as usize;
            page.data[at..at + w as usize].copy_from_slice(&pixels[from..from + w as usize]);
        }
        page.dirty = true;
        Slot {
            page: index as u16,
            rect: [x as u16, y as u16, (x + pw) as u16, (y + ph) as u16],
        }
    }

    /// Ends a run of inserts: when any page changed, the version goes up and those pages take it.
    pub fn commit(&mut self) -> u64 {
        if self.pages.iter().any(|p| p.dirty) {
            self.version += 1;
            for page in self.pages.iter_mut().filter(|p| p.dirty) {
                page.changed = self.version;
                page.dirty = false;
            }
        }
        self.version
    }

    /// Goes up whenever a page changes; 0 while the atlas is empty.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }

    /// The pages changed after version `since`; every page when `None`.
    pub fn pages_since(&self, since: Option<u64>) -> impl Iterator<Item = PageView<'_>> {
        self.pages
            .iter()
            .enumerate()
            .filter(move |(_, p)| since.is_none_or(|v| p.changed > v))
            .map(|(i, p)| PageView {
                index: i as u32,
                width: PAGE_SIZE,
                height: PAGE_SIZE,
                data: &p.data,
            })
    }

    /// The pages changed after `since` for the window (IPC `atlas_pages`). Little-endian:
    ///
    /// | size | contents                                                        |
    /// |------|-----------------------------------------------------------------|
    /// | 8    | `u64` atlas version                                             |
    /// | 4    | `u32` n, the number of pages that follow                        |
    /// |      | per page: `u32` index, `u32` width, `u32` height, `u32` byte    |
    /// |      | length (width x height), then the R8 bytes, rows from the top   |
    ///
    /// Raw, not compressed: a page is 4 MiB and goes over local IPC only.
    pub fn to_bytes(&self, since: Option<u64>) -> Vec<u8> {
        let pages: Vec<_> = self.pages_since(since).collect();
        let size: usize = pages.iter().map(|p| 16 + p.data.len()).sum();
        let mut out = Vec::with_capacity(12 + size);
        out.extend(self.version.to_le_bytes());
        out.extend((pages.len() as u32).to_le_bytes());
        for p in pages {
            for v in [p.index, p.width, p.height, p.data.len() as u32] {
                out.extend(v.to_le_bytes());
            }
            out.extend(p.data);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_packs_200_glyphs_without_overlap() {
        let mut atlas = Atlas::new();
        let mut slots = Vec::new();
        for i in 0..200u32 {
            // Sizes of glyphs from 8 to 128 px, as the size buckets give.
            let (w, h) = (6 + (i * 7) % 90, 8 + (i * 13) % 120);
            let pixels = vec![(i % 255 + 1) as u8; (w * h) as usize];
            slots.push((atlas.insert(w, h, &pixels), w, h, (i % 255 + 1) as u8));
        }
        assert_eq!(atlas.page_count(), 1);
        for (i, (a, ..)) in slots.iter().enumerate() {
            let [x0, y0, x1, y1] = a.rect.map(u32::from);
            assert!(x1 <= PAGE_SIZE && y1 <= PAGE_SIZE);
            for (b, ..) in &slots[i + 1..] {
                let [u0, v0, u1, v1] = b.rect.map(u32::from);
                let apart = x1 <= u0 || u1 <= x0 || y1 <= v0 || v1 <= y0;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
        // Every bitmap is where its slot says, inside its padding.
        let page = atlas.pages_since(None).next().unwrap();
        for (slot, w, h, value) in &slots {
            let [x0, y0, ..] = slot.rect.map(u32::from);
            let at = |x: u32, y: u32| page.data[(y * PAGE_SIZE + x) as usize];
            assert_eq!(at(x0 + PAD, y0 + PAD), *value);
            assert_eq!(at(x0 + PAD + w - 1, y0 + PAD + h - 1), *value);
            assert_eq!(at(x0, y0), 0);
        }
    }

    #[test]
    fn a_full_page_opens_another() {
        let mut atlas = Atlas::new();
        let big = vec![1u8; 1000 * 1000];
        for _ in 0..5 {
            atlas.insert(1000, 1000, &big);
        }
        assert_eq!(atlas.page_count(), 2);
    }

    #[test]
    fn version_counts_commits_with_changes_and_names_changed_pages() {
        let mut atlas = Atlas::new();
        assert_eq!(atlas.commit(), 0);
        atlas.insert(2, 2, &[9; 4]);
        assert_eq!(atlas.commit(), 1);
        assert_eq!(atlas.commit(), 1);
        assert_eq!(atlas.pages_since(Some(1)).count(), 0);
        assert_eq!(atlas.pages_since(Some(0)).count(), 1);

        let bytes = atlas.to_bytes(Some(0));
        assert_eq!(u64::from_le_bytes(bytes[0..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
        let header: Vec<u32> = (0..4)
            .map(|i| u32::from_le_bytes(bytes[12 + 4 * i..16 + 4 * i].try_into().unwrap()))
            .collect();
        assert_eq!(header, [0, PAGE_SIZE, PAGE_SIZE, PAGE_SIZE * PAGE_SIZE]);
        assert_eq!(bytes.len(), 28 + (PAGE_SIZE * PAGE_SIZE) as usize);
        assert_eq!(atlas.to_bytes(Some(1)).len(), 12);
    }
}

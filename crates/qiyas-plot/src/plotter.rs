//! Draws a sheet into one PDF page as the screen draws it: for a layout, model space inside each
//! viewport first (clipped to its frame, without the layers it freezes), then the sheet's own
//! batch: pictures, fills, lines, wipeouts in the paper's white, the wipeouts' frames, text.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, StringFormat};
use opencadcodec::objects::ObjectType;
use qiyas_core::{draw_picture, PaperView, Session};
use qiyas_geom::Xform;
use qiyas_render::{
    Batch, LaidText, Range, FILL_BACKGROUND, IMAGE_ADJUSTED, IMAGE_FADED, IMAGE_MONOCHROME,
    TEXT_BOX,
};

use crate::content::{matrix, Content};
use crate::ctb::{object_pen, Pen};
use crate::font::{em_to_text, flate_stream, real, FontBytes, FontSet};
use crate::look::{Look, Looks};
use crate::page::{PageMap, PT_PER_MM};
use crate::{Area, PlotError, PlotJob, PlotOutput, PlotStyle};

const WHITE: [u8; 3] = [255, 255, 255];
/// Picture resolution on the page, in pixels per inch.
const PICTURE_DPI: f64 = 200.0;

/// One batch to draw and how.
struct Layer<'b> {
    batch: &'b Batch,
    /// Batch (drawing) coordinates to page points.
    xf: Xform,
    /// The viewport frame on the page, for model space inside it.
    clip: Option<[[f64; 2]; 4]>,
    /// `(entity, chain)` of what a viewport's frozen layers hide.
    hidden: HashSet<(u64, u32)>,
    /// Draw its pictures (the screen draws none inside viewports yet).
    pictures: bool,
}

/// What the plot draws with an entity, or `None` when it does not plot.
type Drawn = Option<(Pen, f64)>;

struct Plotter<'s> {
    session: &'s Session,
    style: Option<&'s PlotStyle>,
    lineweights: bool,
    looks: Looks<'s>,
    pens: HashMap<([u8; 3], String), Pen>,
    fonts: FontSet,
    out: Content,
    /// Image XObjects by resource name, and the pictures already drawn by picture id.
    images: Vec<(String, ObjectId)>,
    pictures: HashMap<u32, Option<String>>,
    doc: Document,
    skipped: Vec<String>,
}

fn chain_at(batch: &Batch, chain: u32) -> &[u64] {
    batch.chains.get(chain as usize).map_or(&[], Vec::as_slice)
}

fn absolute(batch: &Batch, x: f32, y: f32) -> [f64; 2] {
    [
        batch.origin[0] + f64::from(x),
        batch.origin[1] + f64::from(y),
    ]
}

fn rgb_of(c: u32) -> [u8; 3] {
    let [_, r, g, b] = c.to_be_bytes();
    [r, g, b]
}

fn is_empty(batch: &Batch) -> bool {
    batch.ranges.is_empty()
        && batch.fill_ranges.is_empty()
        && batch.texts.is_empty()
        && batch.images.is_empty()
}

/// What a viewport's frozen layers hide in the model batch: every entity a range of which is
/// not inside the spans the viewport draws.
fn hidden_by(batch: &Batch, view: &PaperView) -> HashSet<(u64, u32)> {
    let mut hidden = HashSet::new();
    let inside = |r: &Range, spans: &[[u32; 2]]| {
        spans
            .iter()
            .any(|[first, count]| r.first >= *first && r.first + r.count <= first + count)
    };
    for (ranges, spans) in [
        (&batch.ranges, &view.lines),
        (&batch.fill_ranges, &view.fills),
    ] {
        if let Some(spans) = spans {
            for r in ranges.iter().filter(|r| r.count > 0 && !inside(r, spans)) {
                hidden.insert((r.entity, r.chain));
            }
        }
    }
    hidden
}

/// The layout's limits (AutoCAD keeps a layout's printable area there), when they have a size.
fn layout_limits(session: &Session, block: &str) -> Option<[[f64; 2]; 2]> {
    let doc = &session.drawing.doc;
    let record = doc.block_records.get(block)?.handle;
    doc.objects.values().find_map(|o| match o {
        ObjectType::Layout(l) if l.block_record == record => {
            let (lo, hi) = (l.min_limits, l.max_limits);
            (hi.0 > lo.0 && hi.1 > lo.1).then_some([[lo.0, lo.1], [hi.0, hi.1]])
        }
        _ => None,
    })
}

/// Plots `job` into the PDF at `path` (written beside it, then moved over it).
pub fn plot(session: &Session, job: &PlotJob, path: &Path) -> Result<PlotOutput, PlotError> {
    let sheet = session
        .drawing
        .sheets()
        .into_iter()
        .find(|s| s.block.eq_ignore_ascii_case(&job.sheet))
        .ok_or_else(|| PlotError::NoSuchSheet(job.sheet.clone()))?;
    let batch = session.sheet_batch(&sheet.block);
    let views = if sheet.is_model {
        Vec::new()
    } else {
        session.paper_views_of(&sheet.block)
    };
    let model = (!views.is_empty()).then(|| session.model_batch());
    let extents = || {
        if is_empty(&batch) && views.is_empty() {
            Err(PlotError::Nothing)
        } else {
            Ok(batch.bounds)
        }
    };
    let area = match job.area {
        Area::Extents => extents()?,
        Area::Window([a, b]) => [
            [a[0].min(b[0]), a[1].min(b[1])],
            [a[0].max(b[0]), a[1].max(b[1])],
        ],
        Area::Layout if !sheet.is_model => match layout_limits(session, &sheet.block) {
            Some(limits) => limits,
            None => extents()?,
        },
        Area::Layout => extents()?,
    };
    let mm_per_unit = if sheet.is_model {
        session.drawing.units.to_metres() * 1000.0
    } else {
        1.0
    };
    let page = PageMap::new(
        &job.paper,
        job.orientation,
        job.scale,
        job.centre,
        area,
        mm_per_unit,
    )?;

    let mut layers = Vec::new();
    if let Some(model) = &model {
        for view in &views {
            let [lo, hi] = view.rect;
            let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
            let xf = Xform::translate(-view.centre[0], -view.centre[1])
                .then(&Xform::scale(view.scale, view.scale))
                .then(&Xform::translate(centre[0], centre[1]))
                .then(&page.xf);
            let clip = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]].map(|p| page.xf.apply(p));
            layers.push(Layer {
                batch: model,
                xf,
                clip: Some(clip),
                hidden: hidden_by(model, view),
                pictures: false,
            });
        }
    }
    layers.push(Layer {
        batch: &batch,
        xf: page.xf,
        clip: None,
        hidden: HashSet::new(),
        pictures: true,
    });

    let doc = &session.drawing.doc;
    let mut p = Plotter {
        session,
        style: job.style.as_ref(),
        lineweights: job.lineweights,
        looks: Looks::new(doc, session.xrefs.docs()),
        pens: HashMap::new(),
        fonts: FontSet::default(),
        out: Content::default(),
        images: Vec::new(),
        pictures: HashMap::new(),
        doc: Document::with_version("1.5"),
        skipped: Vec::new(),
    };
    if layers.iter().any(|l| l.batch.truncated) {
        p.skipped
            .push("Some blocks are too complex to draw: parts of them are left out".into());
    }
    let bytes = |id: u16| session.font_bytes(id);
    for layer in &layers {
        p.prepare_text(layer, &bytes);
    }
    p.fonts.finish();
    // Round caps and joins, as the screen's lines end.
    p.out.op("1 J 1 j");
    for layer in &layers {
        p.draw(layer, &bytes);
    }
    p.write(path, &page, &sheet.name)?;
    Ok(PlotOutput {
        path: path.to_path_buf(),
        skipped: p.skipped,
    })
}

impl Plotter<'_> {
    /// How `entity` (inside `chain`) drawn `rgb` plots: its pen and line width in points.
    fn drawn(&mut self, look: &Look, rgb: [u8; 3]) -> Drawn {
        if !look.plot {
            return None;
        }
        let pen = match self.style {
            None => object_pen(rgb),
            Some(style) => *self
                .pens
                .entry((rgb, look.style.clone()))
                .or_insert_with(|| style.apply(rgb, Some(&look.style))),
        };
        if !pen.plot {
            return None;
        }
        let object = if self.lineweights {
            look.weight_mm
        } else {
            0.0
        };
        Some((pen, pen.lineweight_mm.unwrap_or(object) * PT_PER_MM))
    }

    fn look(&mut self, batch: &Batch, entity: u64, chain: u32) -> Look {
        self.looks.of(entity, chain_at(batch, chain))
    }

    fn prepare_text(&mut self, layer: &Layer, bytes: &FontBytes) {
        for text in &layer.batch.texts {
            if self.text_pen(layer, text).is_some() {
                self.fonts.prepare(text, bytes);
            }
        }
    }

    fn text_pen(&mut self, layer: &Layer, text: &LaidText) -> Option<Pen> {
        if layer.hidden.contains(&(text.entity, text.chain)) || text.glyphs.is_empty() {
            return None;
        }
        let look = self.look(layer.batch, text.entity, text.chain);
        self.drawn(&look, rgb_of(text.color)).map(|(pen, _)| pen)
    }

    fn draw(&mut self, layer: &Layer, bytes: &FontBytes) {
        self.out.save();
        if let Some(c) = layer.clip {
            self.out.point(c[0], "m");
            for p in &c[1..] {
                self.out.point(*p, "l");
            }
            self.out.op("h W n");
        }
        if layer.pictures {
            self.pictures(layer);
        }
        let batch = layer.batch;
        let frames: HashSet<u64> = batch
            .fill_ranges
            .iter()
            .filter(|r| {
                batch
                    .fill_colors
                    .get(r.first as usize)
                    .is_some_and(|c| c & FILL_BACKGROUND != 0)
            })
            .map(|r| r.entity)
            .collect();
        self.fills(layer, false);
        self.lines(layer, &frames, false);
        if !frames.is_empty() {
            self.fills(layer, true);
            self.lines(layer, &frames, true);
        }
        for text in &batch.texts {
            self.text(layer, text, bytes);
        }
        self.out.restore();
    }

    /// The fill triangles: coloured ones, or (`background`) the wipeouts in the paper's white.
    fn fills(&mut self, layer: &Layer, background: bool) {
        let batch = layer.batch;
        let mut open: Option<[u8; 3]> = None;
        for r in &batch.fill_ranges {
            if r.count == 0 || layer.hidden.contains(&(r.entity, r.chain)) {
                continue;
            }
            let look = self.look(batch, r.entity, r.chain);
            for t in r.first..r.first + r.count {
                let Some(&c) = batch.fill_colors.get(t as usize) else {
                    break;
                };
                if (c & FILL_BACKGROUND != 0) != background {
                    continue;
                }
                let color = if background {
                    look.plot.then_some(WHITE)
                } else {
                    self.drawn(&look, rgb_of(c)).map(|(pen, _)| pen.rgb)
                };
                let Some(color) = color else {
                    continue;
                };
                if open != Some(color) {
                    if open.is_some() {
                        self.out.op("B");
                    }
                    self.out.fill(color);
                    self.out.stroke(color);
                    self.out.width(0.0);
                    open = Some(color);
                }
                let at = 6 * t as usize;
                let Some(v) = batch.fills.get(at..at + 6) else {
                    break;
                };
                for (k, op) in ["m", "l", "l"].iter().enumerate() {
                    let p = absolute(batch, v[2 * k], v[2 * k + 1]);
                    self.out.point(layer.xf.apply(p), op);
                }
                self.out.op("h");
            }
        }
        if open.is_some() {
            self.out.op("B");
        }
    }

    /// The line segments: all but the wipeouts' frames, or (`frames`) only those.
    fn lines(&mut self, layer: &Layer, frames: &HashSet<u64>, only_frames: bool) {
        let batch = layer.batch;
        let mut open: Option<([u8; 3], f64)> = None;
        let mut end: Option<[f32; 2]> = None;
        for r in &batch.ranges {
            if r.flags & TEXT_BOX != 0
                || r.count == 0
                || frames.contains(&r.entity) != only_frames
                || layer.hidden.contains(&(r.entity, r.chain))
            {
                continue;
            }
            let look = self.look(batch, r.entity, r.chain);
            for i in r.first..r.first + r.count {
                let i = i as usize;
                let (Some(&c), Some(s)) =
                    (batch.colors.get(i), batch.segments.get(4 * i..4 * i + 4))
                else {
                    break;
                };
                let Some((pen, width)) = self.drawn(&look, rgb_of(c)) else {
                    continue;
                };
                if open != Some((pen.rgb, width)) {
                    if open.is_some() {
                        self.out.op("S");
                    }
                    self.out.stroke(pen.rgb);
                    self.out.width(width);
                    open = Some((pen.rgb, width));
                    end = None;
                }
                if end != Some([s[0], s[1]]) {
                    self.out
                        .point(layer.xf.apply(absolute(batch, s[0], s[1])), "m");
                }
                self.out
                    .point(layer.xf.apply(absolute(batch, s[2], s[3])), "l");
                end = Some([s[2], s[3]]);
            }
        }
        if open.is_some() {
            self.out.op("S");
        }
    }

    /// A text: real PDF text in its embedded font when it maps back to its characters, else
    /// its glyphs' outlines filled.
    fn text(&mut self, layer: &Layer, text: &LaidText, bytes: &FontBytes) {
        let Some(pen) = self.text_pen(layer, text) else {
            return;
        };
        let to_page = text.xf.then(&layer.xf);
        if self.fonts.as_text(text) {
            self.out.op("BT");
            self.out.fill(pen.rgb);
            let mut font = None;
            let em = 1.0 / f64::from(qiyas_render::CAP_EM);
            for g in &text.glyphs {
                if font != Some(g.font) {
                    self.out.op(&format!("/F{} 1 Tf", g.font));
                    font = Some(g.font);
                }
                let tm = Xform::scale(em, em)
                    .then(&Xform::translate(f64::from(g.x), f64::from(g.y)))
                    .then(&to_page);
                self.out.matrix(matrix(&tm), "Tm");
                let gid = self.fonts.subset_gid(g.font, g.glyph);
                self.out.op(&format!("<{gid:04X}> Tj"));
            }
            self.out.op("ET");
        } else {
            self.out.fill(pen.rgb);
            let start = self.out.buf.len();
            let mut drew = false;
            for g in &text.glyphs {
                let upem = self.fonts.units_per_em(g.font);
                let xf = em_to_text(upem)
                    .then(&Xform::translate(f64::from(g.x), f64::from(g.y)))
                    .then(&to_page);
                drew |= self
                    .fonts
                    .outline(g.font, g.glyph, &xf, bytes, &mut self.out);
            }
            if drew {
                self.out.op("f");
            } else if self.out.buf.len() > start {
                self.out.op("n");
            }
        }
    }

    /// The sheet's pictures (PDF underlays and raster images) at about 200 pixels per inch.
    fn pictures(&mut self, layer: &Layer) {
        let batch = layer.batch;
        let m = &layer.xf.m;
        let pt_per_unit = (m[0][0] * m[1][1] - m[0][1] * m[1][0]).abs().sqrt();
        for quad in &batch.images {
            let name = match self.pictures.get(&quad.id) {
                Some(name) => name.clone(),
                None => {
                    let name = self.picture(quad.id, quad.flags, pt_per_unit);
                    self.pictures.insert(quad.id, name.clone());
                    name
                }
            };
            let Some(name) = name else {
                continue;
            };
            let c = quad.corners;
            let corner = |k: usize| layer.xf.apply(absolute(batch, c[2 * k], c[2 * k + 1]));
            let (tl, br, bl) = (corner(0), corner(2), corner(3));
            self.out.save();
            self.out.matrix(
                [
                    br[0] - bl[0],
                    br[1] - bl[1],
                    tl[0] - bl[0],
                    tl[1] - bl[1],
                    bl[0],
                    bl[1],
                ],
                "cm",
            );
            self.out.op(&format!("/{name} Do"));
            self.out.restore();
        }
    }

    /// Draws picture `id` and adds it as an image XObject; its resource name, or `None` (noted
    /// in `skipped`) when it can't be drawn.
    fn picture(&mut self, id: u32, flags: u32, pt_per_unit: f64) -> Option<String> {
        let px = pt_per_unit / 72.0 * PICTURE_DPI;
        let drawn = self
            .session
            .underlay_image(id, px)
            .and_then(|u| draw_picture(&u, |_| {}));
        let image = match drawn {
            Ok(image) => image,
            Err(e) => {
                self.skipped.push(format!("A picture is left out: {e}"));
                return None;
            }
        };
        let (w, h) = (image.width(), image.height());
        let raw = image.into_raw();
        let (mono, brightness, contrast, fade) = adjustments(flags);
        let mut rgb = Vec::with_capacity(raw.len() / 4 * 3);
        let mut alpha = Vec::with_capacity(raw.len() / 4);
        for px in raw.as_chunks::<4>().0 {
            let mut c = [px[0], px[1], px[2]].map(|v| f64::from(v) / 255.0);
            if mono {
                c = [0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]; 3];
            }
            for v in &mut c {
                *v = ((*v - 0.5) * contrast + 0.5 + brightness).clamp(0.0, 1.0);
                *v += (1.0 - *v) * fade;
            }
            rgb.extend(c.map(|v| (v * 255.0).round() as u8));
            alpha.push(px[3]);
        }
        let mut dict = dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => i64::from(w),
            "Height" => i64::from(h),
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        };
        if alpha.iter().any(|a| *a < 255) {
            let mask = self.doc.add_object(flate_stream(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => i64::from(w),
                    "Height" => i64::from(h),
                    "ColorSpace" => "DeviceGray",
                    "BitsPerComponent" => 8,
                },
                &alpha,
            ));
            dict.set("SMask", mask);
        }
        let object = self.doc.add_object(flate_stream(dict, &rgb));
        let name = format!("Im{}", self.images.len());
        self.images.push((name.clone(), object));
        Some(name)
    }

    fn write(&mut self, path: &Path, page: &PageMap, title: &str) -> Result<(), PlotError> {
        let doc = &mut self.doc;
        let pages = doc.new_object_id();
        let fonts = self.fonts.write(doc);
        let mut xobjects = Dictionary::new();
        for (name, id) in &self.images {
            xobjects.set(name.as_str(), *id);
        }
        let content = doc.add_object(flate_stream(Dictionary::new(), self.out.buf.as_bytes()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), real(page.size[0]), real(page.size[1])],
            "Contents" => content,
            "Resources" => dictionary! { "Font" => fonts, "XObject" => xobjects },
        });
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        let mut utf16 = vec![0xFE, 0xFF];
        utf16.extend(title.encode_utf16().flat_map(u16::to_be_bytes));
        let info = doc.add_object(dictionary! {
            "Producer" => Object::string_literal("Qiyas"),
            "Title" => Object::String(utf16, StringFormat::Hexadecimal),
        });
        doc.trailer.set("Root", catalog);
        doc.trailer.set("Info", info);

        let name = path
            .file_name()
            .ok_or_else(|| PlotError::Output("Give the PDF a file name".into()))?;
        let tmp = path.with_file_name(format!(".{}.qiyas-tmp", name.to_string_lossy()));
        let written = (|| -> Result<(), PlotError> {
            let file = std::fs::File::create(&tmp).map_err(write_error)?;
            let mut w = std::io::BufWriter::new(file);
            doc.save_to(&mut w)
                .map_err(|e| PlotError::Write(e.to_string()))?;
            w.flush().map_err(write_error)?;
            drop(w);
            std::fs::rename(&tmp, path).map_err(write_error)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written
    }
}

fn write_error(e: std::io::Error) -> PlotError {
    PlotError::Write(e.to_string())
}

/// A picture's look from its quad flags, as the screen draws it: grey, brightness added (-1 to
/// 1), contrast about mid grey (0 to 2) and the fade into the paper (0 to 1).
fn adjustments(flags: u32) -> (bool, f64, f64, f64) {
    let mono = flags & IMAGE_MONOCHROME != 0;
    if flags & IMAGE_ADJUSTED == 0 {
        let fade = if flags & IMAGE_FADED != 0 { 0.5 } else { 0.0 };
        return (mono, 0.0, 1.0, fade);
    }
    let byte = |shift: u32| f64::from(((flags >> shift) & 0xFF).min(100));
    (
        mono,
        (byte(8) - 50.0) / 50.0,
        byte(16) / 50.0,
        byte(24) / 100.0,
    )
}

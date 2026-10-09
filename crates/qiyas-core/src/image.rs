//! Raster images (scanned plans, photos) in the drawing, kept as AutoCAD keeps them: a
//! `RasterImage` entity, its `ImageDefinition` in the `ACAD_IMAGE_DICT` dictionary and the
//! `ImageDefinitionReactor` between them, so AutoCAD shows them too. They are drawn as pictures
//! under the lines, as PDF underlays are: the batch carries where (`placed_raster`), the window
//! asks for the pixels (`Session::underlay_image`, then `draw_picture`).
//!
//! The file is decoded once (kept by path, file date and size), cut to 4096 pixels on its long
//! side, and each picture asked for is resized from that. A JPEG's (or TIFF's, PNG's, WebP's)
//! EXIF orientation is applied, so a phone photo stands the way it was taken.
//!
//! A rectangular clip boundary crops the picture; a polygonal or inverted one draws the whole
//! image for now.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

// `::image` is the image crate; this module is `crate::image`.
use ::image::error::{LimitError, LimitErrorKind};
use ::image::imageops::{self, FilterType};
use ::image::metadata::Orientation;
use ::image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbaImage};
use opencadcodec::entities::{ClipMode, ClipType, ImageDisplayFlags, RasterImage};
use opencadcodec::objects::{ImageDefinition, ImageDefinitionReactor, ObjectType};
use opencadcodec::{CadDocument, Color, EntityType, Handle, Vector3};
use qiyas_render::{image_flags, layer_shown, Visibility};
use serde::Serialize;

use crate::underlay::{
    named_dictionary, resolve_link, stored_link, unique_entry, Picture, PictureSource, Placed,
    UnderlayImage,
};
use crate::{Command, CoreError, Outcome, Session};

/// The layer new images go on, so the layers panel can hide them all at once.
pub const IMAGE_LAYER: &str = "IMAGE-UNDERLAY";
/// The file types `Session::attach_image` reads, by extension.
pub const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp"];
/// The named-object dictionary entry AutoCAD keeps image definitions in.
const IMAGE_DICTIONARY: &str = "ACAD_IMAGE_DICT";
/// The longest side a decoded image is kept at, in pixels.
const MAX_DECODED_PX: u32 = 4096;
/// The widest and tallest image read, in pixels; a larger one is refused before decoding.
const MAX_IMAGE_PX: u32 = 20_000;
/// The most memory one decode may take (an A0 colour scan at 300 dpi is about 420 MB).
const MAX_DECODE_BYTES: u64 = 512 << 20;
/// Decoded images kept.
const DECODED_KEPT: usize = 3;

/// An image's brightness, contrast and fade, 0 to 100 each, as AutoCAD keeps them (50, 50 and 0
/// show it as it is).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ImageDisplay {
    pub brightness: u8,
    pub contrast: u8,
    pub fade: u8,
}

/// The file's date and size, to see a changed file.
type Stamp = (u64, u128);

/// A decoded image, by path and stamp.
type Decoded = (PathBuf, Option<Stamp>, Arc<RgbaImage>);

/// The last decoded images; the least recently used first.
static DECODED: Mutex<Vec<Decoded>> = Mutex::new(Vec::new());

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Some((meta.len(), modified))
}

fn unreadable(path: &Path, e: impl std::fmt::Display) -> CoreError {
    CoreError::Image(format!("This image can't be read: {}: {e}", path.display()))
}

/// `unreadable`, but an image past the limits says it is too large.
fn decode_error(path: &Path, e: ::image::ImageError) -> CoreError {
    match e {
        ::image::ImageError::Limits(_) => CoreError::Image(format!(
            "This image is too large: {} (over {MAX_IMAGE_PX} pixels a side, or over {} MB \
             decoded)",
            path.display(),
            MAX_DECODE_BYTES >> 20
        )),
        e => unreadable(path, e),
    }
}

/// A reader for the image at `path`, its type guessed from its first bytes.
fn reader(path: &Path) -> Result<ImageReader<BufReader<File>>, CoreError> {
    let mut reader = ImageReader::open(path)
        .map_err(|e| unreadable(path, e))?
        .with_guessed_format()
        .map_err(|e| unreadable(path, e))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_PX);
    limits.max_image_height = Some(MAX_IMAGE_PX);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    Ok(reader)
}

/// The decoder for the image at `path`, its header read. The `image` crate checks `max_alloc`
/// only for some formats (PNG, TIFF), so the decoded size is checked here for all.
fn decoder(path: &Path) -> Result<impl ImageDecoder, CoreError> {
    let decoder = reader(path)?
        .into_decoder()
        .map_err(|e| decode_error(path, e))?;
    if decoder.total_bytes() > MAX_DECODE_BYTES {
        let e = LimitError::from_kind(LimitErrorKind::InsufficientMemory);
        return Err(decode_error(path, ::image::ImageError::Limits(e)));
    }
    Ok(decoder)
}

/// Whether `o` turns the image a quarter, swapping its width and height.
fn turns(o: Orientation) -> bool {
    matches!(
        o,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

/// Width and height in pixels of the image at `path` as it is shown (turned by its EXIF
/// orientation). Reads the file's header only.
pub fn image_size(path: &Path) -> Result<(u32, u32), CoreError> {
    let mut decoder = decoder(path)?;
    let (w, h) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    Ok(if turns(orientation) { (h, w) } else { (w, h) })
}

/// The image at `path` decoded, turned by its EXIF orientation, and cut to `MAX_DECODED_PX` on
/// its long side.
fn decode(path: &Path) -> Result<RgbaImage, CoreError> {
    let mut decoder = decoder(path)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| decode_error(path, e))?;
    let (w, h) = (image.width(), image.height());
    let long = w.max(h);
    if long > MAX_DECODED_PX {
        // Before the RGBA copy, so a grey scan is resized at one byte a pixel.
        let k = f64::from(MAX_DECODED_PX) / f64::from(long);
        let size = |v: u32| ((f64::from(v) * k).round() as u32).max(1);
        image = image.resize_exact(size(w), size(h), FilterType::Triangle);
    }
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

/// The image at `path` decoded (see `decode`), from the cache while the file is unchanged.
fn decoded(path: &Path) -> Result<Arc<RgbaImage>, CoreError> {
    let stamp = stamp(path);
    let lock = || DECODED.lock().unwrap_or_else(PoisonError::into_inner);
    {
        let mut cache = lock();
        if let Some(at) = cache.iter().position(|(p, s, _)| p == path && *s == stamp) {
            let entry = cache.remove(at);
            let image = entry.2.clone();
            cache.push(entry);
            return Ok(image);
        }
    }
    // Decoded without the lock, so other pictures are served meanwhile.
    let image = Arc::new(decode(path)?);
    let mut cache = lock();
    cache.retain(|(p, ..)| p != path);
    while cache.len() >= DECODED_KEPT {
        cache.remove(0);
    }
    cache.push((path.to_path_buf(), stamp, image.clone()));
    Ok(image)
}

/// The part `crop` (shares of the image, left, top, right, bottom; `None`: all) of the image at
/// `path`, its long side `long_px` pixels or its own when smaller. RGBA, rows from the top.
fn draw_raster(path: &Path, crop: Option<[f64; 4]>, long_px: u32) -> Result<RgbaImage, CoreError> {
    let image = decoded(path)?;
    let (w, h) = (f64::from(image.width()), f64::from(image.height()));
    let [l, t, r, b] = crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let x = ((l * w).floor() as u32).min(image.width() - 1);
    let y = ((t * h).floor() as u32).min(image.height() - 1);
    let cw = (((r * w).ceil() as u32).min(image.width()) - x).max(1);
    let ch = (((b * h).ceil() as u32).min(image.height()) - y).max(1);
    let k = (f64::from(long_px.max(1)) / f64::from(cw.max(ch))).min(1.0);
    let size = |v: u32| ((f64::from(v) * k).round() as u32).max(1);
    let part = imageops::crop_imm(image.as_ref(), x, y, cw, ch);
    if k >= 1.0 {
        return Ok(part.to_image());
    }
    Ok(imageops::resize(
        &*part,
        size(cw),
        size(ch),
        FilterType::Triangle,
    ))
}

/// Draws a picture `Session::underlay_image` described: a PDF page through pdfium (`on_progress`
/// hears 0 to 100), or a raster image. RGBA, rows from the top. Needs no session, so a caller can
/// draw without holding one.
pub fn draw_picture(
    picture: &UnderlayImage,
    on_progress: impl FnMut(u8),
) -> Result<RgbaImage, CoreError> {
    match picture.source {
        PictureSource::Pdf { page, dpi } => Ok(qiyas_pdf::render_page(
            &picture.path,
            page,
            dpi,
            on_progress,
        )?),
        PictureSource::Raster { crop, long_px } => draw_raster(&picture.path, crop, long_px),
    }
}

/// The part of `image` its clip boundary shows, as shares of the image (left, top, right,
/// bottom, from its top-left corner): only for a rectangular boundary that clips the outside;
/// `None` (all of it) otherwise. Clip vertices are in pixels with (-0.5, -0.5) the image's
/// top-left corner and y down, as AutoCAD keeps them.
fn rect_crop(image: &RasterImage) -> Option<[f64; 4]> {
    let clip = &image.clip_boundary;
    if !image.clipping_enabled
        || clip.clip_type != ClipType::Rectangular
        || clip.clip_mode != ClipMode::Outside
        || clip.vertices.len() < 2
    {
        return None;
    }
    let (min, max) = clip.bounding_rect()?;
    let (w, h) = (image.size.x, image.size.y);
    let share = |v: f64, of: f64| ((v + 0.5) / of).clamp(0.0, 1.0);
    let [l, t, r, b] = [
        share(min.x, w),
        share(min.y, h),
        share(max.x, w),
        share(max.y, h),
    ];
    if !(r - l > 0.0 && b - t > 0.0) {
        return None;
    }
    if l <= 0.0 && t <= 0.0 && r >= 1.0 && b >= 1.0 {
        return None;
    }
    Some([l, t, r, b])
}

/// How the raster image `image` of `doc` (a drawing at `drawing`) is drawn, `None` when it is
/// not (hidden, unloaded, no definition), or the problem line when its file is not found.
pub(crate) fn placed_raster(
    doc: &CadDocument,
    drawing: &Path,
    visibility: &Visibility,
    image: &RasterImage,
) -> Result<Option<Placed>, String> {
    if image.common.invisible
        || !image.is_visible()
        || !layer_shown(doc, visibility, &image.common.layer)
    {
        return Ok(None);
    }
    let def = match image.definition_handle.and_then(|h| doc.objects.get(&h)) {
        Some(ObjectType::ImageDefinition(def)) => def,
        _ => return Ok(None),
    };
    let (w, h) = (image.size.x, image.size.y);
    if !def.is_loaded || !(w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()) {
        return Ok(None);
    }
    let Some(file) = resolve_link(drawing, &def.file_name) else {
        return Err(format!("Image {} not found", def.file_name));
    };
    let crop = rect_crop(image);
    let [l, t, r, b] = crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let (o, u, v) = (image.insertion_point, image.u_vector, image.v_vector);
    // The insertion point is the image's bottom-left corner; `u` and `v` are one pixel.
    let at = |fx: f64, from_top: f64| {
        let (x, y) = (fx * w, (1.0 - from_top) * h);
        [o.x + u.x * x + v.x * y, o.y + u.y * x + v.y * y]
    };
    Ok(Some(Placed {
        handle: image.common.handle.value(),
        file,
        picture: Picture::Raster { crop },
        corners: [at(l, t), at(r, t), at(r, b), at(l, b)],
        flags: image_flags(image.brightness, image.contrast, image.fade),
        pickable: true,
    }))
}

impl Session {
    /// Attaches the raster image at `path` (png, jpg, tif, bmp, webp) to the sheet on screen, its
    /// bottom-left corner at `insert_at`, `width_units` drawing units wide (square pixels). Its
    /// handle. One undo step.
    pub fn attach_image(
        &mut self,
        path: &Path,
        insert_at: [f64; 2],
        width_units: f64,
    ) -> Result<u64, CoreError> {
        let cmd = Command::AttachImage {
            path: path.display().to_string(),
            insert_at,
            width_units,
        };
        let Outcome::Added { handles } = self.run(cmd)? else {
            return Ok(0);
        };
        Ok(handles.first().copied().unwrap_or(0))
    }

    /// Sets the brightness, contrast and fade (0 to 100 each) of the image `handle`. One undo
    /// step.
    pub fn set_image_display(
        &mut self,
        handle: u64,
        display: ImageDisplay,
    ) -> Result<(), CoreError> {
        let ImageDisplay {
            brightness,
            contrast,
            fade,
        } = display;
        self.run(Command::SetImageDisplay {
            handle,
            brightness,
            contrast,
            fade,
        })
        .map(|_| ())
    }

    /// The brightness, contrast and fade of the raster image `handle`; `None` when it is not one.
    pub fn image_display(&self, handle: u64) -> Option<ImageDisplay> {
        match self.drawing.doc.get_entity(Handle::new(handle)) {
            Some(EntityType::RasterImage(image)) => Some(ImageDisplay {
                brightness: image.brightness,
                contrast: image.contrast,
                fade: image.fade,
            }),
            _ => None,
        }
    }

    /// Picture `id` of the batch for a zoom of `px_per_unit` (`underlay_image`), drawn.
    pub fn picture(&self, id: u32, px_per_unit: f64) -> Result<RgbaImage, CoreError> {
        draw_picture(&self.underlay_image(id, px_per_unit)?, |_| {})
    }

    /// `Command::AttachImage`: the image entity, its definition (one per file, reused) and the
    /// reactor between them. The image's handle.
    pub(crate) fn add_image(
        &mut self,
        path: &Path,
        insert_at: [f64; 2],
        width_units: f64,
    ) -> Result<u64, CoreError> {
        if !(width_units > 0.0 && width_units.is_finite()) {
            return Err(CoreError::InvalidCommand(
                "The width must be a number above 0".to_string(),
            ));
        }
        if !insert_at.iter().all(|v| v.is_finite()) {
            return Err(CoreError::InvalidCommand(
                "The insert point must be two numbers".to_string(),
            ));
        }
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let (px_w, px_h) = image_size(&path)?;
        let stored = stored_link(&self.drawing.path, &path);
        self.ensure_layer(IMAGE_LAYER, Color::Index(8));
        let owner = self.sheet_owner();
        let doc = &mut self.drawing.doc;
        // IMAGE, IMAGEDEF and IMAGEDEF_REACTOR need their classes for the DWG writer.
        doc.classes.update_defaults();
        let dictionary = named_dictionary(doc, IMAGE_DICTIONARY);
        let existing = match doc.objects.get(&dictionary) {
            Some(ObjectType::Dictionary(d)) => {
                d.entries
                    .iter()
                    .find_map(|(_, h)| match doc.objects.get(h) {
                        Some(ObjectType::ImageDefinition(def))
                            if def.file_name == stored && def.size_in_pixels == (px_w, px_h) =>
                        {
                            Some(*h)
                        }
                        _ => None,
                    })
            }
            _ => None,
        };
        let definition = match existing {
            Some(h) => h,
            None => {
                let mut def = ImageDefinition::with_dimensions(&stored, px_w, px_h);
                def.handle = doc.allocate_handle();
                def.owner = dictionary;
                def.is_loaded = true;
                let stem = path
                    .file_stem()
                    .map_or("IMAGE".to_string(), |s| s.to_string_lossy().to_string());
                let name = unique_entry(doc, dictionary, &stem);
                let h = def.handle;
                if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dictionary) {
                    d.add_entry(name, h);
                }
                doc.objects.insert(h, ObjectType::ImageDefinition(def));
                h
            }
        };
        let handle = doc.allocate_handle();
        let mut reactor = ImageDefinitionReactor::new(handle);
        reactor.handle = doc.allocate_handle();
        reactor.owner = handle;
        let pixel = width_units / f64::from(px_w);
        let mut image = RasterImage::new(
            &stored,
            Vector3::new(insert_at[0], insert_at[1], 0.0),
            f64::from(px_w),
            f64::from(px_h),
        );
        image.common.handle = handle;
        // Shown, also when not lined up with the screen, inside its clip boundary: AutoCAD's own.
        image.flags = ImageDisplayFlags::SHOW_IMAGE
            | ImageDisplayFlags::SHOW_NOT_ALIGNED
            | ImageDisplayFlags::USE_CLIPPING_BOUNDARY;
        image.common.layer = IMAGE_LAYER.to_string();
        if let Some(owner) = owner {
            image.common.owner_handle = owner;
        }
        image.u_vector = Vector3::new(pixel, 0.0, 0.0);
        image.v_vector = Vector3::new(0.0, pixel, 0.0);
        image.definition_handle = Some(definition);
        image.definition_reactor_handle = Some(reactor.handle);
        doc.objects
            .insert(reactor.handle, ObjectType::ImageDefinitionReactor(reactor));
        doc.add_entity(EntityType::RasterImage(image))
            .map_err(|e| CoreError::InvalidCommand(e.to_string()))?;
        Ok(handle.value())
    }

    /// `Command::SetImageDisplay`.
    pub(crate) fn apply_image_display(
        &mut self,
        handle: u64,
        display: ImageDisplay,
    ) -> Result<(), CoreError> {
        let values = [display.brightness, display.contrast, display.fade];
        if values.iter().any(|v| *v > 100) {
            return Err(CoreError::InvalidCommand(
                "Brightness, contrast and fade go from 0 to 100".to_string(),
            ));
        }
        match self.drawing.doc.get_entity_mut(Handle::new(handle)) {
            Some(EntityType::RasterImage(image)) => {
                image.brightness = display.brightness;
                image.contrast = display.contrast;
                image.fade = display.fade;
                Ok(())
            }
            Some(_) => Err(CoreError::InvalidCommand(
                "Only an image has brightness, contrast and fade".to_string(),
            )),
            None => Err(CoreError::NoSuchEntity(handle)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::{ImageFormat, RgbImage};

    fn with_clip(vertices: [(f64, f64); 2]) -> RasterImage {
        let mut image = RasterImage::new("a.png", Vector3::new(0.0, 0.0, 0.0), 200.0, 100.0);
        image.set_clip_rect(
            opencadcodec::Vector2::new(vertices[0].0, vertices[0].1),
            opencadcodec::Vector2::new(vertices[1].0, vertices[1].1),
        );
        image
    }

    #[test]
    fn a_rectangular_clip_gives_shares_from_the_top_left() {
        let crop = rect_crop(&with_clip([(49.5, 9.5), (-0.5, 59.5)])).unwrap();
        assert_eq!(crop, [0.0, 0.1, 0.25, 0.6]);
        // The whole image, or clipping off, crops nothing.
        assert_eq!(rect_crop(&with_clip([(-0.5, -0.5), (199.5, 99.5)])), None);
        let mut off = with_clip([(49.5, 9.5), (-0.5, 59.5)]);
        off.clipping_enabled = false;
        assert_eq!(rect_crop(&off), None);
        // Polygonal and inverted clips draw the whole image.
        let mut polygon = with_clip([(49.5, 9.5), (-0.5, 59.5)]);
        polygon.clip_boundary.clip_type = ClipType::Polygonal;
        assert_eq!(rect_crop(&polygon), None);
        let mut inverted = with_clip([(49.5, 9.5), (-0.5, 59.5)]);
        inverted.clip_boundary.clip_mode = ClipMode::Inside;
        assert_eq!(rect_crop(&inverted), None);
    }

    /// A PNG of `w` x `h` RGB pixels with a header only: an empty IDAT, then IEND.
    fn header_only_png(w: u32, h: u32) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = !0u32;
            for &b in bytes {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
                }
            }
            !crc
        }
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend(w.to_be_bytes());
        ihdr.extend(h.to_be_bytes());
        ihdr.extend([8, 2, 0, 0, 0]); // 8 bits, RGB, deflate, no filter, not interlaced
        for (kind, data) in [(b"IHDR", ihdr), (b"IDAT", vec![]), (b"IEND", vec![])] {
            png.extend((data.len() as u32).to_be_bytes());
            let start = png.len();
            png.extend(kind);
            png.extend(&data);
            let crc = crc32(&png[start..]);
            png.extend(crc.to_be_bytes());
        }
        png
    }

    /// An 8 x 8 baseline JPEG whose frame header (SOF0) claims `w` x `h` pixels.
    fn jpeg_claiming(w: u16, h: u16) -> Vec<u8> {
        let mut jpg = Vec::new();
        RgbImage::new(8, 8)
            .write_to(&mut std::io::Cursor::new(&mut jpg), ImageFormat::Jpeg)
            .unwrap();
        let sof = jpg.windows(2).position(|m| m == [0xFF, 0xC0]).unwrap();
        // Marker, length, precision, then height and width.
        jpg[sof + 5..sof + 7].copy_from_slice(&h.to_be_bytes());
        jpg[sof + 7..sof + 9].copy_from_slice(&w.to_be_bytes());
        jpg
    }

    #[test]
    fn an_image_past_the_limits_is_refused_before_decoding() {
        let dir = tempfile::tempdir().unwrap();
        // 100,000 x 100,000 would need 30 GB; 25,000 x 100 is small but too wide.
        for (w, h) in [(100_000, 100_000), (25_000, 100)] {
            let path = dir.path().join(format!("{w}x{h}.png"));
            std::fs::write(&path, header_only_png(w, h)).unwrap();
            for e in [image_size(&path).unwrap_err(), decode(&path).unwrap_err()] {
                assert!(e.to_string().contains("too large"), "{w}x{h}: {e}");
            }
        }
        // 20,000 x 20,000 is within the sides but 1.2 GB decoded; the JPEG decoder does not
        // check `max_alloc` itself.
        let path = dir.path().join("big.jpg");
        std::fs::write(&path, jpeg_claiming(20_000, 20_000)).unwrap();
        for e in [image_size(&path).unwrap_err(), decode(&path).unwrap_err()] {
            assert!(e.to_string().contains("too large"), "jpeg: {e}");
        }
        // Within the limits the same header is read (and only the missing pixels fail).
        let path = dir.path().join("small.png");
        std::fs::write(&path, header_only_png(300, 200)).unwrap();
        assert_eq!(image_size(&path).unwrap(), (300, 200));
        assert!(!decode(&path).unwrap_err().to_string().contains("too large"));
    }

    #[test]
    fn a_quarter_turn_swaps_the_sides() {
        assert!(turns(Orientation::Rotate90));
        assert!(turns(Orientation::Rotate270FlipH));
        assert!(!turns(Orientation::Rotate180));
        assert!(!turns(Orientation::FlipHorizontal));
    }
}

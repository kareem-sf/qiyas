//! A page as pixels.

use std::path::Path;
use std::sync::mpsc::channel;

use image::RgbaImage;
use pdfium_render::prelude::{PdfBitmap, PdfBitmapFormat, PdfColor, PdfPage, PdfRenderConfig};

use crate::{open, page_error, page_index, worker, PdfError, Result};

/// Longest side of a rendered page, in pixels: a page asked for at a higher resolution is drawn
/// at this size (8192 px is 256 MiB of RGBA).
pub const MAX_SIDE: u32 = 8192;

/// Bands a page is drawn in, so `on_progress` can follow a slow page.
const BANDS: u32 = 8;

/// Draws page `page` (from 1) of the PDF at `path` at `dpi` dots per inch on white, as shown
/// (crop box, `/Rotate` applied), no bigger than `MAX_SIDE` on its long side. `on_progress` hears
/// 0 to 100 as the page is drawn (on the calling thread).
pub fn render_page(
    path: &Path,
    page: u32,
    dpi: f64,
    mut on_progress: impl FnMut(u8),
) -> Result<RgbaImage> {
    let path = path.to_path_buf();
    let (progress, heard) = channel::<u8>();
    let (answer, answered) = channel::<Result<RgbaImage>>();
    let job = Box::new(move |pdfium: &pdfium_render::prelude::Pdfium| {
        let result = (|| {
            let doc = open(pdfium, &path)?;
            let index = page_index(&doc, page)?;
            let size = doc.pages().page_size(index).map_err(page_error)?;
            let (w, h) = pixel_size(
                f64::from(size.width().value),
                f64::from(size.height().value),
                dpi,
            );
            let page = doc.pages().get(index).map_err(page_error)?;
            render_on_worker(&page, w, h, &mut |p| {
                let _ = progress.send(p);
            })
        })();
        let _ = answer.send(result);
    });
    worker()?.send(job).map_err(|_| PdfError::NotInstalled)?;
    // Progress until the job ends (dropping its sender) or answers.
    for p in heard {
        on_progress(p);
    }
    answered
        .recv()
        .unwrap_or_else(|_| Err(PdfError::Page("pdfium stopped on this page".to_string())))
}

/// Pixels for a page of `w` x `h` points at `dpi`, at least 1 and at most `MAX_SIDE` on the long
/// side (shape kept).
fn pixel_size(w: f64, h: f64, dpi: f64) -> (u32, u32) {
    let dpi = if dpi.is_finite() && dpi > 0.0 {
        dpi
    } else {
        72.0
    };
    let fit = (f64::from(MAX_SIDE) / (w.max(h) / 72.0 * dpi)).min(1.0);
    let px = |v: f64| ((v / 72.0 * dpi * fit).round() as u32).clamp(1, MAX_SIDE);
    (px(w), px(h))
}

/// Draws `page` into a `w` x `h` RGBA image on white, a band at a time, telling `progress` 0 to
/// 100. Runs on the pdfium thread.
pub(crate) fn render_on_worker(
    page: &PdfPage,
    w: u32,
    h: u32,
    progress: &mut dyn FnMut(u8),
) -> Result<RgbaImage> {
    let mut bitmap =
        PdfBitmap::empty(w as i32, h as i32, PdfBitmapFormat::BGRA).map_err(page_error)?;
    progress(0);
    let band = h.div_ceil(BANDS).max(1);
    let mut top = 0;
    while top < h {
        let bottom = (top + band).min(h);
        let config = PdfRenderConfig::new()
            .set_target_size(w as i32, h as i32)
            .set_clear_color(PdfColor::WHITE)
            // The whole bitmap is cleared before the first band only.
            .clear_before_rendering(top == 0)
            .render_annotations(true)
            .clip(0, top as i32, w as i32, bottom as i32);
        page.render_into_bitmap_with_config(&mut bitmap, &config)
            .map_err(page_error)?;
        top = bottom;
        progress((u64::from(top) * 100 / u64::from(h)) as u8);
    }
    // RGBA already (pdfium-render asks for reversed byte order by default); rows may be padded.
    let raw = bitmap.as_rgba_bytes();
    let stride = raw.len() / h as usize;
    let row = (w * 4) as usize;
    let rgba = if stride == row {
        raw
    } else {
        raw.chunks_exact(stride)
            .flat_map(|r| &r[..row])
            .copied()
            .collect()
    };
    RgbaImage::from_raw(w, h, rgba).ok_or_else(|| PdfError::Page("bitmap size".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_size_keeps_the_shape_and_the_limit() {
        // A4 at 72 dpi is its size in points.
        assert_eq!(pixel_size(595.0, 842.0, 72.0), (595, 842));
        assert_eq!(pixel_size(100.0, 50.0, 144.0), (200, 100));
        // A0 at 600 dpi would be 19866 px high: drawn 8192 high, same shape.
        let (w, h) = pixel_size(2384.0, 3370.0, 600.0);
        assert_eq!(h, MAX_SIDE);
        assert!((f64::from(w) / f64::from(h) - 2384.0 / 3370.0).abs() < 1e-3);
        assert_eq!(pixel_size(10.0, 10.0, f64::NAN), (10, 10));
    }
}

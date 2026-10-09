//! IPC for PDFs: page count and size, thumbnails, attaching a page (underlay or import),
//! calibrating an underlay, and an underlay's picture. pdfium runs on its own thread
//! (`qiyas_pdf`); reading a PDF here never holds the session lock: a page is read first, then the
//! session is locked briefly to add it. Without pdfium every command answers "PDF support is not installed: run
//! scripts/pdfium.sh".

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError};

use qiyas_core::{AttachMode, CoreError, PreparedPdf, UnderlayImage};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::commands::with_session;
use crate::state::AppState;

/// Event with the percent drawn of the underlay picture being rendered.
pub const PROGRESS: &str = "pdf:progress";
/// Pictures kept, most recently used last.
const KEPT: usize = 4;
/// Thumbnails asked for at once, at most.
const MAX_THUMBNAILS: u32 = 40;

/// The last `KEPT` underlay pictures used, by image id, version and resolution, so zooming back and
/// forth does not draw a page again; the least recently used goes first.
#[derive(Default)]
pub struct PictureCache(VecDeque<(PictureKey, Arc<Vec<u8>>)>);

impl PictureCache {
    /// The picture under `key`, now the most recently used.
    fn get(&mut self, key: PictureKey) -> Option<Arc<Vec<u8>>> {
        let at = self.0.iter().position(|(k, _)| *k == key)?;
        let entry = self.0.remove(at)?;
        let bytes = entry.1.clone();
        self.0.push_back(entry);
        Some(bytes)
    }

    /// Keeps `bytes` under `key`, letting go of the least recently used beyond `KEPT`.
    fn put(&mut self, key: PictureKey, bytes: Arc<Vec<u8>>) {
        self.0.retain(|(k, _)| *k != key);
        while self.0.len() >= KEPT {
            self.0.pop_front();
        }
        self.0.push_back((key, bytes));
    }
}

/// Image id, version, and the resolution (`f64::to_bits` of the dpi).
type PictureKey = (u32, u64, u64);

/// Where the bundled pdfium is: `resources/pdfium/` in the app's resources (next to the program
/// on Windows), or the crate's own `resources/pdfium/` in a debug build.
pub fn use_bundled_pdfium<R: Runtime>(app: &AppHandle<R>) {
    let dir = if cfg!(debug_assertions) {
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium"))
    } else {
        app.path()
            .resource_dir()
            .ok()
            .map(|r| r.join("resources/pdfium"))
    };
    if let Some(dir) = dir {
        qiyas_pdf::set_library_dir(dir);
    }
}

/// Runs blocking PDF work off the async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| "Reading the PDF stopped unexpectedly".to_string())?
}

/// How many pages the PDF at `path` has.
#[tauri::command]
pub async fn pdf_page_count(path: String) -> Result<u32, String> {
    blocking(move || qiyas_pdf::page_count(Path::new(&path)).map_err(|e| e.to_string())).await
}

/// Width and height in points of page `page` (from 1), as shown.
#[tauri::command]
pub async fn pdf_page_size(path: String, page: u32) -> Result<(f64, f64), String> {
    blocking(move || qiyas_pdf::page_size(Path::new(&path), page).map_err(|e| e.to_string())).await
}

/// Pages `first` to `first + count - 1` (from 1, cut at the last page, 40 at most) drawn to fit
/// `px` x `px`. Little-endian: `u32` n, then per page `u32` page number, `u32` width, `u32`
/// height and width x height x 4 RGBA bytes, rows from the top.
#[tauri::command]
pub async fn pdf_thumbnails(
    path: String,
    first: u32,
    count: u32,
    px: u32,
) -> Result<Response, String> {
    blocking(move || {
        let path = PathBuf::from(path);
        let pages = qiyas_pdf::page_count(&path).map_err(|e| e.to_string())?;
        let first = first.max(1);
        let last = (first + count.min(MAX_THUMBNAILS)).min(pages + 1);
        let numbers: Vec<u32> = (first..last).collect();
        let images = qiyas_pdf::thumbnails(&path, &numbers, px).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        out.extend((images.len() as u32).to_le_bytes());
        for (page, image) in numbers.iter().zip(&images) {
            for v in [*page, image.width(), image.height()] {
                out.extend(v.to_le_bytes());
            }
            out.extend(image.as_raw());
        }
        Ok(Response::new(out))
    })
    .await
}

/// Attaches page `page` of the PDF at `path` to the sheet on screen at (`x`, `y`), `scale`
/// drawing units per point: `mode` "underlay" (returns the underlay's handle) or "import"
/// (returns how many entities were added). One undo step.
#[tauri::command]
pub async fn attach_pdf(
    path: String,
    page: u32,
    x: f64,
    y: f64,
    scale: f64,
    mode: AttachMode,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    // The slow part, reading the page, without the session.
    let prepared = blocking(move || {
        PreparedPdf::read(Path::new(&path), page, scale, mode).map_err(|e| e.to_string())
    })
    .await?;
    let handles = with_session(&state, |s| s.apply_pdf(prepared, [x, y]))?;
    Ok(match mode {
        AttachMode::Underlay => handles.first().copied().unwrap_or(0),
        AttachMode::Import => handles.len() as u64,
    })
}

/// True when the drawing itself has changes not saved yet (PDF imports, underlays, calibration).
#[tauri::command]
pub async fn is_dirty(state: State<'_, AppState>) -> Result<bool, String> {
    with_session(&state, |s| Ok(s.is_dirty()))
}

/// Rescales an underlay (or image) about the first point so that the two points are `distance`
/// drawing units apart. `handle` left out: the underlay under the first point. One undo step.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn calibrate(
    handle: Option<u64>,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    distance: f64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_session(&state, |s| {
        let handle = match handle {
            Some(h) => h,
            None => s.underlay_at([x1, y1]).ok_or_else(|| {
                CoreError::InvalidCommand("Pick the first point on a PDF underlay".to_string())
            })?,
        };
        s.calibrate(handle, [x1, y1], [x2, y2], distance)
    })
}

/// The picture of underlay image `id` (from the batch's image table) for a zoom of
/// `px_per_unit` screen pixels per drawing unit: `u32` width, `u32` height, then width x height
/// x 4 RGBA bytes, rows from the top. Sends `pdf:progress` (0 to 100) while pdfium draws it.
#[tauri::command]
pub async fn underlay_image<R: Runtime>(
    id: u32,
    px_per_unit: f64,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    let UnderlayImage {
        path,
        page,
        dpi,
        version,
    } = with_session(&state, |s| s.underlay_image(id, px_per_unit))?;
    let key = (id, version, dpi.to_bits());
    if let Some(bytes) = pictures(&state).get(key) {
        return Ok(Response::new(bytes.as_ref().clone()));
    }
    let emitter = app.clone();
    let bytes = blocking(move || {
        let image = qiyas_pdf::render_page(&path, page, dpi, |p| {
            let _ = emitter.emit(PROGRESS, p);
        })
        .map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(8 + image.as_raw().len());
        out.extend(image.width().to_le_bytes());
        out.extend(image.height().to_le_bytes());
        out.extend(image.as_raw());
        Ok(out)
    })
    .await?;
    let bytes = Arc::new(bytes);
    pictures(&app.state::<AppState>()).put(key, bytes.clone());
    Ok(Response::new(bytes.as_ref().clone()))
}

fn pictures(state: &AppState) -> std::sync::MutexGuard<'_, PictureCache> {
    state
        .pictures
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_keeps_the_four_most_recently_used_pictures() {
        let mut cache = PictureCache::default();
        let key = |id| (id, 1, 0);
        for id in 0..4 {
            cache.put(key(id), Arc::new(vec![id as u8]));
        }
        // Using picture 0 makes picture 1 the oldest.
        assert_eq!(cache.get(key(0)).as_deref(), Some(&vec![0]));
        cache.put(key(4), Arc::new(vec![4]));
        assert!(cache.get(key(1)).is_none());
        for id in [0, 2, 3, 4] {
            assert!(cache.get(key(id)).is_some(), "{id}");
        }
        assert_eq!(cache.0.len(), KEPT);
        // Putting a key again replaces it.
        cache.put(key(4), Arc::new(vec![9]));
        assert_eq!(cache.get(key(4)).as_deref(), Some(&vec![9]));
        assert_eq!(cache.0.len(), KEPT);
    }
}

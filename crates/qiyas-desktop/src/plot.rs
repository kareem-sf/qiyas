//! IPC for plotting: a sheet to a PDF in the drawing's folder, printing a PDF through the
//! operating system, the paper sizes and printers, and plot style (CTB/STB) files. Output
//! paths chosen in a Save dialog come later; by command a plot stays inside the drawing's
//! folder.

use std::path::Path;

use qiyas_plot::{Paper, PlotJob, PlotOutput, PlotStyle, StyleKind};
use tauri::State;

use crate::commands::with_session;
use crate::state::AppState;

/// The built-in paper sizes, portrait.
#[tauri::command]
pub async fn paper_catalog() -> Vec<Paper> {
    qiyas_plot::paper_catalog()
}

/// Plots `job` to a PDF in the drawing's folder (`job.output` a name inside it, or none for
/// `<drawing>-<sheet>.pdf` beside the drawing); the file and what was left out. The session is
/// held only while the plot's data is read; the PDF is written on a blocking thread.
#[tauri::command]
pub async fn plot_pdf(job: PlotJob, state: State<'_, AppState>) -> Result<PlotOutput, String> {
    let (path, snapshot) = with_session(&state, |s| {
        Ok(qiyas_plot::output_path(s, &job)
            .and_then(|path| Ok((path, qiyas_plot::plot_snapshot(s, &job)?))))
    })?
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        qiyas_plot::write_pdf(&snapshot, &path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Plots `job` to a PDF in the temporary folder and prints it on `printer` (the default one
/// when none): quick print, and print all one sheet at a time. What happened, for the status
/// bar.
#[tauri::command]
pub async fn plot_and_print(
    job: PlotJob,
    printer: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (path, snapshot) = with_session(&state, |s| {
        Ok(qiyas_plot::spool_path(s, &job.sheet)
            .and_then(|path| Ok((path, qiyas_plot::plot_snapshot(s, &job)?))))
    })?
    .map_err(|e| e.to_string())?;
    // The PDF is written and printed without the session locked.
    tauri::async_runtime::spawn_blocking(move || {
        let out = qiyas_plot::write_pdf(&snapshot, &path).map_err(|e| e.to_string())?;
        qiyas_plot::print_pdf(&out.path, printer.as_deref()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Prints the PDF at `path` on `printer` (the default one when none).
#[tauri::command]
pub async fn print_pdf(path: String, printer: Option<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        qiyas_plot::print_pdf(Path::new(&path), printer.as_deref()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The printers this computer knows.
#[tauri::command]
pub async fn list_printers() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(qiyas_plot::list_printers)
        .await
        .map_err(|e| e.to_string())
}

/// The default printer, if any.
#[tauri::command]
pub async fn default_printer() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(qiyas_plot::default_printer)
        .await
        .map_err(|e| e.to_string())
}

/// Reads a CTB or STB plot style file.
#[tauri::command]
pub async fn load_plot_style(path: String) -> Result<PlotStyle, String> {
    PlotStyle::load(Path::new(&path)).map_err(|e| e.to_string())
}

/// Writes `style` to `path`, a `.ctb` file for a CTB or `.stb` for an STB.
#[tauri::command]
pub async fn save_plot_style(path: String, style: PlotStyle) -> Result<(), String> {
    let path = Path::new(&path);
    let want = match style.kind {
        StyleKind::Ctb => "ctb",
        StyleKind::Stb => "stb",
    };
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(want))
    {
        return Err(format!("Save this plot style as a .{want} file"));
    }
    style.save(path).map_err(|e| e.to_string())
}

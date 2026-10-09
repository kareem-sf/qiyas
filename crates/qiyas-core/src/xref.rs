//! External references (xrefs): other drawings a drawing shows inside itself. Each is found on
//! disk, opened read-only and drawn, measured, counted and searched with the host; nothing is
//! ever written into an xref's file.
//!
//! Where an xref is looked for, first found wins: the file the user chose for it (the project's
//! `xref_overrides`), the path stored in the drawing as it is (when absolute) or relative to the
//! folder of the drawing that references it, then the same file name in that folder and in its
//! `xrefs/` and `x-refs/` subfolders. Xrefs inside xrefs are opened too, `MAX_XREF_DEPTH` levels
//! down; an unloaded xref is listed and not opened, an overlay inside an xref is left out (as
//! AutoCAD does), and a name already loaded is not loaded twice (the first one found wins).

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use opencadcodec::tables::normalize_name;
use qiyas_geom::{XrefDocs, MAX_XREF_DEPTH};
use serde::Serialize;

use crate::{CoreError, Drawing, LayerInfo};

/// One xref of the host drawing (or of an xref inside it).
pub struct Xref {
    /// The xref's block name, e.g. `plan-arch`; its layers show as `plan-arch|LAYER`.
    pub name: String,
    /// The path the referencing drawing stores, as written.
    pub path_in_file: String,
    /// Every place looked, in order.
    pub path_tried: Vec<PathBuf>,
    /// The file opened; `None` when none of `path_tried` exists (or the xref is unloaded).
    pub found: Option<PathBuf>,
    pub overlay: bool,
    /// Unloaded in the drawing: listed, not opened.
    pub unloaded: bool,
    /// The xref drawing; `None` when it is missing, unloaded or could not be read.
    pub drawing: Option<Drawing>,
    /// What went wrong finding or reading it, worded for the host's problem list.
    pub problems: Vec<String>,
    /// When `found` was last changed, read as it was opened.
    pub modified: Option<SystemTime>,
    /// 1 for an xref of the host drawing, 2 for one inside that, and so on.
    pub depth: usize,
}

/// The xrefs of a drawing, those inside them following, in the order they were found.
#[derive(Default)]
pub struct XrefSet {
    pub entries: Vec<Xref>,
}

/// What the window lists about one xref.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct XrefInfo {
    pub name: String,
    /// The file was found.
    pub found: bool,
    /// The file opened, or the path stored in the drawing when none was found.
    pub path: String,
    pub overlay: bool,
    pub unloaded: bool,
    /// Layers it adds to the layer list (its own, less layer "0").
    pub layer_count: usize,
    /// What went wrong finding or reading it (also in the drawing's problems).
    pub problems: Vec<String>,
}

impl Drawing {
    /// Opens the drawing at `path` and its xrefs, `depth` levels down (at most `MAX_XREF_DEPTH`).
    /// A missing or unreadable xref is listed with its problem; the drawing still opens.
    pub fn open_with_xrefs(path: &Path, depth: usize) -> Result<(Drawing, XrefSet), CoreError> {
        let drawing = Drawing::open(path)?;
        let xrefs = XrefSet::load(&drawing, &HashMap::new(), depth);
        Ok((drawing, xrefs))
    }
}

impl XrefSet {
    /// Finds and opens the xrefs of `host`, `depth` levels down (at most `MAX_XREF_DEPTH`).
    /// `overrides` maps an xref name (any case) to the file the user chose for it.
    pub fn load(host: &Drawing, overrides: &HashMap<String, String>, depth: usize) -> XrefSet {
        let depth = depth.min(MAX_XREF_DEPTH);
        let host_file = std::fs::canonicalize(&host.path).ok();
        let mut seen = HashSet::new();
        let mut set = XrefSet::default();
        if depth == 0 {
            return set;
        }
        for (name, path, overlay, unloaded) in references(host, false, &mut seen) {
            let folder = folder_of(&host.path);
            set.entries.push(resolve(
                name,
                path,
                overlay,
                unloaded,
                &folder,
                overrides,
                1,
                host_file.as_deref(),
            ));
        }
        let mut next = 0;
        while next < set.entries.len() {
            let i = next;
            next += 1;
            let level = set.entries[i].depth;
            if level >= depth {
                continue;
            }
            let Some(drawing) = &set.entries[i].drawing else {
                continue;
            };
            let folder = folder_of(&drawing.path);
            let inner = references(drawing, true, &mut seen);
            for (name, path, overlay, unloaded) in inner {
                let xref = resolve(
                    name,
                    path,
                    overlay,
                    unloaded,
                    &folder,
                    overrides,
                    level + 1,
                    host_file.as_deref(),
                );
                set.entries.push(xref);
            }
        }
        set
    }

    /// The xref named `name` (any case).
    pub fn get(&self, name: &str) -> Option<&Xref> {
        let key = normalize_name(name);
        self.entries.iter().find(|x| normalize_name(&x.name) == key)
    }

    /// The opened xref drawings, by upper-case name, for drawing and measuring.
    pub fn docs(&self) -> XrefDocs<'_> {
        self.entries
            .iter()
            .filter_map(|x| Some((normalize_name(&x.name), &x.drawing.as_ref()?.doc)))
            .collect()
    }

    /// What the window lists, in load order.
    pub fn info(&self) -> Vec<XrefInfo> {
        self.entries
            .iter()
            .map(|x| XrefInfo {
                name: x.name.clone(),
                found: x.found.is_some(),
                path: x
                    .found
                    .as_ref()
                    .map_or_else(|| x.path_in_file.clone(), |p| p.display().to_string()),
                overlay: x.overlay,
                unloaded: x.unloaded,
                layer_count: x
                    .drawing
                    .as_ref()
                    .map_or(0, |d| d.doc.layers.iter().filter(|l| l.name != "0").count()),
                problems: x.problems.clone(),
            })
            .collect()
    }

    /// The layers the opened xrefs add, named `XREF|LAYER` with the xref's colour and state.
    /// Layer "0" of an xref is not one: what is on it takes the xref reference's layer.
    pub fn layers(&self) -> Vec<LayerInfo> {
        self.entries
            .iter()
            .filter_map(|x| Some((x, x.drawing.as_ref()?)))
            .flat_map(|(x, d)| {
                d.layers()
                    .into_iter()
                    .filter(|l| l.name != "0")
                    .map(move |l| LayerInfo {
                        name: format!("{}|{}", x.name, l.name),
                        ..l
                    })
            })
            .collect()
    }

    /// Every xref's problems, in load order.
    pub fn problems(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().flat_map(|x| &x.problems)
    }
}

/// The xref block records of `drawing` not seen yet: name, stored path, overlay, unloaded.
/// `nested` leaves overlays out.
fn references(
    drawing: &Drawing,
    nested: bool,
    seen: &mut HashSet<String>,
) -> Vec<(String, String, bool, bool)> {
    drawing
        .doc
        .block_records
        .iter()
        .filter(|b| b.flags.is_xref || b.flags.is_xref_overlay)
        .filter(|b| !(nested && b.flags.is_xref_overlay))
        .filter(|b| seen.insert(normalize_name(&b.name)))
        .map(|b| {
            (
                b.name.clone(),
                b.xref_path.clone(),
                b.flags.is_xref_overlay,
                b.flags.is_xref_unloaded,
            )
        })
        .collect()
}

/// Looks for one xref and opens it.
#[allow(clippy::too_many_arguments)]
fn resolve(
    name: String,
    path_in_file: String,
    overlay: bool,
    unloaded: bool,
    folder: &Path,
    overrides: &HashMap<String, String>,
    depth: usize,
    host_file: Option<&Path>,
) -> Xref {
    let mut xref = Xref {
        name,
        path_in_file,
        path_tried: Vec::new(),
        found: None,
        overlay,
        unloaded,
        drawing: None,
        problems: Vec::new(),
        modified: None,
        depth,
    };
    if unloaded {
        return xref;
    }
    let key = normalize_name(&xref.name);
    let chosen = overrides
        .iter()
        .find(|(n, _)| normalize_name(n) == key)
        .map(|(_, p)| PathBuf::from(p));
    xref.path_tried = candidates(folder, &xref.path_in_file, chosen.as_deref());
    xref.found = xref.path_tried.iter().find(|p| p.is_file()).cloned();
    let Some(found) = xref.found.clone() else {
        let tried: Vec<String> = xref
            .path_tried
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        xref.problems.push(format!(
            "Xref {} not found; tried: {}",
            xref.name,
            if tried.is_empty() {
                "no path".to_string()
            } else {
                tried.join(", ")
            }
        ));
        return xref;
    };
    if host_file.is_some() && std::fs::canonicalize(&found).ok().as_deref() == host_file {
        xref.problems.push(format!(
            "Xref {} is the drawing itself; not drawn again",
            xref.name
        ));
        return xref;
    }
    xref.modified = std::fs::metadata(&found).and_then(|m| m.modified()).ok();
    match Drawing::open(&found) {
        Ok(drawing) => {
            xref.problems.extend(
                drawing
                    .problems
                    .iter()
                    .map(|p| format!("Xref {}: {p}", xref.name)),
            );
            xref.drawing = Some(drawing);
        }
        Err(e) => xref
            .problems
            .push(format!("Xref {} could not be read: {e}", xref.name)),
    }
    xref
}

/// Where to look for an xref stored as `path_in_file` by a drawing in `folder`, in order (see the
/// module notes), with the user's `chosen` file first. Backslashes count as folder separators, so
/// a path written on Windows is found on any system.
pub fn candidates(folder: &Path, path_in_file: &str, chosen: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = chosen.map(Path::to_path_buf).into_iter().collect();
    let stored = path_in_file.trim().replace('\\', "/");
    if !stored.is_empty() {
        let as_written = PathBuf::from(&stored);
        if as_written.is_absolute() {
            out.push(as_written);
        } else {
            out.push(tidy(&folder.join(as_written)));
        }
        if let Some(file) = stored.rsplit('/').next().filter(|f| !f.is_empty()) {
            out.push(folder.join(file));
            out.push(folder.join("xrefs").join(file));
            out.push(folder.join("x-refs").join(file));
        }
    }
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// `path` without its `.` parts.
fn tidy(path: &Path) -> PathBuf {
    path.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect()
}

/// The folder a drawing sits in ("." for a bare file name).
fn folder_of(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_follow_the_order() {
        let folder = Path::new("/jobs/villa");
        let tried = candidates(
            folder,
            "..\\shared\\plan-arch.dwg",
            Some(Path::new("/elsewhere/a.dwg")),
        );
        let expected: Vec<PathBuf> = [
            "/elsewhere/a.dwg",
            "/jobs/villa/../shared/plan-arch.dwg",
            "/jobs/villa/plan-arch.dwg",
            "/jobs/villa/xrefs/plan-arch.dwg",
            "/jobs/villa/x-refs/plan-arch.dwg",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(tried, expected);
    }

    #[test]
    fn an_absolute_path_is_tried_as_it_is_and_duplicates_go() {
        let tried = candidates(Path::new("/jobs"), "/jobs/plan.dwg", None);
        assert_eq!(tried[0], PathBuf::from("/jobs/plan.dwg"));
        assert_eq!(tried.len(), 3, "{tried:?}");
        let relative = candidates(Path::new("/jobs"), "./plan.dwg", None);
        assert_eq!(relative[0], PathBuf::from("/jobs/plan.dwg"));
        assert!(candidates(Path::new("/jobs"), "", None).is_empty());
    }
}

# Qiyas Phase 1b: Drawing Input and Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qiyas opens a real project the way a QS receives it (DWG sets with xrefs, PDFs, scanned images), draws real text, picks inside blocks, snaps, and has the navigation and inquiry tools of the spec's Phase 1 that the foundation left out.

**Architecture:** The batch grows a version field, per-segment sub-entity paths, per-path layer and colour from inside blocks, textured quads (glyphs and images), and arc-exact measurement. Xrefs and PDF/image underlays are separate `Drawing`s or textures composed into the same batch with their own transforms. Snaps come from `opencadkernel::geom2d::snap` over the entities near the cursor. Everything stays behind the command bus so the AI gets it for free.

**Tech Stack:** opencadkernel (snap, intersect, arclength, tessellate), rustybuzz 0.20 + ab_glyph 0.2 (glyph atlas), pdfium-render 0.9 with the pdfium binary fetched by `scripts/pdfium.sh` into a git-ignored folder and bundled as a Tauri resource, image 0.25, tiny-skia.

**Spec:** `docs/superpowers/specs/2026-10-08-qiyas-design.md`. Carried items: `docs/2026-10-08-qiyas-foundation-deferred.md` (every "Phase 1b" line is a requirement here).

## Global Constraints

- Everything in the foundation plan's Global Constraints.
- Batch byte layout version 2 starts with `u32 magic 0x51495942 ("QIYB")`, `u32 version = 2`; the TS parser rejects other versions with a clear error. Sections stay 8/4-aligned.
- Fonts for text inside drawings: Noto Sans (OFL) and Noto Sans Arabic (OFL) bundled under `src-tauri` resources and `src/fonts/noto/`; SHX and missing TrueType fonts fall back to them. Thmanyah stays UI-only.
- pdfium binary: downloaded by `scripts/pdfium.sh` from the `bblanchon/pdfium-binaries` release matching the platform, placed in `crates/qiyas-desktop/resources/pdfium/` (git-ignored), licence BSD-3 kept in `LICENSES/`.
- Xref and underlay files are opened read-only; nothing is ever written to them.
- Arc, circle and bulge lengths and areas are exact (analytic), not chord sums.

## Review Focus

1. A bound-xref drawing where the whole plan is one block: clicking a wall must measure that wall's length, and the take-off row must name the block path. Task 1.
2. A missing xref file: the drawing must open, the xref manager must list it as missing with the path tried, and nothing else must break. Task 5.
3. A scanned plan attached as an image and calibrated with two points 10 m apart: a measured distance between the same two points must read 10.000 m within 0.5 percent. Task 7.
4. Arabic text in a drawing stored as reversed glyph order (old AutoCAD habit): it must render readable, with a per-drawing toggle "Arabic text is reversed" when the heuristic is wrong. Task 3.
5. A PDF with 2,000 pages or a 200 MB scan: attach must stream one page at a time and never read the whole file into memory; the UI must stay responsive (progress events). Task 6.

---

### Task 1: Pick inside blocks, nested layers and colours, OCS, arc-exact measures

**Files:**
- Modify: `crates/qiyas-geom/src/flatten.rs` (emit `FlatPath { path, layer: String, color: Color, chain: Vec<u64> /* insert handles down to the entity */, entity: u64, normal_flipped: bool }`; apply the extrusion normal (`Matrix4::arbitrary_axis`) so mirrored blocks flatten right; attribute text boxes), `crates/qiyas-geom/src/exact.rs` (new: `exact_length(entity) -> Option<f64>`, `exact_area(entity) -> Option<f64>` for Line, LwPolyline with bulges, Polyline, Arc, Circle, Ellipse; falls back to flatten otherwise), `crates/qiyas-render/src/batch.rs` (ranges become `(top: u64, entity: u64, first, count)` and a `chains` table; hidden layers and colours apply to the inner entity's layer; layout v2 with magic/version), `src/viewport/batch.ts`, `crates/qiyas-core/src/session.rs` (`pick` returns `Picked { top: u64, entity: u64, chain: Vec<u64> }`; `entity_info` and `MeasureEntity` accept a chain and measure the inner entity in world coordinates with the accumulated transform), `src/api.ts`, `src/tools/useTool.ts` (select measures the inner entity; a second click toggles between the inner entity and the whole block, shown in the status bar), `crates/qiyas-core/src/takeoff.rs` (`Measurement.chain: Vec<u64>`), `crates/qiyas-core/src/embed.rs` (chain stored in xdata as a comma list)
- Tests: geom (nested insert yields chain and inner layer; mirrored normal; exact circle area = πr² to 1e-9), render (hidden inner layer removes only those segments; v2 header), core (pick inside the DOOR block returns the leaf line; MeasureEntity on it gives 0.9 × scale), TS parser (rejects version 1 buffers).

- [ ] Tests first, implement, `scripts/check.sh`, commit.

---

### Task 2: Shared layer-visibility helper and batch etag

**Files:** `crates/qiyas-render/src/visibility.rs` (one `is_visible(doc, layer, vis)`), remove the copy in core; `Session::batch_version() -> u64` (hash of sheet, hidden set, document revision counter incremented by every mutating command); IPC `render_batch` takes `if_version: Option<u64>` and returns `NotModified` so the frontend keeps its buffers.
- [ ] Tests: layer click without a change does not re-send; a command bumps the version. Commit.

---

### Task 3: Text rendering with a glyph atlas

**Files:**
- Create: `crates/qiyas-render/src/text.rs` (shaping with rustybuzz: Latin, Arabic with `arab` script and RTL, numerals; font fallback chain: drawing's TrueType name if found in the system → Noto Sans → Noto Sans Arabic; SHX names map to Noto), `crates/qiyas-render/src/atlas.rs` (ab_glyph rasterisation into a 2048² R8 atlas page, grown by pages; glyph cache keyed by font, glyph id, size bucket), batch section `quads` (`f32 ×8` positions per quad relative to origin, `u16 ×4` uv, `u32` colour, `u32` kind: 1 glyph, 2 image) and an `atlas` section (page index, width, height, bytes); `src/viewport/gl.ts` (second program for textured quads, atlas texture upload, SDF-free alpha), `src/viewport/batch.ts`
- Modify: text flattening keeps the pick box; MText formatting codes stripped; attributes rendered; the "Arabic text is reversed" per-drawing toggle (Session + project file) applies a reversal before shaping when the text has no shaped forms and reads reversed by the heuristic (isolated-form characters in visual order).
- Tests: shaping "قياس" yields 4 glyphs with Arabic forms; reversed input renders the same glyph string after the toggle; a 10,000-text drawing builds its atlas in under 300 ms release (bench example).

- [ ] Tests, implement, commit.

---

### Task 4: Snaps, tracking, grid, drawing units

**Files:**
- Create: `crates/qiyas-core/src/snap.rs` (`Session::snap(&self, p, pixel_tol_world, modes: SnapModes) -> Option<Snap { point, kind: End|Mid|Centre|Node|Intersection|Nearest|Perpendicular, handle }>` using the pick index to shortlist entities and `opencadkernel::geom2d::snap::{characteristic_points, nearest_to, perpendicular_from}` and `intersect` for crossings), `src/viewport/snapMarker.ts` (marker glyph per kind drawn in the overlay), `src/shell/DraftingSettings.tsx` (snap modes checkboxes, ortho F8, polar F10 with 45° steps, grid F7 with spacing and visibility, units display precision), status bar toggles
- Modify: `useTool` asks for a snap on every pointer move (debounced to one IPC per frame) and uses the snapped point on click; distance/area tools honour ortho and polar; `Viewport` draws the grid as a shader-generated pattern.
- Tests: snap to the rectangle's corner from 0.05 away returns End at (4,3); midpoint; intersection of the line and the rectangle edge; ortho locks the second point's y.

- [ ] Tests, implement, commit.

---

### Task 5: Xrefs

**Files:**
- Create: `crates/qiyas-core/src/xref.rs` (`XrefSet { entries: Vec<Xref { name, path_tried: PathBuf, found: bool, drawing: Option<Drawing>, overlay: bool }> }`, resolution order: absolute path, relative to the host drawing, same folder by file name; `Drawing::open_with_xrefs(path, depth ≤ 4)`), `src/shell/XrefManager.tsx`
- Modify: flatten/render treat an insert of an xref block as the xref drawing's model space under the insert transform; layers appear as `NAME|LAYER` in the layers panel (hide works); counts and finds include xref content; `read_text` includes it; `embed` never writes into xref content; `Session::info()` lists xrefs.
- Tests: host + xref fixtures written to a temp dir; segment count includes the xref; missing file → found false and the path tried; hide `X|A-WALL` hides it.

- [ ] Tests, implement, commit.

---

### Task 6: PDF import and PDF underlay (`qiyas-pdf`)

**Files:**
- Create: `crates/qiyas-pdf/{Cargo.toml, src/lib.rs, src/pages.rs, src/vectors.rs, src/raster.rs}`, `scripts/pdfium.sh`, `src/shell/AttachPdfDialog.tsx`
- Interfaces: `page_count(path)`, `page_size(path, page) -> (w_pt, h_pt)`, `render_page(path, page, dpi, progress) -> RgbaImage`, `extract_vectors(path, page, scale /* drawing units per point */) -> Vec<EntityType>` (path objects → LwPolylines/Lines on layers named by stroke colour `PDF-<rrggbb>` with line weight from stroke width; text objects → Text entities on `PDF-TEXT`; beziers flattened at 0.1 mm), `Session::attach_pdf(path, page, insert_at, scale, as: Underlay|Import)`; underlays are stored as `EntityType::Underlay` + `UnderlayDefinition` so AutoCAD sees them; drawn as a textured quad rendered at the current zoom level (re-rendered when the zoom changes by more than 2×, cached by level); `Session::calibrate(handle, p1, p2, real_distance)` rescales the underlay or image.
- Dialog: file, page picker with thumbnails (first 20 lazily), scale (points → drawing units; presets 1:100 on A1 etc.), insert point by click, Underlay or Import.
- Tests: a PDF written in the test with `printpdf` containing a 100 × 50 pt rectangle and the text "WALL": import yields a closed polyline 100×50 × scale and a Text; page render is an RGBA image of the right size; a 2,000-page synthetic PDF attaches page 1,999 without reading other pages (memory stays under 300 MB measured by `/proc/self/statm` in the test, Linux only).

- [ ] Tests, implement, commit.

---

### Task 7: Image underlay and calibration

**Files:** `crates/qiyas-core/src/image.rs` (`attach_image(path, insert_at, width_units)` → `RasterImage` + `ImageDefinition` entities; decode with `image` crate; downsample to ≤ 4096 px for the texture, keep the file path), `src/shell/AttachImageDialog.tsx` (file, insert point, width, calibrate with two points and a known distance), render: textured quad with brightness/contrast/fade sliders from the entity flags.
- Tests: attach a PNG written in the test; calibrate 10 m between two points → `measure_distance` between them reads 10.000 within 0.5 percent; the DWG saved reopens with the image reference (path relative to the drawing).

- [ ] Tests, implement, commit.

---

### Task 8: Inquiry tools: measure geometry, divide and measure along a path, ID point, data extraction

**Files:** `crates/qiyas-core/src/commands.rs` (`MeasureGeometry {handle}` → radius, diameter, angle, centre for arcs/circles/ellipses as an `Outcome::Info` card; `DivideAlong {handle, count}` and `MeasureAlong {handle, step}` → points placed as `QIYAS-POINT` Point entities and a Count measurement of the points; `IdPoint {p}` → `Outcome::Info`; `ExtractData {filter, fields}` → `Outcome::Table` and an Excel export through `qiyas-boq::export_table`), ribbon Measure tab buttons, an Info card in the status area.
- Tests: circle → radius 1.5, diameter 3, perimeter 2π·1.5 exact; divide a 10 m line into 5 → 4 points at 2 m steps; extract attributes from 3 DOOR inserts → 3 rows with MARK values.

- [ ] Tests, implement, commit.

---

### Task 9: Command line, aliases, properties palette

**Files:** `src/shell/CommandLine.tsx` (bottom input: history ↑/↓, autocomplete, prompts for points `x,y` or clicks, Escape cancels; parses `DI`/`DIST`, `AA`/`AREA`, `COUNT <block>`, `LAYOFF <name>`, `LAYON`, `LAYISO`, `Z E`/`ZOOM E`, `Z W`, `PDFATTACH`, `IMAGEATTACH`, `XREF`, `OPEN`, `SAVE`, `SAVEAS`, `EXPORT`, `U`, `REDO`; command names stay English in Arabic mode), `src/tools/commandParser.ts` (pure, tested), `src/shell/PropertiesPanel.tsx` (shows the picked entity's `EntityInfo` extended with colour, linetype, lineweight, length/area/radius, block name, attributes; read-only this phase), `crates/qiyas-core/src/session.rs` (`entity_info` extended).
- Tests: parser (12 cases incl. Arabic digits normalised), properties panel renders fields.

- [ ] Tests, implement, commit.

---

### Task 10: Named views, recents, file tabs, empty-drawing warning

**Files:** `crates/qiyas-core` (`Session::views()` from `doc.views`, `set_view(name)` returns bounds; `Command::SaveView {name}` writes a `View` table entry), `src/shell/ViewMenu.tsx`, recents in settings (last 10 paths, menu under Open, missing files greyed), multiple open drawings: `AppState` holds `HashMap<u32, Session>` with an `active` id; every IPC command takes an optional `session` id (default active); `src/shell/FileTabs.tsx` above the viewport; closing a tab with unsaved changes asks; a drawing that opens with zero model-space entities shows "This drawing has no geometry in Model" in the status bar.
- Tests: two sessions open with independent take-offs; `set_view` returns the saved bounds.

- [ ] Tests, implement, commit.

---

### Task 11: Viewport polish carried from the foundation

**Files:** `src/viewport/Viewport.tsx`, `gl.ts`, `src/shell/*`: touchpad zoom scaled by `deltaY` magnitude and clamped; DPR refresh via `matchMedia("(resolution)")`; click on pointerup with a 4 px move threshold (drag never clicks); roving tabindex on ribbon and sheet tabs; `.prefs` no longer overlaps tabs below 1100 px; white-layer swatch shows Ink on light; layer-op busy feedback (buttons disabled while busy); Space never activates a just-clicked button (blur after click); keyboard-reachable problems list (a "Problems" button opening a small list).
- Tests: existing suites pass; new tests for the click threshold and the parser of the resolution media query are not needed (manual check with screenshots in both themes and languages).

- [ ] Implement, manual check, commit.

---

### Task 12: Check and hand-over

- [ ] `scripts/check.sh`; bench on the two real drawings (text rendering changes the batch size: record it); open the founder's bound-xref drawing and measure a wall inside the block; attach a PDF and an image from the founder's folder (copies) and calibrate; write `docs/2026-10-08-qiyas-phase1b-check.md`; report. No push.

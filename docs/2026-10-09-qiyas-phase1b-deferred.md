# Qiyas Phase 1b: rulings and items carried forward

Date: 2026-10-09. Source: the build ledger of the Phase 1b plan (`docs/superpowers/plans/2026-10-08-qiyas-phase1b-drawing-io.md`). Check results: `docs/2026-10-09-qiyas-phase1b-check.md`.

## Rulings made during the build
- Batch layout v2 carries a section table (kind, offset, length); later tasks add sections without a version bump; the TS parser ignores unknown kinds. Per-segment handle sections (kinds 1 and 2) were removed before anything built on them.
- `Picked` and `Measurement` carry `xref: Option<String>`; `entity` is the handle inside that drawing; chains list host-side inserts only. Blocks inside an xref measure as a count.
- Batch version hashes sheet, hidden layers, document revision, epoch, xref set and underlay set; the app reads the version then the batch (two IPC calls).
- Glyph atlas pages travel outside the batch through `atlas_pages(since_version)`; section 8 carries the atlas version and zoom hint; section 7 carries glyph quads. Fonts: bundled Noto Sans and Noto Sans Arabic (OFL) as the fallback for every drawing font; system TrueType by file name when found; SHX falls back to Noto. Bidi through `unicode-bidi`; reversed Arabic detected by a join-consistency check and fixable by a toggle.
- opencadkernel pinned at 99b6b48 (geom2d, MPL-2.0, licence notice shipped).
- Underlay and image pixels travel outside the batch: section 10 is image quads, section 11 the image table; `underlay_image(id, px_per_unit)` returns RGBA rendered at the current zoom bucket. pdfium is downloaded by `scripts/pdfium.sh` (pinned tag, SHA-256 checked), bundled as a resource; absent at runtime means PDF features are off with a clear message.
- PDF underlay scale 1 = paper size in INSUNITS (AutoCAD's documented behaviour); unitless drawings count as millimetres. The founder verifies once against a real AutoCAD DWG.
- Dirty flag: the session tracks `document_dirty`; the window asks "Save changes to the drawing?" on close. Open adds a tab and does not prompt.
- One unit everywhere a QS reads numbers: the status bar and info cards show metres with an "m" suffix; `export_table` writes numbers as numeric cells, never formulas.
- Tabs: the existing active session slot stays single; other drawings are parked `Session` values and swapped in on switch; the assistant conversation clears on a switch; no session id on IPC. A Phase 3 split view would need real multi-session state.
- Final review: the pick-index regression was codegen-unit partitioning in `qiyas-render` (fixed with `codegen-units = 1` for that crate in both profiles, guarded by a release timing test).

## Carried to Phase 3
- XCLIP (clipped xrefs) is ignored, so clipped sheet sets over-count. Phase 3 xref task.
- Handle-only AI paths (`add_measurement`, `read_text` rows) can bind an xref handle to a host entity with the same number.
- `extent()` ignores `xref_insert_layers`; an attribute on an insert inside an xref is not hidden with it; an xref count override drops user points.
- Document undo steps clone the whole drawing; Phase 3 edits will make this slow on big files.
- The pick index is rebuilt on every layer toggle (about 0.35 s on a 1.4 M-segment drawing); build it in the background and filter hidden layers at pick time.
- Per-tab camera is not remembered (fit on switch); parked tabs drop their caches and rebuild on activate.
- Images inside blocks are not drawn; only an image's frame is pickable; transparent PNGs ignore alpha; polygonal and inverted clips show the whole image.
- Xrefs inside host blocks are drawn but not counted or searched; Reload and Locate hold the session lock while reading.

## Deferred minors (judged minor, left as is)
- Parser: no 8-byte alignment check of section offsets; NOT_MODIFIED marker constants not cross-checked Rust to TS; `if_version` path unused.
- Geometry: bulge maths duplicated three times; -Z-only normal tests; 255-byte xdata chain strings unguarded; self-crossing entities give no Intersection snap; non-uniformly scaled arcs lose Centre and Mid snaps.
- Text: zoom past the bucket limits re-sends the batch; a changed atlas page re-sends 4 MB; AI pictures draw text as boxes; no mipmaps; marks reversed by plain char order land on the wrong letter; ZWNJ splits visual words; `text.rs` is about 1,000 lines.
- Snaps: shortlist of 8 can crowd out a true End on dense drawings; Perpendicular needs the cursor near the foot; precision clamp 0..6 vs check 2..4; hover marker stale one round trip after leaving a snap; ortho and polar are mutually exclusive through the F keys.
- Underlays: picture hint dropped while busy; command-bus PDF path reads under the lock; EXIF orientation and clip y-direction to verify in AutoCAD; default image width grows with earlier images; brightness and contrast curve guessed.
- Inquiry: measure-along start on mirrored arcs unverified; silent 5,000-row cut in the extract window; step description has no unit; ellipse branch and mm drawings untested.
- Command line: after COUNT or AI the repeat target is cleared; aliases editable only in settings.json until Phase 4; text height in the palette not block-scaled.
- Tabs: canonical recents dedup; empty-model scan on every drawing change; held Space repeats on tabs; blur-after-click removes focus a dialog restores to; problem-list physical padding.
- Arabic: command-line history stays in the typed language; a native speaker should settle the Measure tab label ("القياسات").
- Status bar shows metres while typed points stay in drawing units (Task 9 ruling); worth a note in the help later.
- Images over 512 MB decoded are refused, which also refuses colour scans over about 170 megapixels; an A0 scan at 300 dpi still fits.

## Founder checks (need AutoCAD or a real window)
- PDF underlay size and place against AutoCAD; EXIF orientation; clip direction; xref layer visibility.
- High-DPI switch while running; dialogs and Space on tabs in the real Tauri window.
- Open five drawings as tabs and watch memory.

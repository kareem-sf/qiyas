# Qiyas: the measurement CAD for quantity surveyors

Date: 2026-10-08. Status: draft for the founder's review.

Qiyas (قياس, "measurement") is a desktop CAD app under the QS Mind umbrella. A QS opens the project's drawings, measures and edits them, and an AI staff member measures with them, builds and checks the BOQ and schedules, and reports every conflict and gap it finds.

## Why

Today a QS takes off quantities by hand in AutoCAD or Bluebeam, types them into Excel, and checks the BOQ against the drawings by eye. Drawings disagree with schedules, schedules disagree with the BOQ, and nobody finds out until site. Open CAD Studio proved a full CAD can be built in Rust by one person. Qiyas takes that idea and points it at our users: the drawing is the source of truth, every quantity traces back to geometry you can click, and the AI does the slow part.

Success: a QS opens a real project (DWGs, xrefs, a PDF or two, the tender BOQ in Excel), asks "measure the concrete and check it against the BOQ", and within minutes has a take-off they can audit line by line, plus a list of conflicts and missing items they would have missed. They trust it because every number highlights the geometry behind it.

## Decisions already made

- Desktop only (Windows, Linux, macOS). No web version. Founder, 2026-10-08.
- The QS both edits and measures, so Qiyas is a real 2D CAD, not a viewer. Founder, 2026-10-08.
- Features chosen from the Open CAD Studio list (`docs/research/2026-10-08-opencadstudio-features.md`): 1, 3, 6, 7, 8, 13, 14, 15, all of groups 2 to 8 and 10 to 13 except 86 (web). DXF (2) comes free with DWG. 3D (group 9) is out.
- Stack: Tauri 2, React, Rust core. Same family as Quantix, Tawreed and OpenConKit.
- DWG and DXF reading and writing come from `opencadcodec` (MPL-2.0), geometry from `opencadkernel` (MPL-2.0), both by the Open CAD Studio author and already used by Quantix. We pin revisions. We write everything else ourselves. No GPL code from Open CAD Studio is copied.
- Name: Qiyas, Arabic قياس. Sphere face: anodised titanium, violet.
- AI: Claude via the API, default model `claude-opus-5-5`, plus an MCP server so Claude Desktop and Claude Code can drive Qiyas too.

## What the app is

Four parts, one window:

1. **Drawing.** Open, view, edit, annotate and save DWG and DXF, with xrefs, PDFs and images underneath. The CAD half.
2. **Take-off.** Measurements as first-class objects: length, area, count, volume, item. Each one points at geometry. The take-off panel groups them, totals them, and exports them.
3. **BOQ and schedules.** Import the project BOQ from Excel. Read door, window, finish and rebar schedules from the drawings' tables. Link measurements to BOQ lines. Compare.
4. **AI staff.** An assistant panel that talks in English or Arabic and calls the same commands the user does: measure, count, read tables, build BOQ lines, compare, flag conflicts. Every change it makes is a normal undoable command, highlighted in the drawing.

### Conflicts and gaps (the report)

The comparison engine produces one list, each row with a reason and a "show me" link:

- **Quantity mismatch:** drawing quantity vs BOQ quantity differs by more than a tolerance (default 5 percent, per item).
- **Missing from BOQ:** measured in the drawings, no BOQ line.
- **Not in drawings:** BOQ line with nothing measurable found.
- **Schedule vs plan:** a door schedule says 42 doors, the plans have 39 door blocks.
- **Drawing vs drawing:** the same element differs between sheets (plan vs section height, plan vs elevation count).
- **Unit or description doubts:** the BOQ says m², the measurement is a length; or the AI could not tell which line an item belongs to.

The report lives in a panel and exports to Excel. Rows the QS resolves are marked and kept.

## Architecture

```
qiyas/
  crates/
    qiyas-core      the document: opencadcodec CadDocument in memory, layers, blocks,
                    layouts, xrefs, selection, snaps, units, undo and redo, command bus
    qiyas-geom      2D geometry on top of opencadkernel: offset, trim, fillet, area,
                    length, closed regions, intersections
    qiyas-render    turns the document into GPU buffers: lines, fills, text glyphs,
                    hatches, images; spatial index; dirty tracking per handle
    qiyas-takeoff   measurements, schedules, BOQ model, links, the comparison engine
    qiyas-boq       Excel in and out (calamine to read, rust_xlsxwriter to write)
    qiyas-pdf       PDF import as vectors and text, PDF underlay pages, PDF plot
    qiyas-ai        tool catalogue over the command bus, MCP server, Claude client
    qiyas-desktop   the Tauri app: IPC, file dialogs, printing, keychain, updates
  src/              React + TypeScript + Vite: viewport, ribbon, command line, panels
  scripts/check.sh  cargo test, cargo clippy, tsc, vitest, in one run
```

**One command bus.** Every action is a named command with typed arguments: `line`, `offset`, `measure_area`, `link_boq`, `compare`. The ribbon, the command line, scripts, the MCP server and the AI assistant all go through the same bus. A command returns a result and records an undo entry. This is what makes the AI a real user instead of a bolt-on.

**Data flow.** The document lives in Rust. The React side never holds entities. It holds a view: GPU buffers from `qiyas-render`, the selection, and panel data. After a command, Rust emits "handles changed" and the viewport fetches only those buffers over Tauri's binary channel. Large drawings load once, then redraw only what changed.

**Viewport.** WebGL2 in TypeScript. Lines as instanced segments with widths and linetypes, fills as triangle lists, text as glyph quads from a Rust-side atlas (Arabic shaped with `rustybuzz`), images as textures. Orthographic 2D camera, zoom and pan at 60 frames per second on a 50 MB drawing is the bar.

**Files.** The drawing is the DWG or DXF. Qiyas keeps a `.qiyas` project file (JSON) next to it: measurements, links, BOQ import, schedules, conflicts, AI notes, and which drawings belong to the project. On save, measurements are also written into the DWG as polylines and points on layers named `QIYAS-*` with xdata, so they survive a trip through AutoCAD. Opening a drawing with those layers rebuilds the measurements if the project file is missing.

**Take-off model.**

```
Measurement { id, kind: length | area | count | volume | item, geometry: [handles or own polyline],
              value, unit, layer, sheet, boq_item?, source: user | ai, note, created }
Schedule    { name, sheet, columns, rows, source table handle }
BoqItem     { code, description, unit, quantity, rate?, section }
Conflict    { kind, boq_item?, measurements, schedule?, expected, found, reason, status }
```

Volume is an area times a height the user or AI supplies (slab thickness, wall height) and remembers per layer.

**AI.** `qiyas-ai` exposes about twenty tools, each a thin wrapper over commands: `open_drawing`, `list_sheets`, `list_layers`, `show_layers`, `find_entities` (by layer, type, block name, text), `get_entity`, `measure_length`, `measure_area`, `count_blocks`, `read_text`, `read_tables`, `read_dimensions`, `add_measurement`, `set_height`, `import_boq`, `link_boq`, `compare`, `list_conflicts`, `add_markup`, `zoom_to`, `view_image` (a rendered picture of the current view, so the model can look). Two front doors:

- **Built-in assistant panel.** Chat in English or Arabic. Rust calls the Claude Messages API over HTTPS (`reqwest`, streaming, a manual tool loop; Rust has no official SDK). Model `claude-opus-5-5`, adaptive thinking, server-side fallbacks on. The user's API key goes in the OS keychain. Every tool call shows in the panel and highlights its geometry. The assistant's system prompt carries the project's units, markets and the QS Mind take-off rules.
- **MCP server.** The same tools over stdio and a local port, so a QS who already has Claude Desktop or Claude Code can say "open the villa project in Qiyas and measure the slabs" with no key in the app.

**Languages.** English and Arabic, right-to-left layout, Thmanyah Sans for Arabic (swash headings), Instrument Sans for English. Command names stay English in both.

## Features by phase

Each phase ends with a check the founder can run on a real drawing.

**Phase 1, open and measure.** Open DWG and DXF with xrefs (1, 14), file tabs and recents (15), PDF import and underlay, image underlay (6, 7, 8). Layers panel with isolate, freeze, match (47, 48). Zoom and pan, named views, layouts tabs (56, 58, 61). Object snaps, tracking, grid, units (72 to 74). Properties palette, command line (75, 77). Measure: distance, area, geometry, divide, ID, count, data extraction (31 to 37). Take-off panel with measurements as objects, totals, Excel export. Save DWG and DXF with the QIYAS layers (3). Ribbon, status bar, docking, dark and light themes (78). English and Arabic (87).
Check: open a real villa set, measure slab areas and wall lengths, count doors, export the take-off to Excel, reopen the project.

**Phase 2, AI and BOQ.** Command bus exposed as tools, MCP server, assistant panel (81, 82). BOQ import from Excel, links, schedules read from tables (38, 43). Comparison engine and conflicts panel with Excel report.
Check: import the tender BOQ, ask the assistant to measure the concrete and compare, get a conflict list with at least one real finding the founder confirms.

**Phase 3, edit and annotate.** Drawing tools (16 to 22). Modify tools (23 to 30). Text, all dimension types, dimension edit, leaders, tables, fields, tolerance, annotation scale (39 to 46). Blocks: insert, create, edit in place, attributes, palette, design centre, tool palettes (51 to 55). Layer states, translator, viewport overrides (49, 50). Groups, clipboard, match properties (76). Backups and recovery (4, 5). Multiple viewports and tiling, UCS, sheet sets (60, 62, 63).
Check: redraw a missing wall, add a dimension and a revision cloud, save, open the file in AutoCAD and see the same thing.

**Phase 4, output and the rest.** Plot to PDF, print, plot styles (13). Command scripts, headless conversion, plugins, audit, purge, options, customise the interface (79, 80, 83, 84). Parametric constraints (69 to 71). Update check, single instance, crash recovery (88).

## Brand

Qiyas joins `brand/` as `brand/qiyas/`: mark, small mark, lockups in English ("Qiyas") and Arabic (قياس in Thmanyah Sans Bold with swash), favicon and app icon. Face colour anodised titanium, #C9A6E0 to #6E4A9E, added to the colour table in `brand/README.md` and `brand/guide.html`. Window title "Qiyas". English text says "Qiyas", Arabic text says "قياس". The lockup line in the About box: "Qiyas, a QS Mind app". No tagline; only QS Mind has one.

## Repo and workflow

- New private repo `kareem-sf/qiyas`, cloned to `qiyas/` in this workspace. Added to `repos.txt` and the apps table in the workspace `CLAUDE.md`. Its own `CLAUDE.md` carries these rules.
- No CI. `scripts/check.sh` runs everything locally before any push. No push, no release, without the founder's local check and yes.
- Tests: Rust unit tests per command and per comparison rule, on synthetic drawings written with `opencadcodec` (the way Quantix's `qx-dwg sample` does). Golden numbers for a set of synthetic take-offs. Vitest on pure TypeScript (viewport maths, panel state). Real customer drawings stay outside the repo in a local folder for manual checks.
- Licences shipped: opencadcodec and opencadkernel (MPL-2.0, notices kept), pdfium (Apache-2.0), fonts (Instrument Sans OFL; Thmanyah never in the repo, loaded from the user's machine or bundled only in private builds under the licence).

## Risks and how we handle them

- **Viewport speed on big drawings.** Buffers built in Rust, redraw only dirty handles, spatial index for picking. Measure early with a 50 MB drawing in Phase 1.
- **Arabic text in drawings.** AutoCAD stores Arabic in several ways (reversed, shaped, SHX fonts). Shape with `rustybuzz`, test on real Egyptian and Gulf drawings in Phase 1.
- **PDF vectors.** Architects' PDFs vary wildly. `pdfium-render` for pages and paths; import by layer when the PDF has them, by colour and line weight when it does not.
- **AI trust.** The assistant never produces a number without geometry behind it. Every measurement it adds is highlighted and undoable, and the conflicts panel shows expected vs found. It also looks at the view as an image when text and geometry are not enough.
- **Scope.** Groups 2, 3 and 5 are most of the work. They are Phase 3 so the product (Phases 1 and 2) exists first.

## Out of scope

3D modelling (group 9), the web app (86), point clouds (9), OBJ, STL, STEP (10), LandXML (11). Orbit and 3D cameras (57) and visual styles (59) mean nothing in a 2D app, so they are out even though group 8 was picked. Plain CSV export (12) is covered by Excel export. Moving code from Quantix, Tawreed or OpenConKit into Qiyas needs its own decision in `docs/`.

# Qiyas

Qiyas (قياس) is a desktop measurement CAD for quantity surveyors, part of QS Mind. It opens DWG and DXF drawings, lets the QS measure and edit, and uses AI to build and check BOQs against the drawing. Private repo (`kareem-sf/qiyas`). Stack: Tauri 2, React, TypeScript, Vite, Rust.

## Layout

```
qiyas/
  crates/
    qiyas-core      the document: opencadcodec CadDocument in memory, layers, blocks,
                    layouts, xrefs, selection, snaps, units, undo and redo, command bus, draws
    qiyas-geom      2D geometry on top of opencadkernel: offset, trim, fillet, area,
                    length, closed regions, intersections
    qiyas-render    turns the document into GPU buffers: lines, fills, text glyphs,
                    hatches, images; spatial index; dirty tracking per handle
    qiyas-takeoff   the comparison engine: take-off and drawings against the BOQ
                    (conflicts); the BOQ, schedule and conflict types live in qiyas-core
    qiyas-boq       Excel in and out (calamine to read, rust_xlsxwriter to write)
    qiyas-pdf       PDF import as vectors and text, PDF underlay pages, PDF plot (later phase)
    qiyas-ai        the 32 AI tools over the command bus, the agent loop, the Claude and
                    OpenAI-compatible clients, the MCP server
    qiyas-mcp       the stdio bridge Claude Desktop and Claude Code start (bundled beside the app)
    qiyas-desktop   the Tauri app: IPC, file dialogs, settings, keychain, assistant, MCP switch
  src/              React + TypeScript + Vite: viewport, ribbon, command line, panels
  scripts/check.sh  cargo fmt, clippy, test, a release speed test, tsc, vitest, in one run
```

All the crates above exist; `qiyas-pdf` reads PDFs (import, underlays), PDF plot comes later.

Pinned libraries (git revs in the workspace `Cargo.toml`): opencadcodec `a35f43e` (DWG/DXF) and
opencadkernel `99b6b48` (`geom2d` for snaps and edits, `offset` for polyline offsets: it brings
cavalier_contours, MIT OR Apache-2.0). Both MPL-2.0. Licence texts of code we ship
live in `LICENSES/` (opencadcodec, opencadkernel, pdfium); add one there for every new non-MIT
library. Font licences sit beside the fonts in `crates/qiyas-render/fonts/`.

Samples: `samples/test-plan.dwg` (written by `cargo run -p qiyas-core --example write_test_plan`) and `samples/test-boq.xlsx` (written by `cargo run -p qiyas-boq --example write_test_boq`); the exact quantities and the two conflicts the BOQ must give are in `samples/test-plan.md`. `cargo run -p qiyas-ai --example phase2_walkthrough` measures, imports, links and compares them through the AI tools, no model needed.

## Global constraints

- Desktop only. No web build, no CI workflows. Local check is `scripts/check.sh`.
- Window title and product name: `Qiyas`. Arabic name: `قياس`. Bundle identifier: `com.qsmind.qiyas`.
- This repo is its own git repo. Commit inside it. Never push or release without the founder's yes.
- Only MPL-2.0, Apache-2.0, MIT, BSD, ISC or OFL dependencies (ISC: `ring`, `rustls-webpki`, `untrusted`). No code copied from Open CAD Studio (GPL).
- Thmanyah font files are never committed. `scripts/fonts.sh` copies them from `~/Downloads/Thmanyah-Font-Family/thmanyah typeface` into the git-ignored `src/fonts/thmanyah/`. Instrument Sans (OFL) is committed. The viewport draws drawing text with Noto Sans and Noto Sans Arabic (OFL, unhinted Regular, with their licences in `crates/qiyas-render/fonts/`), behind the drawing's own TrueType font when the computer has it.
- English text says "Qiyas" and "QS Mind"; Arabic text says "قياس" and "كيو إس مايند". Arabic uses Thmanyah Sans; headings use `font-feature-settings: "swsh"`.
- Measured values are stored in drawing units and displayed in metres, square metres and pieces. Unit factor comes from the drawing header (`insertion_units`): 1 = inches, 2 = feet, 4 = mm, 5 = cm, 6 = m, 0 or unknown = treat as mm and show a warning in the status bar.
- Every mutating command is undoable. Undo and redo stacks live in `qiyas-core`.
- Brand colours: Ink `#0E0F12`, Stone `#F2EEE7`, Paper `#F9F8F4`, Slate `#6E6A64`, Qiyas face anodised titanium `#C9A6E0` to `#6E4A9E`.

## Working here

- Use pnpm, not npm.
- Run `scripts/check.sh` before every commit.
- `qiyas-render` builds with one codegen unit (`Cargo.toml`): split, the pick index builds about
  4 times slower. `cargo test -p qiyas-render --release pick_index_one_million` guards it.
- Bound-xref layer names (`XREF$0$A-WALL`) are shown as `A-WALL` (`short_layer_name`); the
  full name stays in `layer`.
- Never push or release without the founder's yes.
- Run the app: `pnpm tauri dev --config crates/qiyas-desktop/tauri.conf.json`.
- The `.qiyas` project file is saved next to the drawing after every command.
- Save keeps a `.bak` of the original drawing (e.g. `villa.dwg.bak`) before writing over it.

## Undo

- `qiyas-core/src/undo.rs`: one step per command, at most 200, each with its `Source`
  (`undo_if` lets the AI undo only its own). Take-off, BOQ and conflict steps keep that part whole.
- A drawing command (`Part::Document` in `Command::changes`) runs inside
  `Session::record_document_change`: opencadcodec's entity change recorder keeps the before image
  of each entity it adds, removes or edits (`DocChange`), never a copy of the whole drawing.
  Commands that change layers, views, block records or underlay and image definitions also keep a
  `TablesSnapshot`; divide and measure along keep the take-off too. A failed command is put back.
- Undo removes added entities, restores removed ones under their own handles (a removal leaves
  the handle in its block's list, so `restore_entity_arc` puts it back in place) and swaps edited
  ones back. Every drawing step, undo and redo moves `document_revision` on (batch version, dirty
  flag, tabs).
- `Command::DeleteEntities { targets }`: each `Target { handle, chain }` as picked (`Picked`). Only
  the drawing's own entities at the top of the sheet on screen (empty chain, owned by the sheet);
  a block's inside or an xref's (non-empty chain) or another sheet's are refused, and then nothing
  is deleted.

## Drawing on screen

- Picking reaches inside blocks: the first click picks the line inside the block, a slow second
  click the whole block, a double click or Enter measures it. Measurements keep the block chain.
- Drawing text is drawn as real glyphs (TEXT, MTEXT, attributes) with Noto Sans and Noto Sans
  Arabic, or the drawing's own TrueType font when the computer has it; SHX falls back to Noto.
  Arabic is shaped and joined. View tab, "Arabic text is reversed" fixes drawings whose Arabic
  was stored backwards (saved in the project file).
- Snaps (end, mid, centre, node, intersection, nearest, perpendicular), ortho, polar and the dot
  grid: status bar toggles, F3 / F8 / F10 / F7, settings under the gear. Saved per drawing in the
  project file.
- Xrefs are read-only: loaded from the stored path, beside the drawing, `xrefs/` or `x-refs/`
  (depth 4); View tab, External references to reload or locate one (kept in the project file).
- Command line (Ctrl+9 shows or hides it): AutoCAD-style words and aliases (DI, AA, COUNT, DIV,
  ME, ID, Z E/W/IN/OUT, LAYOFF, U…), typed points `x,y`, `@dx,dy`, `@d<angle` in drawing units,
  Arabic digits accepted. Extra aliases: `aliases` in `settings.json` (no screen for them yet).
- Properties palette under Layers: what the Select tool picked (colour as drawn, linetype,
  weight, attributes, values in metres).
- The batch also carries fill triangles (section 12, `u32` colour each, bit 24 =
  `FILL_BACKGROUND`) and their ranges (13), only when there are fills; GL draws fills under the
  lines, then wipeouts in the background colour over all lines (they hide line work under them,
  even lines drawn later: no draw order yet), then the wipeout frames. Solid hatches, 2D solids
  and traces fill; gradient hatches fill with their first colour; pattern hatches draw pattern
  lines clipped to the boundary (range flag `PATTERN`, never part of a hatch boundary, left out
  when denser than 20,000 lines); rays and xlines run across the sheet without growing its
  bounds; multilines draw their element lines but measure along their centre line (the drawn
  points); points draw as a cross (PDSIZE, or 1% of the sheet). Splines flatten from control
  points and knots (the curve through the fit points when there are none). Measuring sees
  outlines only (`FlattenOutcome::into_paths`).

## Drawing

- `Command::Draw(DrawSpec)` (`qiyas-core/src/draw.rs`): line, polyline (bulges), rectangle,
  polygon, circle, arc (centre or three points), ellipse, spline (fit points plus the cubic's
  control points), point, donut, ray, xline, revcloud, wipeout, hatch (closed entities or a
  ring; solid, ANSI31, ANSI37 or a built-in pattern: LINE, NET, DASH, BRICK), 2D solid, trace,
  multiline, sketch. One undo step on the sheet on screen; `Outcome::Added` lists the handles.
  Wire form `{"type":"draw","kind":"line",...}`, angles in degrees. Not an AI tool yet.
- `Session::current` (layer, colour, linetype, lineweight; layer "0" and ByLayer by default)
  styles new entities. `Command::SetCurrent` is not an undo step; saved in the project file;
  layer and linetype as the drawing spells them, lineweights from AutoCAD's standard set; a
  layer gone from the drawing falls back to "0". Layers panel: the pen button makes a layer
  current (not an xref's).
- Home tab, Draw: Line (C close, U undo the last line), Polyline (A arc, L line, C close, U
  undo; Esc keeps what is drawn), Rectangle, Circle (2P, 3P, typed radius), Arc (three points,
  or C centre first), Ellipse, Spline, Point, Hatch, Revision cloud, Wipeout.
- Hatch: click inside. `Session::boundary_at` runs `arrangement::bounded_faces` over the drawn
  segments in a box about 50 screen pixels across, growing it 4 times until the region found
  fits inside the box; regions inside it become islands. More than 50,000 segments in the box
  is `TooDense` (the prompt says zoom in), never a ring from part of them. Type a pattern
  (SOLID, ANSI31, ANSI37, LINE, NET, DASH, BRICK), S scale, A angle; the default scale is
  0.025 / metres per unit (25 in millimetres).
- Command line: L, PL, REC, C, A, EL, SPL, PO, H, REVCLOUD, WIPEOUT; typed whole: POLYGON (POL)
  sides x,y radius [I|C], DONUT (DO) inner outer x,y, RAY / XLINE (XL) x,y x,y, SOLID (SO) 3 or
  4 points, TRACE width points, MLINE (ML) points, SKETCH points.

## Modify

- `Command::Modify(ModifySpec)` (`qiyas-core/src/modify.rs`): move, copy (A: several in a row),
  rotate and scale (C: a copy), mirror (keep; texts and attributes still read forwards, as
  MIRRTEXT 0), stretch (line and polyline vertices in the window move; other entities move whole
  when wholly inside or when their centre or insertion point is), align (one or two pairs, no
  scaling), rectangular and polar arrays (polar `angle` is between items), explode (block
  references one level, as drawn: base point and turned array cells; layer 0 and ByBlock take
  the reference's; visible attributes become texts holding their values; polylines and
  multilines; anything else is left as it is, `Outcome::Exploded { left_alone }` says how many).
  One undo step. Wire form `{"type":"modify","kind":"move","targets":[...],...}`, angles in
  degrees. Erase is `DeleteEntities`. Not an AI tool yet.
- Targets as for delete: the drawing's own entities at the top of the sheet, all or nothing.
  Changes in place keep handles; copies get new ones (attributes too; vertex, face and sequence
  end handles are nulled for the writers to give) and drop reactors, extension dictionaries and
  hatch associativity. Dimensions and tables only move and copy: a dimension's `*D` block is in
  world coordinates and moves with it (its own insertion point stays); a table refers to its
  `*T` block, which stays; a copy gets its own block under a new anonymous name. The other tools
  refuse them; the window leaves them out of a selection for those tools and says how many.
  Images and PDF underlays move but are not copied.
- The take-off follows edits in the same undo step (`Session::follow_takeoff`): rows measuring
  a moved, turned, scaled, mirrored, stretched or aligned entity are measured again; erase and
  explode keep the row's value but drop its link. Erasing an image or underlay cleans its
  definition's link.
- `Session::pick_window(bounds, crossing)` (IPC `pick_window`): a window takes entities drawn
  wholly inside, a crossing those with a segment touching it (clipped exactly); always the entity
  at the top of the sheet (a block or xref reference whole), with its kind. The select tool
  keeps a selection set: a click replaces it (PICKADD), Shift+click adds, a left drag draws a box
  (left to right a window, solid; right to left a crossing, dashed), Shift+drag adds, Esc clears;
  after an undo or redo it keeps only what is still drawn. A modify tool starts on it (or on the select
  tool's pick, whole), else asks for one (clicks or a box, then Enter); stretch always asks for
  its crossing window. Home tab, Modify group; command line M, CO/CP, RO, SC, MI, S, AL, AR (R or
  P), X, E. Grips come later.
- `Command::Edit(EditSpec)` (`qiyas-core/src/modify2.rs`, wire `{"type":"edit","kind":"trim",...}`):
  trim, extend, break, join, lengthen, reverse, offset, fillet, chamfer, blend, pedit, overkill,
  flatten; same targets, one undo step. The take-off follows (`follow_edit`): rows on a cut
  entity measure every piece, rows on joined ones the joined polyline (the outcome says how many
  rows measure it), and a row that can't be measured any more is unlinked and counted (`unlinked`),
  keeping its value. Lines, arcs, circles and flat
  polylines are reshaped; anything drawn can cut or bound. Trim and break keep the handle on the
  longest piece; join keeps the first's. Offset keeps exact arcs (kernel `offset_polyline`, sharp
  straight corners). Fillet and chamfer: two lines or two straight sides of one polyline.
  `Outcome::Edited { added, removed, left_alone }`. Modify's second row; command line TR, EX
  (Shift+click swaps), BR, J, LEN, REVERSE, O, F, CHA, BLEND, PE, OVERKILL, FLATTEN.

## PDF

- PDFs are read through pdfium (BSD-3), a shared library loaded at run time through
  `pdfium-render`. `scripts/pdfium.sh` downloads the build for this computer from
  bblanchon/pdfium-binaries into `crates/qiyas-desktop/resources/pdfium/` (git-ignored) and its
  licence into `LICENSES/pdfium-BSD-3.txt`. It installs the tested release only (`TESTED_TAG`,
  chromium/8086) and checks the download against `scripts/pdfium.sha256`; a mismatch installs
  nothing. Run it once per machine. Without it the PDF commands
  answer "PDF support is not installed: run scripts/pdfium.sh" and the PDF tests are ignored, so
  `scripts/check.sh` runs them only where the script has been run (no CI checks them).
- `QIYAS_PDFIUM` (the library or its folder) overrides where it is looked for.
- Release: run `scripts/pdfium.sh` before `pnpm bundle`; `tauri.bundle.conf.json` ships the
  folder as the resource `resources/pdfium/`.
- pdfium is not thread-safe: every call runs on one worker thread (`qiyas_pdf`).
- View tab: "Attach PDF…" (underlay or import, scale, page, insert point) and "Calibrate".
  Underlays are kept in the DWG/DXF as AutoCAD PDF underlays on layer `PDF-UNDERLAY`; imports
  go on `PDF-RRGGBB`, `PDF-FILL-RRGGBB` and `PDF-TEXT`. Save As rewrites underlay and image
  links for the new folder. Opening another drawing or closing the window with unsaved drawing
  changes asks "Save changes to the drawing?".

## Images

- View tab: "Attach image…" (png, jpg, tif, bmp, webp; width in drawing units, default a tenth of
  the sheet; insert point typed or clicked), then "Calibrate" as for PDFs. Images are kept as
  AutoCAD images (`RasterImage` + `ImageDefinition` in `ACAD_IMAGE_DICT` + reactor) on layer
  `IMAGE-UNDERLAY`, linked relative to the drawing when inside its folder.
- Images already in a DWG/DXF are drawn when their file is found (as stored, beside the drawing,
  in its `images/` folder); otherwise the problems list says "Image <file> not found".
  Rectangular clips crop; polygonal and inverted clips draw the whole image for now; images
  inside blocks are not drawn.
- Images over 20,000 px a side or 512 MB decoded are refused ("This image is too large").
- Files are decoded once (cut to 4096 px, EXIF orientation applied) with the `image` crate; the
  window gets a picture per zoom (64 to 2048 px) through the same `underlay_image` IPC as PDFs.
- Picking an image (Select tool, click its frame) shows brightness, contrast and fade sliders
  (`set_image_display`, one undo step per release).

## Inquiry

- Measure tab: Geometry (click an entity: radius, diameter, centre, angles, lengths, block and text
  facts in an Info card at the top of the right panel), Divide (click, then the number of parts),
  Measure along (click, then the step in metres), ID point (snapped coordinates in metres), and
  Extract data (filter by layers, type, block, text; pick fields and attribute tags; copy as TSV
  or export one "Data" sheet to Excel).
- Divide and measure along place point entities on layer `QIYAS-POINT` by true arc length and
  count them in the take-off; points and count are one undo step (a drawing change: Save asks).
- The same five are AI tools: `measure_geometry`, `divide_along`, `measure_along` (step in
  metres), `id_point`, `extract_data` (paged under 40 KB).

## Drawings, tabs, recents and views

- Several drawings open at once, one file tab each above the drawing (title, "•" while the drawing
  has unsaved changes, × or a middle click closes; Ctrl+Tab / Ctrl+Shift+Tab / Ctrl+W). The tab on
  screen is `AppState::session`, so every IPC command, the assistant and MCP act on it; the others
  are parked (`AppState::parked`, without their batch and pick index) and swapped in by
  `activate_tab`. Lock order: `tabs`, `session`,
  `parked`. Switching or closing the tab on screen starts a new assistant conversation. Opening a
  file that is open already shows its tab. Closing (or quitting) with unsaved drawing changes asks.
- Recent drawings: `recents` in `settings.json` (10 at most, newest first, kept by the app on every
  open and Save As; the Settings dialog never changes them); the arrow beside Open lists them,
  greyed "(missing)" when the file is gone, then Clear recents.
- Named views: the drawing's `VIEW` table (model space only). View tab, Views: choose one to zoom
  to it, Save current view… (Model sheet only) or × to delete; both are undoable drawing changes.
- A drawing that opens with nothing in model space says "This drawing has no geometry in Model"
  in the status bar.

## AI assistant and settings

- Settings live in `settings.json` in the app's config folder (Linux:
  `~/.config/com.qsmind.qiyas/settings.json`, mode 0600): provider (`anthropic` or
  `openai_compatible`), model (default `claude-opus-5-5`), base URL, effort, MCP on/off, port and
  token, language, theme.
- The API key is never in a file: it lives in the OS keychain (service `qiyas`, account = the
  provider name), is set only in the Settings dialog, and is never sent back to the window.
  Agents never read or type a real key; the founder pastes it himself.
- `QIYAS_AI_MOCK=1` with a debug build (`pnpm tauri dev`) uses a canned provider: it says a
  sentence, measures the first polyline on layer A-WALL and answers. No key, no network. Release
  builds leave it out.
- AI measurements are marked `ai`; the card's Undo only undoes when the last step is the AI's.
- The in-app agent is called Mizan (ميزان): the side tab, the panel and the system prompt use the
  name; code keeps `assistant` / `useAssistant`.
- Agent log: `logs/agent.log` beside `settings.json` (mode 0600, moved to `agent.log.1` past
  5 MB). Each conversation's provider, model and address, then every user message, request,
  answer (text, calls, tokens, time), tool run (input, result) and error with the service's
  whole answer; texts clipped at 4,000 characters, never the key. The panel's Log button shows
  its newest 256 KB (`ai_log`); an error offers "Show log".
- OpenAI-compatible streams: Gemini (also behind Runware) sends parallel calls whole, all at
  index 0 or with none, so a new id, or a second name, starts a new call (`openai.rs`, `slot`).
  Input that was not valid JSON goes back as `{}`, as for Claude, so one bad call can't make
  every later request fail.

## MCP

Outside AI apps can drive Qiyas through its MCP server (the same tools as the in-app assistant;
their measurements are marked `mcp`).

- Turn it on in Settings, MCP server. It listens on `127.0.0.1:48123/mcp` (port in Settings) and
  needs `Authorization: Bearer <token>`; a request with an `Origin` other than the server's own
  gets 403, so web pages can't use it; the token is in Settings and in `settings.json` (mode
  0600). The dialog shows whether it is listening.
- Settings gives both setups, with the bridge's path filled in:
  - Claude Code: `claude mcp add qiyas -e QIYAS_MCP_TOKEN=<token> -- <app folder>/qiyas-mcp`
  - Claude Desktop (`claude_desktop_config.json`):
    `{"mcpServers": {"qiyas": {"command": "<app folder>/qiyas-mcp", "env": {"QIYAS_MCP_TOKEN": "<token>"}}}}`
- `qiyas-mcp` speaks MCP on stdio and forwards to the app over HTTP. `QIYAS_MCP_URL` overrides
  the address (default `http://127.0.0.1:48123/mcp`). When the app is not running it answers
  "Qiyas is not running or MCP is disabled in its settings".
- Dev: `cargo build -p qiyas-mcp` puts the bridge in `target/debug`, the folder Settings shows
  under `pnpm tauri dev`.
- Release builds: always `pnpm bundle`, never plain `pnpm tauri build`, so the bridge ships.
  `pnpm bundle` runs both steps below.
- Bundling: `scripts/build-sidecar.sh` builds it in release into
  `crates/qiyas-desktop/binaries/qiyas-mcp-<target triple>` (git-ignored), then
  `pnpm tauri build --config crates/qiyas-desktop/tauri.bundle.conf.json` ships it beside the
  app. The external binary lives in that extra config, not `tauri.conf.json`, so plain builds
  and tests don't need the sidecar built first.

## Where the plans live

In this repo, `docs/` (write new specs, plans, checks and reports here, never in the workspace):

- Spec: `docs/superpowers/specs/2026-10-08-qiyas-design.md`
- Plan (Phase 1a): `docs/superpowers/plans/2026-10-08-qiyas-foundation.md`
- Plan (Phase 2, AI and BOQ): `docs/superpowers/plans/2026-10-08-qiyas-phase2-ai-boq.md`
- Plan (Phase 1b, drawing I/O): `docs/superpowers/plans/2026-10-08-qiyas-phase1b-drawing-io.md`
- Checks: `docs/2026-10-08-qiyas-foundation-check.md`, `docs/2026-10-09-qiyas-phase2-check.md`,
  `docs/2026-10-09-qiyas-phase1b-check.md`

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
  `arboard` (the system clipboard, MIT OR Apache-2.0) is allowed; it is built without its Wayland
  feature (that brings a Zlib crate), and on Windows it uses `clipboard-win` and `error-code`
  (BSL-1.0, permissive: founder's check).
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
- Autosave writes `<name>.sv$` and `<name>.sv$.qiyas` beside a drawing with unsaved changes (see
  Clipboard, viewports, UCS, sheet sets and recovery); a save deletes them.

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
  weight, attributes, values in metres); a drawing's own entity is edited there (see Layers,
  groups and properties).
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

## Text

- `qiyas-core/src/text.rs`: `Command::Text` (one line; `rotation` in degrees) and `MText` (wire
  `mtext`; plain text, `\n` becomes `\P`, `\`, `{`, `}` escaped; box top left and width) go on the
  current layer and state with a text style (`None`: the drawing's TEXTSTYLE, else "Standard");
  `EditText { target, value, tag }` changes a text, an MText (as plain text: its formatting goes)
  or one attribute (`tag`, may be cleared) of a block reference; chains refused. One undo step
  each.
- New and edited text is stored in reading order and marked with xdata app `QIYAS_TEXT`
  (`qiyas_geom::LOGICAL_TEXT_APP`, never `QIYAS`: `embed_measurements` deletes those);
  `FlatText::logical` makes the renderer ignore "Arabic text is reversed" for it.
- `FindReplace { find, replace, scope: sheet | drawing, match_case, whole_word }`: texts, MText
  plain runs (codes such as `\C1;` and braces are never matched), attribute values (a multiline
  one's embedded MText, every line) and dimension overrides (`<>` kept) on the sheet or every
  sheet, not inside block definitions; one undo step, `Outcome::Replaced { count, handles,
  skipped }` (none when nothing changes). A TEXT the result would leave blank or break into two
  lines, or a single-line attribute it would break, is left as it is and counted in `skipped`. `FindText` lists the
  matches (`Outcome::Found`), no step. `DrawingInfo` has `text_styles` and `text_style`;
  `EntityInfo::plain_text` is an MText's text without codes.
- Home tab, Text group; command line TEXT (DT): click, type in place, Enter (a click elsewhere
  places it too; Esc drops it), H height, R rotation; MTEXT (MT, T): two corners, then the MText
  dialog; TEXTEDIT (ED, DDEDIT) or a double click on a text with Select: the edit dialog (an
  unchanged value sends nothing); FIND: the find and replace dialog (Find next zooms to each
  entity, switching sheet: the sheet's first fit goes to the match, `fitMatch`; Replace all
  highlights what changed and says how many were skipped). New text height starts at
  2.5 mm in drawing units, then the last one used.

## Dimensions

- Drawn as themselves: a Dimension draws its `*D` block (or, without one, the geometry
  `qiyas_geom::dimension_entities` generates from its points and style); leaders draw their line
  and arrowhead, tolerances their frame (flatten's `owner`: every path, even an arrowhead block
  inside, is the entity, chain as for any entity). Picking, windows, `entity_info` (its text) and
  `MeasureEntity` (its measured distance, a length row; angles have none) see the dimension.
- `Command::Dimension(DimSpec)` (`qiyas-core/src/dimension.rs`, wire `{"type":"dimension",
  "kind":"linear",...}`): linear (no rotation: horizontal or vertical by where the line point is),
  aligned, angular (3 points + arc point), radius, diameter, arc length, ordinate, baseline,
  continue (on the base's own line; baseline a DIMDLI step further out each), quick (lines, arcs,
  circles); an ordinate may read 0. Each is a real Dimension on the current layer in the
  current style (DIMSTYLE) with a `*Dn` block: Lines, Solid arrowheads, one Text (marked
  `QIYAS_TEXT`), sized DIMASZ/DIMTXT/DIMEXO/DIMEXE/DIMGAP x DIMSCALE, numbers by DIMDEC, DIMZIN,
  DIMLFAC, DIMRND, DIMDSEP, DIMPOST (decimal only).
- `DimEdit` (text override with `<>`, flip, move text) and find and replace rebuild the block in
  the same step (`rebuild_dimension`). `DimBreak` and `DimJogLine` change the block's lines only:
  a later rebuild drops them. `DimSpace` moves parallel linear dimensions to equal spacing.
  Dimensions are not re-measured by `follow_edit`; a moved dimension's take-off row follows.
- `Leader`/`MLeader` (a Leader + MText; Qiyas writes no MULTILEADER), `Tolerance` (frame in
  AutoCAD codes). `SetDimStyle` (create from current, common fields, make current; header DIM vars
  follow; an edit rebuilds, in the same step, the sheets' dimensions of that style whose block
  Qiyas generated: every part of such a block carries `QIYAS_TEXT`; AutoCAD's blocks stay as they
  are). `AnnotationScale` sets CANNOSCALE ("1:50", header and the variable dictionary); new
  dimensions, leaders and tolerances keep the scale's factor as their own DIMSCALE (`DSTYLE`
  override; it replaces the style's, which applies at 1:1), new text 2.5 mm x scale. No
  annotative scale lists. All undoable (header bits are in `TablesSnapshot`). DIMTAD 4 puts the
  text below the line; `entity_info` shows `%%c`/`%%d` as their signs.
- Annotate tab (Dimensions, Leaders, Dimension style…); command line DLI, DAL, DAN, DRA, DDI, DAR,
  DOR, DBA, DCO, QDIM, DED (T text, F flip, M move), DIMBREAK, DIMSPACE, DIMJOGLINE, LE, MLD, TOL,
  D (DIMSTYLE). Not AI tools yet.

## Tables and fields

- Tables are drawn as themselves (`qiyas_geom::table_entities`, flatten's Table arm): the `*T`
  block when it has something in it (in table coordinates: insertion point, horizontal direction,
  rows going down), else grid lines (none inside a merged cell) and each cell's text as an MText
  generated from the rows, columns and cells. Picking and windows give the table; `entity_info`
  has `table` (cells as plain text, widths, heights, merges, field cells) and no length or area;
  `table_cell_at` (IPC too) is the cell under a point.
- `qiyas-core/src/table.rs`: `Command::Table { at, rows, cols, col_width, row_height, data }` (current
  layer, current text style, text half the row height, middle centre), `EditCell` (plain text,
  stored escaped; a merged cell's first), `InsertRow/InsertCol { at }`, `DeleteRow/DeleteCol`
  (one kept), `MergeCells { first, last }` (one cell unmerges). One undo step each; each draws the
  `*T` block again (`rebuild_table`; a block another table shares gets a fresh `*Tn` instead).
  A table's block is found by name, else by `block_record_handle` (the DWG reader sets only the
  handle; `qiyas_geom::table_block`), also when copying. Find and replace reaches cells. Qiyas's
  tables are marked `QIYAS_TEXT` (reading order).
- `qiyas-core/src/field.rs`: fields are AutoCAD field codes in the text (`%<\AcVar Date ...>%`,
  `%<\AcVar Filename>%`, `%<\AcObjProp Object(%<\_ObjId HEX>%).Area|Length ...>%`, Qiyas's own
  `%<\AcVar QiyasCount "BLOCK">%`). The visible text is the value; the code (the whole text as
  typed) is kept in `QIYAS_FIELD` xdata (on a table: row, column, code per cell). `Command::Field
  { at, height, field }` adds a one-line text; codes typed into Text, MText or a cell count too.
  `UpdateFields` works them all out again; area and length fields follow every modify, edit and
  erase in the same step (`follow_fields`). Metres and square metres (`in_metres`, as the
  take-off), 3 decimals; UTC date; `####` when the entity is gone. Not worked out on open. Find
  and replace leaves field texts and field cells as they are and counts them in `skipped`.
- Annotate tab, Tables group: Table (click, then rows, columns and sizes), Edit table (or a double
  click on a table: `TableEditor`, a grid in the drawing's column order, Shift+click to merge),
  Field (click, then `FieldDialog`; area and length pick their entity; also from the table
  editor into a cell), Update fields. Command line TABLE (TB), TABLEDIT, FIELD, UPDATEFIELD.

## Blocks

- `qiyas-core/src/blocks.rs`: `CreateBlock { name, base, targets, convert_to_insert }` (sheet
  entities leave for a new block, relative to `base`; names AutoCAD refuses, or the drawing's own,
  are refused; with convert a reference takes their place; rows on them are unlinked and counted,
  `Outcome::Edited`), `InsertBlock { name, at, scale, rotation, attribute_values, from_path }`
  (current layer; attributes from the block's ATTDEFs, constants excepted; `from_path` copies a
  missing block with its layers, linetypes, styles and nested blocks from another drawing, read
  only), `WriteBlock { name, path }` (a new .dwg in the drawing's folder, never the drawing
  itself; the Save dialog's path goes through IPC `write_block`), `AttDef` (current layer; drawn on a sheet as its tag; inside a block
  only constants draw), `EditAttributes`, `EditAttDef`, `SyncAttributes` (ATTSYNC, values kept),
  `RedefineBlock { name, from_path }`. One undo step each; WBLOCK is none. Constant ATTDEFs are
  never prompted (`AttDefInfo::constant`).
- Edit in place (`EditBlockInPlace { target }`, a sheet reference, not arrayed, even scale, no
  dimensions or tables inside): the block's entities are copied onto the sheet through the
  reference (no step), the reference hidden, the rest faded (`blocks::fade`), locked
  (`sheet_targets`) and left out of `pick` and `pick_window`; new entities on that sheet join the
  block. Commands are steps as usual, but undo stops at the edit's start (`History` floor, which
  follows the 200-step cap); only CreateBlock, WriteBlock, another edit in place, redefining the
  block (or one holding it) and inserting it into itself wait; Save waits too. Closing is by snapshot, never by undo:
  `SaveBlockEdits` makes the sheet's new entities the block's contents (a changed copy replaces
  its entity under the same handle, an unchanged one leaves it, gone ones go, a dimension's `*D`
  block is taken back too), `DiscardBlockEdits` drops them; everything else done meanwhile stays.
  Neither is an undo step, and nothing before it can be undone after it: Save clears the whole
  history, Discard too unless every step made while editing was undone (`History::close_edit`); a
  failed Save puts back what it wrote and leaves the edit open. `Outcome::BlockEditClosed` says how
  many steps went, and how many take-off rows lost their link. `DrawingInfo::block_edit` names the block.
- Insert tab, Block group: Create block (B: pick, base point, name dialog), Insert… and Write
  block… (`BlockPalette`: thumbnails from `block_thumbnail` through `render_png`), Edit in place
  (REFEDIT/BEDIT; the bar saves or discards; REFCLOSE saves), Define attribute (ATT), Edit
  attributes (ATTEDIT, or a double click on an attribute: `AttributesDialog`), Attribute manager
  (BATTMAN), Design Center (another drawing's blocks with their attribute definitions: insert,
  prompting for them, or redefine one this drawing has),
  Tool palette (`tool_palette` in `settings.json`, blocks and command lines; kept apart from the
  Settings dialog like `recents`). Command line B, I, W, REFEDIT, BEDIT, REFCLOSE, ATT, ATTEDIT,
  BATTMAN.

## Layers, groups and properties

- `qiyas-core/src/layers.rs`: `SetLayer { name, prop: on | frozen | locked | color | linetype |
  lineweight | plot, value }` (JSON value; a layer colour is "ACI n" or "#rrggbb"; the current
  layer can't be frozen), `NewLayer`, `DeleteLayer` (never 0, Defpoints, the current layer or one
  with entities, inside blocks too), `RenameLayer` (codec `rename_layer`: entities, layer states;
  take-off rows' `layer`, the current and hidden layers follow; a name another layer has is
  refused). Off and frozen layers are left out of the batch, counts and measures as before
  (`layer_shown`). Locked layers: `sheet_targets` refuses their entities for every command that
  changes what it is given (modify, edit, erase, set and match properties, text, dimension and
  table edits, blocks); `sheet_targets_any` lets them through where they are only read or not
  changed (cutters and boundaries, what a dimension or field measures, groups, a viewport's
  freezes, a match's source). A document step that moves `Session::current`, the hidden layers
  or the layer states' hidden layers keeps them as they were (`undo::ViewState`) and undo and
  redo put them back. Layer states are AutoCAD's own (`ACAD_LAYERSTATES` XRecords, codec
  `capture_layer_state`): `SaveLayerState` (Qiyas's current layer as the state's; the layer
  table's extension dictionary is made outside the step so an undo never leaves a dangling
  pointer), `RestoreLayerState` (the header's current layer is put back), `RenameLayerState`,
  `DeleteLayerState`; the hidden layers at save time live in the project file (`qiyas_hidden`)
  and come back on restore; the current layer stays thawed. `TranslateLayers { from_path,
  mappings: [{ from, to, color }] }`: renames, or merges into an existing `to` (entities move,
  viewports' freezes follow, the old layer goes), then the standard's layer properties (another
  drawing, read-only; IPC `drawing_layers`) and the mapping's colour. `SetViewportFrozen {
  viewport, layer, frozen }` edits a paper-space `Viewport.frozen_layers` (the Layers panel's VP
  freeze column, VPLAYER). `DrawingInfo` has `linetypes`, `layer_states`, `groups`,
  `viewports` (the sheet on screen's, not number 1). The window draws each viewport's model space
  without its frozen layers (see Clipboard, viewports, UCS, sheet sets and recovery).
- `qiyas-core/src/groups.rs`: `CreateGroup { name, targets }`, `AddToGroup`, `RemoveFromGroup`,
  `DeleteGroup` (AutoCAD's explode of a group: the entities stay), `Ungroup { targets }` (every
  group holding one): `GROUP` objects in `ACAD_GROUP`, members carry the group as a reactor; sheet
  entities only. `GroupInfo` lists each member's kind. Select: a click on a grouped entity selects the group (Home, Layers, "Group
  selection" is PICKSTYLE; Ctrl picks one alone); boxes pick entities.
- `qiyas-core/src/props.rs`: `SetProperty { target, prop, value }` (layer, color, linetype,
  lineweight, text, text_height, text_rotation and insert_rotation in degrees, text_style, radius,
  closed, global_width, insert_scale, dim_style; sheet entities only) and `MatchProperties {
  source, targets, props }` (the source may be inside a block or an xref; an xref's layer is
  never given to an entity). Insert scale and rotation scale and turn the reference about its
  insertion point (`modify::place`), so its attributes follow; the scale is the size of its x
  scale, a mirrored reference stays mirrored and `EntityInfo::scale` is negative for one. A dimension's text
  height is a `DSTYLE` DIMTXT override (`qiyas_geom::set_dimtxt`, before DIMSCALE) and its block
  is drawn again; a table's sets every cell; a change of shape runs `follow_edit`; rows on an
  entity take its new layer. `EntityInfo` has `rotation`, `closed`, `global_width`, `scale`, and
  a dimension's text `height`.
- Window: the Layers panel opens a layer's properties under its row (on, frozen, locked, plot,
  colour, linetype, lineweight, rename, delete), adds layers, and opens Layer states… (LAYSTATE,
  LAS) and Translate layers… (LAYTRANS, `LayerTranslator`); the properties palette edits the
  picked host entity in place (xref and block content stay read-only) and, with nothing picked,
  what new entities are drawn with. Home tab, Layers group; command line LAYER (LA: the panel),
  LAYSTATE (LAS), LAYTRANS, VPLAYER (the VP freeze column), GROUP (G: pick, Enter, name),
  UNGROUP, MATCHPROP (MA: source, then targets, Enter), PROPERTIES (PR, CH: the palette).

## Clipboard, viewports, UCS, sheet sets and recovery

- Clipboard (`qiyas-core/src/clipboard.rs`): `CopyToClipboard { targets, base }` (base null: the
  lower left of what they draw) makes a small DXF drawing of the sheet entities with the layers,
  linetypes, styles and blocks they use, marked `CLIPBOARD_MARK` (a `999` comment), coordinates
  from the base; not an undo step, `Outcome::Clipboard`. `PasteFromClipboard { at, text, as_block,
  height }` reads it through the block `Importer` (new handles; a missing layer comes with its
  colour, a block or style of that name already there is kept), base at `at`, one step; as a
  block (PASTEBLOCK) a new `A$Cnnnnnnnn` block under one reference; plain text becomes a Text
  (MText for several lines) 2.5 mm at the annotation scale. The desktop `run_command` fills `text`
  from the system clipboard (`arboard`) and puts a copy's text on it. Home tab, Clipboard group;
  Ctrl+C (COPYCLIP), Ctrl+Shift+C (COPYBASE), Ctrl+V (PASTECLIP), Ctrl+Shift+V (PASTEBLOCK); Ctrl+C
  with text selected in a panel copies the text.
- Paper-space viewports (`qiyas-core/src/viewports.rs`): `MView { corner1, corner2 }` (wire
  `mview`; layouts only) adds a `Viewport` on the current layer showing model extents
  (`set_scale`). Its frame draws (flatten's Viewport arm, never number 1); a click inside one with
  nothing of the layout near picks it. `paper_views()` gives each viewport's frame, model centre,
  scale and, when it freezes layers, the spans of the model batch it draws; the window fetches
  model space once (`sheet_batch("*Model_Space")`, `Session::with_sheet` keeps two sheets beside
  the one on screen) and `gl.ts` draws it per viewport with its own camera and a scissor box
  (`src/viewport/inset.ts`): fills, lines, text (wipeouts there hide no lines; pictures and
  twisted views are not drawn yet). The palette sets `viewport_scale` and `viewport_centre`.
  View tab, Viewports group; MVIEW (MV).
- Split view: VPORTS (VPORTS 1 / 2, or View, Two viewports) shows `SplitPane` beside the main
  view on the drawing on screen: own camera and sheet, the main batch shared on the same sheet,
  view only (tools work in the main view). Two drawings side by side wait for a multi-session
  window (founder's ruling).
- UCS (`qiyas-core/src/ucs.rs`): `Ucs { origin, x_axis }` (`UcsObject { target }`: a line's,
  polyline's, arc's, circle's, text's or block reference's), view state (no undo step) in
  `Session::ucs`, `DrawingInfo::ucs` and the project file; saving writes the header (UCSORG,
  UCSXDIR, UCSYDIR) and a `QIYAS` UCS table entry; a drawing without a project file takes its
  header's. The status bar shows "UCS x, y", typed points (`x,y`, `@dx,dy`, `@d<a`), ortho, polar
  and the grid run along it (`src/tools/ucs.ts`). UCS (origin, axis point), UCS OB, UCS W.
- Sheet sets (`qiyas-core/src/sheetset.rs`, `src/shell/SheetSetManager.tsx`): a `.qss` JSON `{ name,
  drawings: [{ path, layouts }] }`, paths relative with `/` and inside its folder (others refused,
  on save and on load); only the `.qss` is written. New from open drawings, open, add, remove,
  open all; Print all waits for plotting (Phase 4). SHEETSET or View, Sheet set….
- Autosave (`qiyas-core/src/autosave.rs`): every `autosave_minutes` (settings.json and the
  Settings dialog, 5 by default, 0 off) `autosave_drawings` writes each tab with unsaved changes
  (not during an edit in place) to `<name>.sv$` (a whole DWG) and `<name>.sv$.qiyas`, each to a
  temp file renamed over. On launch (the recent drawings) and before an open, `RecoveryDialog`
  offers each autosave newer than its drawing: Recover opens it in a new tab as
  `<name> (recovered).<ext>` with its take-off, unsaved, then Save As; Discard deletes it; Later
  opens the drawing itself. A save deletes the drawing's autosave. Lock files and the panic hook
  are Phase 4's.
- RECOVER (Home tab, Recover…): opens a damaged drawing through the failsafe reader (as every open
  does) and `RecoverReportDialog` lists what it read and dropped by kind (`ReadReport`: DWG object
  types by number, else the reader's code). A truncated DWG loses its file header and does not
  open at all. AUDIT is Phase 4's.

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

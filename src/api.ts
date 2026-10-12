/** Thin wrappers over the Tauri commands of qiyas-desktop (crates/qiyas-desktop/src/commands.rs). */
import { invoke } from "@tauri-apps/api/core";

export type Sheet = { name: string; block: string; is_model: boolean };

export type LayerInfo = {
  name: string;
  color: [number, number, number];
  /** Its colour as "ACI n" or "#rrggbb" (optional in tests that don't edit layers). */
  color_name?: string;
  off: boolean;
  frozen: boolean;
  locked: boolean;
  linetype?: string;
  /** Hundredths of a millimetre; -3 Default. */
  lineweight?: number;
  /** Plotted. */
  plot?: boolean;
};

/** A group of the drawing: its name and entities (`qiyas_core::GroupInfo`). */
export type GroupInfo = { name: string; handles: number[]; kinds: string[]; selectable: boolean };

/** A paper-space viewport of the sheet on screen and the layers it freezes on its own. */
export type ViewportInfo = { handle: number; frozen: string[] };

/** A layer property `set_layer` changes: on, frozen, locked and plot take booleans; color
 *  "ACI n" or "#rrggbb"; linetype a name; lineweight a number. */
export type LayerProp = "on" | "frozen" | "locked" | "color" | "linetype" | "lineweight" | "plot";

/** A property the palette edits (`qiyas_core::EntityProp`). */
export type EntityProp =
  | "layer"
  | "color"
  | "linetype"
  | "lineweight"
  | "text"
  | "text_height"
  | "text_rotation"
  | "text_style"
  | "radius"
  | "closed"
  | "global_width"
  | "insert_scale"
  | "insert_rotation"
  | "dim_style"
  /** A paper-space viewport's scale (layout units per model unit) and the model point at its
   *  centre ([x, y]). */
  | "viewport_scale"
  | "viewport_centre";

/** What match properties copies (`qiyas_core::MatchProp`). */
export type MatchProp =
  | "layer"
  | "color"
  | "linetype"
  | "lineweight"
  | "text_style"
  | "text_height"
  | "dim_style"
  | "hatch_pattern";

export const MATCH_ALL: MatchProp[] = [
  "layer",
  "color",
  "linetype",
  "lineweight",
  "text_style",
  "text_height",
  "dim_style",
  "hatch_pattern",
];

/** One line of the layer translator. */
export type LayerMapping = { from: string; to: string; color?: string | null };

export type Units = "inches" | "feet" | "millimetres" | "centimetres" | "metres" | "unknown";

/** One xref (another drawing shown in this one, `qiyas_core::XrefInfo`). */
export type XrefInfo = {
  name: string;
  /** The file was found. */
  found: boolean;
  /** The file was found and read (false when unreadable or it is the drawing itself). */
  loaded: boolean;
  /** The user chose its file ("Locate…"); "Clear chosen file" forgets it. */
  chosen: boolean;
  /** The file opened, or the path the drawing stores when none was found. */
  path: string;
  overlay: boolean;
  /** Unloaded in the drawing: listed, not opened. */
  unloaded: boolean;
  /** Layers it adds to the list, named `NAME|LAYER`. */
  layer_count: number;
  /** What went wrong finding or reading it (also in the drawing's `problems`). */
  problems: string[];
};

export type DrawingInfo = {
  path: string;
  sheets: Sheet[];
  /** The drawing's layers, then its xrefs' as `NAME|LAYER`, sorted by name. */
  layers: LayerInfo[];
  units: Units;
  problems: string[];
  /** The drawing holds its Arabic text backwards (typed in visual order for old CAD fonts). */
  arabic_reversed: boolean;
  /** Its xrefs and those inside them, in load order. */
  xrefs: XrefInfo[];
  /** Block name of the sheet on screen. */
  sheet: string;
  /** The layers the user has hidden, sorted. */
  hidden_layers: string[];
  /** What new entities are drawn with. */
  current: Current;
  /** The drawing's own text styles, sorted. */
  text_styles: string[];
  /** The style new text takes: the drawing's current one, or "Standard". */
  text_style: string;
  /** The drawing's own dimension styles, sorted. */
  dim_styles: DimStyleInfo[];
  /** The style new dimensions take: the drawing's current one, or "Standard". */
  dim_style: string;
  /** The annotation scale (CANNOSCALE), e.g. "1:50". */
  annotation_scale: string;
  /** Drawing units per paper unit it gives (50 for "1:50"): new text and annotations are that
   *  many times their paper size. */
  annotation_factor: number;
  /** The block being edited in place, if one is. */
  block_edit: string | null;
  /** The drawing's linetypes, sorted (optional in older tests). */
  linetypes?: string[];
  /** The drawing's layer states. */
  layer_states?: string[];
  /** The drawing's groups. */
  groups?: GroupInfo[];
  /** The paper-space viewports of the sheet on screen. */
  viewports?: ViewportInfo[];
  /** The user coordinate system (world when left out). */
  ucs?: Ucs;
};

/** A user coordinate system: an origin and a unit x axis in the plan (`qiyas_core::Ucs`). */
export type Ucs = { origin: P; x_axis: P };

/** One viewport of a layout and how model space draws inside it (`qiyas_core::PaperView`):
 *  its frame on the layout, the model point at its centre, layout units per model unit, and the
 *  spans [first, count] of the model batch's segments, fills and glyph quads it draws (null:
 *  all of them). */
export type PaperView = {
  handle: number;
  rect: [P, P];
  centre: P;
  scale: number;
  lines: [number, number][] | null;
  fills: [number, number][] | null;
  texts: [number, number][] | null;
};

/** A dimension style's common fields, in drawing units before its `scale` (DIMASZ, DIMTXT,
 *  DIMDEC, DIMEXO, DIMEXE, DIMGAP, DIMSCALE). */
export type DimStyleFields = {
  arrow_size: number;
  text_height: number;
  precision: number;
  ext_offset: number;
  ext_extension: number;
  gap: number;
  scale: number;
};

export type DimStyleInfo = { name: string; fields: DimStyleFields };

/** A dimension to add (`qiyas_core::DimSpec`); points in drawing units, `rotation` in degrees
 *  (null: horizontal or vertical by where `line_point` is). */
export type DimSpec =
  | { kind: "linear"; p1: P; p2: P; line_point: P; rotation: number | null }
  | { kind: "aligned"; p1: P; p2: P; offset: number }
  | { kind: "angular"; vertex: P; p1: P; p2: P; arc_point: P }
  | { kind: "radius"; target: EntityTarget; point: P }
  | { kind: "diameter"; target: EntityTarget; point: P }
  | { kind: "arc_length"; target: EntityTarget; point: P }
  | { kind: "ordinate"; feature: P; leader_end: P }
  | { kind: "baseline"; from: EntityTarget; points: P[] }
  | { kind: "continue"; from: EntityTarget; points: P[] }
  | { kind: "quick"; targets: EntityTarget[]; line_point: P };

/** What a dimension edit changes: its text (`<>` is the measurement, "" clears), its arrows, or
 *  where its text sits. */
export type DimEditOp = { text_override: string } | "flip" | { move_text: P };

/** In `problems` when the drawing opened with nothing in model space (`qiyas_core::EMPTY_MODEL`). */
export const EMPTY_MODEL = "This drawing has no geometry in Model";

export type EntityInfo = {
  handle: number;
  kind: string;
  /** The layer it is drawn on (layer "0" inside a block takes its insert's). */
  layer: string;
  /** `layer` as shown to the QS: a bound xref's layer without its `XREF$0$` (A-WALL). */
  layer_short: string;
  /** In drawing units, where its block references place it. */
  length: number | null;
  area: number | null;
  /** For a block reference, its block (a dynamic block by its own name). */
  block_name: string | null;
  text: string | null;
  /** `text` as read: an MText's without its formatting codes, its lines split by "\n". */
  plain_text?: string | null;
  /** The block the entity sits in, when it is inside one. */
  inside_block: string | null;
  /** The block references it sits inside, outermost first; empty on the sheet. */
  chain: number[];
  /** The block names along `chain`, outermost first. */
  block_path: string[];
  /** The xref the entity is in; `handle` is then that drawing's. */
  xref?: string | null;
  /** Its own colour: "ByLayer", "ByBlock", "None", "ACI n" or "#rrggbb". */
  color: string;
  /** The colour it is drawn with (ByLayer and ByBlock resolved). */
  color_rgb: [number, number, number];
  /** Its linetype name, or "ByLayer". */
  linetype: string;
  /** In hundredths of a millimetre; -1 ByLayer, -2 ByBlock, -3 Default. */
  lineweight: number;
  /** A circle's or an arc's radius, in drawing units. */
  radius: number | null;
  /** A block reference's attributes, as [tag, value]. */
  attributes: [string, string][];
  /** A text's height, in drawing units, and its style. */
  height: number | null;
  style: string | null;
  /** A table's cells. */
  table?: TableInfo | null;
  /** A text's or a block reference's own rotation, in degrees. */
  rotation?: number | null;
  /** A polyline is closed; its constant width. */
  closed?: boolean | null;
  global_width?: number | null;
  /** A block reference's scale. */
  scale?: number | null;
  /** A paper-space viewport's scale (layout units per model unit) and the model point at its
   *  centre. */
  viewport_scale?: number | null;
  viewport_centre?: P | null;
};

/** A table's cells for the table editor (`qiyas_core::TableInfo`). */
export type TableInfo = {
  /** Each cell's text as read (formatting codes gone), row by row. */
  cells: string[][];
  /** Column widths and row heights, in drawing units. */
  widths: number[];
  heights: number[];
  /** Merged cells: [top row, left column, bottom row, right column]. */
  merges: [number, number, number, number][];
  /** The cells holding a field: [row, column]. */
  fields: [number, number][];
};

/** A field (`qiyas_core::FieldKind`): today's date, the file name, an entity's area or length,
 *  or a count of a block's references on the sheet. */
export type FieldKind =
  | { kind: "date" }
  | { kind: "file_name" }
  | { kind: "area"; target: EntityTarget }
  | { kind: "length"; target: EntityTarget }
  | { kind: "count"; block: string };

/** What is under the cursor (`qiyas_core::Picked`): the entity, the block references it sits inside
 *  (outermost first, empty on the sheet) and the top-level entity on the sheet. Inside an xref,
 *  `chain` ends with the xref's reference and `xref` names it. */
/** `kind`: the kind of the entity at the top ("Line", "Insert", "Dimension"…). */
export type Picked = { top: number; entity: number; chain: number[]; xref: string | null; kind?: string | null };

/** Filter for the session's `find_entities` query and `extract_data`. Names match case-insensitively. */
export type EntityFilter = {
  /** Sheet block or layout name; the current sheet when null. */
  sheet?: string | null;
  layer?: string | null;
  kind?: string | null;
  block_name?: string | null;
  text_contains?: string | null;
  /** [[min_x, min_y], [max_x, max_y]]: entities with at least one point inside. */
  within?: [[number, number], [number, number]] | null;
  offset?: number;
  /** Rows per page, at most 500 (5,000 for `extract_data`); 100 when left out. */
  limit?: number;
};

/** One column of `extract_data` (`qiyas_core::Field`): lengths, areas, X and Y in metres. */
export type Field =
  | "handle"
  | "kind"
  | "layer"
  | "block_name"
  | "text"
  | "length"
  | "area"
  | "x"
  | "y"
  | "sheet"
  | "xref"
  | { attribute: string };

/** One entity found. Length and area in drawing units; bounds in world coordinates. */
export type EntityRow = {
  handle: number;
  kind: string;
  layer: string;
  block_name: string | null;
  text: string | null;
  length: number | null;
  area: number | null;
  bounds: [[number, number], [number, number]];
  /** Set for an entity of an xref; measure it with `chain`. */
  xref?: string;
  chain?: number[];
};

/** One page of results; `total` counts every match. */
export type Page<T> = { rows: T[]; total: number; next_offset: number | null };

export type TextRow = {
  handle: number;
  text: string;
  at: [number, number];
  height: number;
  inside_block: string | null;
  xref?: string;
};

/** A dimension; `measurement` in drawing units (degrees for angular), null when not computed. */
export type DimRow = {
  handle: number;
  kind: string;
  measurement: number | null;
  text_override: string | null;
  at: [number, number];
};

/** A table read off a sheet. `source` is the table entity's handle, or 0 for a text grid. */
export type Schedule = {
  name: string;
  sheet: string;
  source: number;
  columns: string[];
  rows: string[][];
};

/** References of one block on one layer; `attributes` are their distinct attribute values. */
export type BlockCount = { block_name: string; layer: string; count: number; attributes: string[] };

/** Opens the drawing in a new tab, or shows its tab when it is open already. */
export const openDrawing = (path: string) => invoke<DrawingInfo>("open_drawing", { path });

/** One file tab (`qiyas_desktop::tabs::TabInfo`). */
export type TabInfo = { id: number; path: string; title: string; dirty: boolean };

/** Every open drawing's tab, in order, and the one on screen (null when none is open). */
export type Tabs = { tabs: TabInfo[]; active: number | null };

export const tabs = () => invoke<Tabs>("tabs");

/** Shows the drawing of tab `id`; its info. The assistant starts a new conversation. */
export const activateTab = (id: number) => invoke<DrawingInfo>("activate_tab", { id });

/** What `closeTab` refuses with for a drawing with unsaved changes, without `force`. */
export const UNSAVED = "unsaved";

/** Closes tab `id`; the tabs left. */
export const closeTab = (id: number, force = false) => invoke<Tabs>("close_tab", { id, force });

/** A drawing opened or saved lately. */
export type Recent = { path: string; name: string; folder: string; missing: boolean };

/** The recent drawings, most recent first (at most 10). */
export const recents = () => invoke<Recent[]>("recents");

export const clearRecents = () => invoke<void>("clear_recents");

/** A named view of model space: its name and the box it shows on the drawing. */
export type NamedView = { name: string; bounds: [[number, number], [number, number]] };

export const views = () => invoke<NamedView[]>("views");

export const drawingInfo = () => invoke<DrawingInfo>("drawing_info");

export const setSheet = (block: string) => invoke<void>("set_sheet", { block });

export const setLayerHidden = (name: string, hidden: boolean) =>
  invoke<void>("set_layer_hidden", { name, hidden });

/** Looks for every xref again and reopens it; the drawing's info with the new list. */
export const reloadXrefs = () => invoke<DrawingInfo>("reload_xrefs");

/** Uses the file `path` for the xref `name` (null: look for it again), then reloads the xrefs.
 *  Saving the project remembers the choice. */
export const setXrefPath = (name: string, path: string | null) =>
  invoke<DrawingInfo>("set_xref_path", { name, path });

/** Names the current sheet's batch: the same number means the same batch. */
export const batchVersion = () => invoke<number>("batch_version");

/** What `render_batch` sends instead of a batch that is still `ifVersion`: "QINM" and four zeros. */
export function isNotModified(bytes: ArrayBuffer): boolean {
  const marker = [0x51, 0x49, 0x4e, 0x4d, 0, 0, 0, 0];
  const got = new Uint8Array(bytes);
  return got.length === marker.length && marker.every((b, i) => got[i] === b);
}

/** Raw bytes of the current sheet's batch; decode with `parseBatch`. `null` when the batch is
 *  still `ifVersion` (from `batchVersion`), so the caller keeps the one it has. `pxPerUnit` is the
 *  zoom to draw the text glyphs for (screen pixels per drawing unit); left out, the core keeps the
 *  zoom of its batch, or fits a new sheet. */
export async function renderBatch(
  ifVersion?: number,
  pxPerUnit?: number,
): Promise<ArrayBuffer | null> {
  const bytes = await invoke<ArrayBuffer>("render_batch", { ifVersion, pxPerUnit });
  return isNotModified(bytes) ? null : bytes;
}

/** Names the batch of the sheet `sheet` (block name), which need not be the one on screen. */
export const sheetBatchVersion = (sheet: string) => invoke<number>("sheet_batch_version", { sheet });

/** `renderBatch` for the sheet `sheet`, which stays off screen: model space for a layout's
 *  viewports, or a split window's second view. */
export async function sheetBatch(
  sheet: string,
  ifVersion?: number,
  pxPerUnit?: number,
): Promise<ArrayBuffer | null> {
  const bytes = await invoke<ArrayBuffer>("sheet_batch", { sheet, ifVersion, pxPerUnit });
  return isNotModified(bytes) ? null : bytes;
}

/** The viewports of the sheet `sheet` (the one on screen when left out) and how model space
 *  draws inside each. */
export const paperViews = (sheet?: string) => invoke<PaperView[]>("paper_views", { sheet: sheet ?? null });

/** Raw bytes of the glyph atlas pages changed after `sinceVersion` (all when left out); decode
 *  with `parseAtlas`. */
export const atlasPages = (sinceVersion?: number) =>
  invoke<ArrayBuffer>("atlas_pages", { sinceVersion });

/** How `attachPdf` brings a page in: a picture under the drawing, kept in the DWG as a PDF underlay,
 *  or its vectors and texts as entities. */
export type AttachMode = "underlay" | "import";

/** Event with the percent drawn (0 to 100) of the underlay picture pdfium is rendering. */
export const PDF_PROGRESS = "pdf:progress";

/** How many pages the PDF at `path` has. Fails with "PDF support is not installed: run
 *  scripts/pdfium.sh" when pdfium is missing. */
export const pdfPageCount = (path: string) => invoke<number>("pdf_page_count", { path });

/** Width and height in points (1/72 inch) of page `page` (from 1), as shown. */
export const pdfPageSize = (path: string, page: number) =>
  invoke<[number, number]>("pdf_page_size", { path, page });

/** Raw bytes of pages `first` to `first + count - 1` drawn to fit `px` x `px`; decode with
 *  `parseThumbnails`. */
export const pdfThumbnails = (path: string, first: number, count: number, px: number) =>
  invoke<ArrayBuffer>("pdf_thumbnails", { path, first, count, px });

/** Attaches page `page` of the PDF at `path` to the sheet on screen, its bottom-left corner at
 *  `at`, `scale` drawing units per point. Resolves to the underlay's handle, or for an import the
 *  number of entities added. One undo step. */
export const attachPdf = (
  path: string,
  page: number,
  at: [number, number],
  scale: number,
  mode: AttachMode,
) => invoke<number>("attach_pdf", { path, page, x: at[0], y: at[1], scale, mode });

/** True when the drawing itself has changes not saved yet (PDF imports, underlays, calibration);
 *  the take-off is saved in the project file after every command. */
export const isDirty = () => invoke<boolean>("is_dirty");

/** Rescales the PDF underlay or image under `p1` about `p1` so that `p1` to `p2` is `distance`
 *  drawing units. One undo step. */
export const calibrate = (p1: [number, number], p2: [number, number], distance: number) =>
  invoke<void>("calibrate", { x1: p1[0], y1: p1[1], x2: p2[0], y2: p2[1], distance });

/** Raw bytes of picture `id` (from the batch's image table: a PDF underlay or an image) for a zoom
 *  of `pxPerUnit` screen pixels per drawing unit; decode with `parsePicture`. */
export const underlayImage = (id: number, pxPerUnit: number) =>
  invoke<ArrayBuffer>("underlay_image", { id, pxPerUnit });

/** The raster image types Attach image offers. */
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp"];

/** Width and height in pixels of the image at `path`, as shown (its EXIF orientation applied). */
export const imageSize = (path: string) => invoke<[number, number]>("image_size", { path });

/** Attaches the image at `path` to the sheet on screen, bottom-left corner at `at`, `width` drawing
 *  units wide. Resolves to its handle. One undo step. */
export const attachImage = (path: string, at: [number, number], width: number) =>
  invoke<number>("attach_image", { path, x: at[0], y: at[1], width });

/** An image's brightness, contrast and fade, 0 to 100 each (50, 50 and 0: as it is). */
export type ImageDisplay = { brightness: number; contrast: number; fade: number };

/** The brightness, contrast and fade of the image `handle`; null when it is not an image. */
export const imageDisplay = (handle: number) =>
  invoke<ImageDisplay | null>("image_display", { handle });

/** The entity nearest to the drawing point (x, y) within `tol` drawing units, down to the entity
 *  inside any blocks. */
export const pick = (x: number, y: number, tol: number) =>
  invoke<Picked | null>("pick", { x, y, tol });

/** What a selection box from `bounds[0]` to `bounds[1]` (drawing coordinates) picks: the entities
 *  wholly inside it, or with `crossing` those touching it. Always whole entities at the top of the
 *  sheet (`top === entity`, no chain), sorted by handle. */
export const pickWindow = (bounds: [P, P], crossing: boolean) =>
  invoke<Picked[]>("pick_window", {
    minX: bounds[0][0],
    minY: bounds[0][1],
    maxX: bounds[1][0],
    maxY: bounds[1][1],
    crossing,
  });

/** What kind of point an object snap found (`qiyas_core::SnapKind`), best first. */
export type SnapKind =
  | "end"
  | "intersection"
  | "mid"
  | "centre"
  | "node"
  | "perpendicular"
  | "nearest";

/** A point found by an object snap, on the entity `handle` inside the block references `chain`. */
export type Snap = { point: [number, number]; kind: SnapKind; handle: number; chain: number[] };

export type SnapModes = Record<SnapKind, boolean>;

/** How the window draws and measures (`qiyas_core::Drafting`), saved in the project file. */
export type Drafting = {
  /** Object snaps on at all (F3). */
  osnap: boolean;
  snap_modes: SnapModes;
  /** Each next point straight across or up from the one before (F8). */
  ortho: boolean;
  /** Each next point's direction snapped to a multiple of `polar_step_deg` when close (F10). */
  polar: boolean;
  polar_step_deg: number;
  /** The dot grid shown (F7), `grid_spacing` drawing units apart, every tenth dot a major one. */
  grid: boolean;
  grid_spacing: number;
  /** Points go to the nearest grid dot when no object snap is found. */
  grid_snap: boolean;
  /** Decimals shown in the take-off and the status bar: 2, 3 or 4. */
  precision: number;
};

/** What `boundaryAt` finds (`qiyas_core::Boundary`): the ring around the point (counter-clockwise)
 *  and its islands, with each ring's bulges (the ring first; an edge along a drawn circle or arc
 *  is that arc), no closed area, or too much line work there to look through (zoom in). */
export type Boundary =
  | { found: "ring"; ring: [number, number][]; holes: [number, number][][]; bulges: number[][] }
  | { found: "none" }
  | { found: "too_dense" };

/** The closed area around (x, y) the drawn lines make, for a hatch: looked for `start` drawing
 *  units around it first, then 4 times as far each time until one fits. */
export const boundaryAt = (x: number, y: number, start: number) =>
  invoke<Boundary>("boundary_at", { x, y, start });

/** The object snap within `tol` drawing units of (x, y) with the drawing's snap modes; `from` is the
 *  previous point, for perpendicular snaps. Null when there is none or snaps are off. */
export const snap = (x: number, y: number, tol: number, from: [number, number] | null = null) =>
  invoke<Snap | null>("snap", { x, y, tol, fromX: from?.[0] ?? null, fromY: from?.[1] ?? null });

export const getDrafting = () => invoke<Drafting>("get_drafting");

/** Replaces the drafting settings (not an undo step). Rejects a value out of range. */
export const setDrafting = (drafting: Drafting) => invoke<void>("set_drafting", { drafting });

/** One entity; give `chain` for an entity inside blocks. */
export const entityInfo = (handle: number, chain: number[] = []) =>
  invoke<EntityInfo>("entity_info", { handle, chain });

/** The cell [row, column] of the table `handle` under `p`; a merged cell by its first. */
export const tableCellAt = (handle: number, p: [number, number]) =>
  invoke<[number, number] | null>("table_cell_at", { handle, x: p[0], y: p[1] });

export type Kind = "length" | "area" | "count" | "volume" | "item";

/** Who made a measurement: the user, the in-app AI, or an outside AI through MCP. */
export type Source = "user" | "ai" | "mcp";

/**
 * One take-off line. `value` is in drawing units (units² for areas, units³ for volumes, the
 * item's own unit for items); `shown` is in `unit`.
 */
export type Measurement = {
  id: string;
  kind: Kind;
  value: number;
  /** "m", "m2", "m3", "pcs", or an item's own unit. */
  unit: string;
  shown: number;
  description: string;
  layer: string;
  sheet: string;
  handles: number[];
  points: [number, number][];
  block_name: string | null;
  self_intersecting: boolean;
  source: Source;
  note: string;
  created: string;
  /** A volume's height, in drawing units. */
  height: number | null;
  /** Code of the linked BOQ item. */
  boq_code: string | null;
  /** The block references the measured entity sits inside, outermost first; empty otherwise. */
  chain: number[];
  /** The block names along `chain`, outermost first. */
  block_path: string[];
  /** The xref the measured entities are in (`chain` ends with its reference). */
  xref?: string | null;
};

export type Total = { kind: Kind; unit: Measurement["unit"]; count: number; sum: number };

/** One BOQ line (`qiyas_core::BoqItem`). */
export type BoqItem = {
  code: string;
  description: string;
  /** "m", "m2", "m3", "pcs", "kg", "t", or the workbook's own text. */
  unit: string;
  quantity: number;
  rate: number | null;
  /** The section heading above the item; "" when none. */
  section: string;
  /** Allowed difference in percent; null: the user's setting. */
  tolerance: number | null;
  /** The item's row in its sheet, from 1 as Excel shows it. */
  row: number;
};

/** An imported bill of quantities (`qiyas_core::Boq`). */
export type Boq = {
  /** The workbook it was read from. */
  source: string;
  /** RFC 3339. */
  imported: string;
  items: BoqItem[];
  /** Item rows left out because their quantity is not a number. */
  skipped_rows: number;
};

/** What a comparison found (`qiyas_core::ConflictKind`). */
export type ConflictKind =
  | "quantity_mismatch"
  | "missing_from_boq"
  | "not_in_drawings"
  | "schedule_vs_plan"
  | "drawing_vs_drawing"
  | "unit_doubt";

export type ConflictStatus = "open" | "resolved" | "ignored";

/** One conflict of a comparison (`qiyas_core::Conflict`). Quantities are in `unit`. */
export type Conflict = {
  /** 16 hex digits, the same on every comparison for the same finding. */
  id: string;
  kind: ConflictKind;
  boq_code: string | null;
  /** Ids of the measurements involved, sorted. */
  measurements: string[];
  /** Name of the schedule involved. */
  schedule: string | null;
  /** Names of the sheets involved (layout names), sorted. */
  sheets: string[];
  /** What the BOQ or the schedule (or the first sheet) says. */
  expected: number | null;
  /** What the drawings measure (or the second sheet counts). */
  found: number | null;
  unit: string;
  /** A short English sentence with the numbers in it. */
  reason: string;
  reason_ar: string;
  status: ConflictStatus;
  note: string;
  /** When it was first found, RFC 3339. */
  created: string;
  /** What `id` is hashed from, for debugging. */
  hash_key: string;
};

type P = [number, number];

/** A hatch's fill (`qiyas_core::HatchPattern`): a pattern of the built-in table by name (ANSI31,
 *  ANSI37, LINE, NET, DASH, BRICK) turned by `angle` degrees and scaled by `scale`. */
export type HatchPattern =
  | "solid"
  | "ansi31"
  | "ansi37"
  | { custom: { name: string; angle: number; scale: number } };

/** What a draw adds (`qiyas_core::DrawSpec`), in drawing coordinates; angles in degrees. */
export type DrawSpec =
  | { kind: "line"; a: P; b: P }
  /** One bulge per point (the segment starting there), or none. */
  | { kind: "polyline"; points: P[]; bulges: number[]; closed: boolean }
  | { kind: "rectangle"; a: P; b: P }
  | { kind: "polygon"; centre: P; sides: number; radius: number; inscribed: boolean }
  | { kind: "circle"; centre: P; radius: number }
  /** Counter-clockwise from `start` to `end`. */
  | { kind: "arc"; centre: P; radius: number; start: number; end: number }
  | { kind: "three_point_arc"; a: P; b: P; c: P }
  /** `major`: the major semi-axis from the centre; `ratio` minor over major (0 to 1]. */
  | { kind: "ellipse"; centre: P; major: P; ratio: number }
  | { kind: "spline"; fit_points: P[] }
  | { kind: "point"; p: P }
  /** Inner and outer diameters. */
  | { kind: "donut"; centre: P; inner: number; outer: number }
  | { kind: "ray"; origin: P; dir: P }
  | { kind: "xline"; origin: P; dir: P }
  | { kind: "revcloud"; points: P[]; arc_len: number }
  | { kind: "wipeout"; points: P[] }
  /** Inside the closed entities `boundary_handles` or the ring `points` (less the islands
   *  `holes`, with each ring's `bulges` as `Boundary` gives them): one of the two. */
  | {
      kind: "hatch";
      boundary_handles: number[];
      points: P[];
      holes?: P[][];
      bulges?: number[][];
      pattern: HatchPattern;
    }
  | { kind: "solid2d"; points: P[] }
  | { kind: "trace"; points: P[]; width: number }
  | { kind: "mline"; points: P[]; style: string | null; scale: number | null }
  | { kind: "sketch"; points: P[] };

/** An entity as picked: its handle and the block references it sits inside (empty on the sheet). */
export type EntityTarget = { handle: number; chain: number[] };

/** What a modify changes (`qiyas_core::ModifySpec`): entities at the top of the sheet, in drawing
 *  coordinates; angles in degrees counter-clockwise. */
export type ModifySpec =
  | { kind: "move"; targets: EntityTarget[]; delta: P }
  /** `count` copies, each `delta` further than the one before. */
  | { kind: "copy"; targets: EntityTarget[]; delta: P; count: number }
  | { kind: "rotate"; targets: EntityTarget[]; base: P; angle: number; copy: boolean }
  | { kind: "scale"; targets: EntityTarget[]; base: P; factor: number; copy: boolean }
  /** `keep`: mirror a copy and keep the originals. */
  | { kind: "mirror"; targets: EntityTarget[]; p1: P; p2: P; keep: boolean }
  /** `window`: [lower left, upper right]; the vertices inside it move. */
  | { kind: "stretch"; targets: EntityTarget[]; window: [P, P]; delta: P }
  /** One or two [source, destination] pairs. */
  | { kind: "align"; targets: EntityTarget[]; pairs: [P, P][] }
  | { kind: "array_rect"; targets: EntityTarget[]; rows: number; cols: number; dy: number; dx: number }
  /** `count` items, the originals included, each turned `angle` from the one before. */
  | { kind: "array_polar"; targets: EntityTarget[]; centre: P; count: number; angle: number }
  | { kind: "explode"; targets: EntityTarget[] };

/** A polyline edit (`qiyas_core::PEditOp`). */
export type PEditOp =
  | "close"
  | "open"
  | "fit_curve"
  | "decurve"
  | "convert_to_polyline"
  | { width: number }
  /** A vertex on the polyline nearest the point. */
  | { add_vertex: P }
  /** The vertex nearest the point goes. */
  | { delete_vertex: P };

/** What an edit changes (`qiyas_core::EditSpec`): entities at the top of the sheet, in drawing
 *  coordinates. `at_*` points are where the entity was clicked: which piece, end or side. */
export type EditSpec =
  /** No cutters: everything drawn cuts. */
  | { kind: "trim"; cutters: EntityTarget[]; target: EntityTarget; at_point: P }
  /** No boundaries: everything drawn. */
  | { kind: "extend"; boundaries: EntityTarget[]; target: EntityTarget; at_end: P }
  | { kind: "break"; target: EntityTarget; p1: P; p2: P }
  | { kind: "join"; targets: EntityTarget[] }
  | {
      kind: "lengthen";
      target: EntityTarget;
      at_end: P;
      by: { delta: number } | { total: number } | { percent: number };
    }
  | { kind: "reverse"; targets: EntityTarget[] }
  | { kind: "offset"; target: EntityTarget; distance: number; side_point: P }
  | { kind: "fillet"; a: EntityTarget; b: EntityTarget; at_a: P; at_b: P; radius: number }
  | { kind: "chamfer"; a: EntityTarget; b: EntityTarget; at_a: P; at_b: P; d1: number; d2: number }
  | { kind: "blend"; a: EntityTarget; b: EntityTarget; at_a: P; at_b: P }
  | { kind: "pedit"; target: EntityTarget; op: PEditOp }
  | { kind: "overkill"; targets: EntityTarget[]; tol: number }
  | { kind: "flatten"; targets: EntityTarget[] };

/** What new entities are drawn with (`qiyas_core::Current`). */
export type Current = {
  layer: string;
  /** "ByLayer", "ByBlock", "ACI n" or "#rrggbb". */
  color: string;
  /** "ByLayer", "ByBlock" or a linetype of the drawing. */
  linetype: string;
  /** Hundredths of a millimetre; -1 ByLayer, -2 ByBlock, -3 Default. */
  lineweight: number;
};

/** The command bus (`qiyas_core::Command`). Points are in drawing coordinates. */
export type Command =
  | { type: "measure_distance"; points: [number, number][] }
  | { type: "measure_area"; points: [number, number][] }
  /** `chain`: the block references the entity sits inside, outermost first; empty on the sheet. */
  | { type: "measure_entity"; handle: number; chain: number[] }
  | { type: "count_block"; block_name: string; layer: string | null }
  | { type: "remove_measurement"; id: string }
  | { type: "set_description"; id: string; text: string }
  | { type: "set_layer_hidden"; name: string; hidden: boolean }
  | { type: "set_sheet"; block: string }
  /** The drawing holds its Arabic text backwards; not an undo step. */
  | { type: "set_arabic_reversed"; on: boolean }
  /** The drafting settings; not an undo step. */
  | { type: "set_drafting"; drafting: Drafting }
  /** Length, area and count values come from the geometry (points or handles, not both). */
  | {
      type: "add_measurement";
      kind: Kind;
      value: number | null;
      unit: string | null;
      height: number | null;
      points: [number, number][];
      handles: number[];
      description: string;
      note: string;
    }
  | { type: "set_height"; id: string; height: number }
  | { type: "link_boq"; id: string; code: string | null }
  | { type: "set_note"; id: string; text: string }
  /** Replaces the BOQ; `importBoq` reads one from Excel and runs this. */
  | { type: "set_boq"; boq: Boq }
  /** Allowed difference in percent (0 or more) for the BOQ items with `code`; null: the user's setting. */
  | { type: "set_boq_tolerance"; code: string; percent: number | null }
  /** Replaces the conflicts and schedules; `compare` runs this. */
  | { type: "set_conflicts"; conflicts: Conflict[]; schedules: Schedule[] }
  | { type: "resolve_conflict"; id: string; status: ConflictStatus; note: string }
  /** `attachPdf` runs these two; `scale` is drawing units per PDF point. */
  | { type: "import_pdf"; path: string; page: number; insert_at: [number, number]; scale: number }
  | {
      type: "attach_pdf_underlay";
      path: string;
      page: number;
      insert_at: [number, number];
      scale: number;
    }
  /** `calibrate` runs this. */
  | {
      type: "calibrate";
      handle: number;
      p1: [number, number];
      p2: [number, number];
      real_distance: number;
    }
  /** `attachImage` runs this; `width_units` is the image's width in drawing units. */
  | { type: "attach_image"; path: string; insert_at: [number, number]; width_units: number }
  /** 0 to 100 each; one undo step. */
  | {
      type: "set_image_display";
      handle: number;
      brightness: number;
      contrast: number;
      fade: number;
    }
  /** The entity's geometry as an `info` outcome; `at` picks a polyline's nearest segment. */
  | { type: "measure_geometry"; handle: number; chain: number[]; at: [number, number] | null }
  /** Points on layer QIYAS-POINT dividing the path into `count` parts (2 to 1000), counted. */
  | { type: "divide_along"; handle: number; chain: number[]; count: number }
  /** A point every `step` drawing units along the path from its start, counted. */
  | { type: "measure_along"; handle: number; chain: number[]; step: number }
  /** Where `p` is, in metres, and the snap that found it, as an `info` outcome. */
  | { type: "id_point"; p: [number, number]; kind: SnapKind | null }
  /** A `table` outcome: one row of `fields` per entity `filter` finds (up to 5,000). */
  | { type: "extract_data"; filter: EntityFilter; fields: Field[] }
  /** Saves the named view `name` (replacing one of that name) showing `bounds`; Model sheet only. */
  | { type: "save_view"; name: string; bounds: [[number, number], [number, number]] }
  /** Removes the named view `name`; `nothing` when there is none. */
  | { type: "delete_view"; name: string }
  /** Adds what the spec describes on the current layer: one undo step, an `added` outcome. */
  | ({ type: "draw" } & DrawSpec)
  /** What new entities are drawn with; not an undo step. */
  | { type: "set_current"; current: Current }
  /** Deletes entities at the top of the sheet: one undo step. */
  | { type: "delete_entities"; targets: EntityTarget[] }
  /** Moves, copies, turns… entities at the top of the sheet: one undo step (`changed`, or `added`
   *  with the new handles). */
  | ({ type: "modify" } & ModifySpec)
  /** Trims, extends, breaks, joins… entities at the top of the sheet: one undo step (`edited`, or
   *  `added` for offset and blend). */
  | ({ type: "edit" } & EditSpec)
  /** One line of text on the current layer; `rotation` in degrees; `style` null: the drawing's
   *  current one. One undo step, an `added` outcome. */
  | { type: "text"; at: P; height: number; rotation: number; value: string; style: string | null }
  /** Plain text (lines split by "\n") in a box `width` wide (0: no wrapping), top left at `at`. */
  | { type: "mtext"; at: P; width: number; height: number; value: string; style: string | null }
  /** A text's or an MText's new value (plain text), or a block reference's attribute `tag`'s. */
  | { type: "edit_text"; target: EntityTarget; value: string; tag: string | null }
  /** One undo step (none when nothing matches): a `replaced` outcome. */
  | {
      type: "find_replace";
      find: string;
      replace: string;
      scope: TextScope;
      match_case: boolean;
      whole_word: boolean;
    }
  /** The matches, without changing anything: a `found` outcome. */
  | { type: "find_text"; find: string; scope: TextScope; match_case: boolean; whole_word: boolean }
  /** Dimensions on the current layer in the current style, each with its block: an `added`
   *  outcome, one undo step. */
  | ({ type: "dimension" } & DimSpec)
  | { type: "dim_edit"; target: EntityTarget; op: DimEditOp }
  /** A gap in the dimension's lines where `cutter` crosses them. */
  | { type: "dim_break"; target: EntityTarget; cutter: EntityTarget }
  /** Parallel linear dimensions `spacing` apart going out from `base` (null: twice the text). */
  | { type: "dim_space"; base: EntityTarget; targets: EntityTarget[]; spacing: number | null }
  /** A jog in a linear dimension's line nearest `at`. */
  | { type: "dim_jog_line"; target: EntityTarget; at: P }
  /** A leader through `points` (arrow at the first) with `text` beside the last: `added` with
   *  the leader, then the text. MLEADER adds a short landing. */
  | { type: "leader"; points: P[]; text: string }
  | { type: "mleader"; points: P[]; text: string }
  /** A feature control frame (`{\Fgdt;p}` a symbol, `%%v` between fields) at `at`. */
  | { type: "tolerance"; at: P; frame: string }
  /** CANNOSCALE, "paper:drawing" such as "1:50". */
  | { type: "annotation_scale"; name: string }
  /** The style `name` (from the current one when new) takes `fields`; `current` makes it the one
   *  new dimensions take. */
  | { type: "set_dim_style"; name: string; fields: DimStyleFields; current: boolean }
  /** A table on the current layer, top left at `at`, cells filled from `data` (plain text): an
   *  `added` outcome, one undo step. The others change a table, one undo step each. */
  | {
      type: "table";
      at: P;
      rows: number;
      cols: number;
      col_width: number;
      row_height: number;
      data: string[][];
    }
  | { type: "edit_cell"; target: EntityTarget; row: number; col: number; text: string }
  | { type: "insert_row"; target: EntityTarget; at: number }
  | { type: "insert_col"; target: EntityTarget; at: number }
  | { type: "delete_row"; target: EntityTarget; row: number }
  | { type: "delete_col"; target: EntityTarget; col: number }
  /** One cell (`first` = `last`) unmerges the merged cell it is in. */
  | { type: "merge_cells"; target: EntityTarget; first: [number, number]; last: [number, number] }
  /** A one-line text holding the field: an `added` outcome. */
  | { type: "field"; at: P; height: number; field: FieldKind }
  /** Works every field out again: one undo step when one changes. */
  | { type: "update_fields" }
  /** A new block from `targets` (they leave the sheet, relative to `base`); with
   *  `convert_to_insert` a reference takes their place. An `edited` outcome. */
  | { type: "create_block"; name: string; base: P; targets: EntityTarget[]; convert_to_insert: boolean }
  /** A reference to `name` on the current layer, `rotation` in degrees; attributes by tag. With
   *  `from_path`, a block the drawing lacks is copied from that drawing first. */
  | {
      type: "insert_block";
      name: string;
      at: P;
      scale: number;
      rotation: number;
      attribute_values: Record<string, string>;
      from_path?: string | null;
    }
  /** Writes the block to a .dwg in the drawing's folder (the Save dialog uses `writeBlock`). */
  | { type: "write_block"; name: string; path: string }
  /** Edits the reference's block in place until saved or discarded: not an undo step. */
  | { type: "edit_block_in_place"; target: EntityTarget }
  /** End the edit in place, the block taking what was edited or not: a `block_edit_closed`
   *  outcome, no undo step (the edit's steps leave the history). */
  | { type: "save_block_edits" }
  | { type: "discard_block_edits" }
  | { type: "att_def"; tag: string; prompt: string; default: string; at: P; height: number }
  | { type: "edit_attributes"; target: EntityTarget; values: Record<string, string> }
  | { type: "edit_att_def"; block: string; tag: string; prompt: string; default: string }
  /** Every reference gets the block's attributes as defined now, keeping each tag's value. */
  | { type: "sync_attributes"; block: string }
  | { type: "redefine_block"; name: string; from_path: string }
  /** Layer states, kept in the drawing as AutoCAD keeps them (the hidden layers beside them). */
  | { type: "save_layer_state"; name: string }
  | { type: "restore_layer_state"; name: string }
  | { type: "delete_layer_state"; name: string }
  | { type: "rename_layer_state"; from: string; to: string }
  | { type: "set_layer"; name: string; prop: LayerProp; value: boolean | string | number }
  | { type: "new_layer"; name: string }
  | { type: "delete_layer"; name: string }
  | { type: "rename_layer"; from: string; to: string }
  | { type: "translate_layers"; from_path: string | null; mappings: LayerMapping[] }
  | { type: "set_viewport_frozen"; viewport: EntityTarget; layer: string; frozen: boolean }
  /** An empty name: the app asks for one (GROUP). */
  | { type: "create_group"; name: string; targets: EntityTarget[] }
  | { type: "add_to_group"; name: string; targets: EntityTarget[] }
  | { type: "remove_from_group"; name: string; targets: EntityTarget[] }
  | { type: "delete_group"; name: string }
  | { type: "ungroup"; targets: EntityTarget[] }
  | { type: "match_properties"; source: EntityTarget; targets: EntityTarget[]; props: MatchProp[] }
  | { type: "set_property"; target: EntityTarget; prop: EntityProp; value: boolean | string | number | P }
  /** The clipboard: the entities as Qiyas's clipboard text (base null: their lower left), and
   *  a paste of the system clipboard at `at` (as one block reference with `as_block`). */
  | { type: "copy_to_clipboard"; targets: EntityTarget[]; base: P | null }
  | { type: "paste_from_clipboard"; at: P; as_block: boolean; height?: number | null }
  /** A paper-space viewport on the layout on screen. */
  | { type: "mview"; corner1: P; corner2: P }
  /** The user coordinate system (not an undo step): an origin and an x axis, or an entity's. */
  | { type: "ucs"; origin: P; x_axis: P }
  | { type: "ucs_object"; target: EntityTarget };

/** Where find and replace looks: the sheet on screen, or every sheet. */
export type TextScope = "sheet" | "drawing";

/** An entity holding a match (a block reference, for an attribute) and its sheet (block name). */
export type TextMatch = { handle: number; sheet: string };

export type Outcome =
  | { type: "measured"; measurement: Measurement }
  | { type: "removed"; id: string }
  | { type: "changed" }
  /** Entities added to the drawing (PDF import or underlay). */
  | { type: "added"; handles: number[] }
  /** An explode's parts, how many entities picked had none and were left as they are, and the
   *  take-off rows that followed (as `takeoff_followed`). */
  | { type: "exploded"; handles: number[]; left_alone: number; remeasured: number; unlinked: number }
  /** An update of fields or attributes that left locked entities as they were: how many changed,
   *  how many locked ones were left. */
  | { type: "updated"; count: number; skipped: number }
  /** An erase (or a move, turn, scale... that left a row unmeasurable): take-off rows measured
   *  again without an erased entity, and rows that lost their link (their values stay). */
  | { type: "takeoff_followed"; remeasured: number; unlinked: number }
  /** An edit in place: entities it added and removed, how many picked it left as they were, how
   *  many take-off rows lost their link (their entities could no longer be measured), and for a
   *  join how many rows now measure the joined polyline. */
  | {
      type: "edited";
      added: number[];
      removed: number[];
      left_alone: number;
      unlinked: number;
      joined_rows: number;
    }
  /** A find and replace: how many replacements, the entities changed, and how many texts were
   *  left as they were (a text can't be left blank or break onto two lines). */
  | { type: "replaced"; count: number; handles: number[]; skipped: number }
  /** A find: how many matches, and each entity holding one with its sheet. */
  | { type: "found"; count: number; matches: TextMatch[] }
  | { type: "nothing" }
  /** Facts to show: rows of a label key ("radius", "centre"…) and its value written out. */
  | { type: "info"; title: string; rows: [string, string][] }
  | { type: "table"; columns: string[]; rows: string[][] }
  /** An edit in place ended: rows that lost their link, undo steps that left with it. */
  | { type: "block_edit_closed"; name: string; saved: boolean; unlinked: number; dropped: number }
  /** A copy went onto the system clipboard: how many entities. */
  | { type: "clipboard"; text: string; count: number };

export const runCommand = (cmd: Command) => invoke<Outcome>("run_command", { cmd });

/** False when there was nothing to undo. */
export const undo = () => invoke<boolean>("undo");

/** False when there was nothing to redo. */
export const redo = () => invoke<boolean>("redo");

export const takeoff = () => invoke<Measurement[]>("takeoff");

export const totals = () => invoke<Total[]>("totals");

/** Saves the take-off to the drawing's `.qiyas` project file; resolves to the path written. */
export const saveProject = () => invoke<string>("save_project");

/** Saves the drawing over its own file with the measurements embedded, and its project file. */
export const saveDrawing = () => invoke<void>("save_drawing");

/** Saves the drawing to `path` (.dwg or .dxf); from then on the open drawing is that file. */
export const saveDrawingAs = (path: string) => invoke<void>("save_drawing_as", { path });

/** Writes the take-off and its totals to an Excel workbook at `path`. */
export const exportTakeoff = (path: string) => invoke<void>("export_takeoff", { path });

/** Writes a table (from `extract_data`) to an Excel workbook at `path`: one sheet "Data". */
export const exportTable = (path: string, columns: string[], rows: string[][]) =>
  invoke<void>("export_table", { path, columns, rows });

/** Reads the BOQ in the Excel workbook at `path` (English or Arabic headers) and makes it the
 *  drawing's BOQ, as an undo step. Rejects with "No BOQ header found. …" when no sheet has one. */
export const importBoq = (path: string) => invoke<Boq>("import_boq", { path });

/** The drawing's BOQ; null when none is imported. */
export const boq = () => invoke<Boq | null>("boq");

/** Compares the BOQ and the take-off with the schedules and block counts of every sheet and keeps
 *  the conflicts, as an undo step; resolves to them. `tolerance` is the allowed difference in
 *  percent for BOQ items with none of their own (5 when left out). Conflicts found before keep
 *  their status and note. */
export const compare = (tolerance?: number) =>
  invoke<Conflict[]>("compare", { toleranceDefault: tolerance ?? null });

/** The conflicts of the last comparison. */
export const conflicts = () => invoke<Conflict[]>("conflicts");

/** Sets a conflict's status and note, as an undo step. */
export const resolveConflict = (id: string, status: ConflictStatus, note: string) =>
  runCommand({ type: "resolve_conflict", id, status, note });

/** Writes the conflicts to an Excel workbook at `path` (sheets "Conflicts" and "Summary"). */
export const exportConflicts = (path: string) => invoke<void>("export_conflicts", { path });

/** Who answers the in-app assistant. */
export type Provider = "anthropic" | "openai_compatible";

export const EFFORTS = ["low", "medium", "high", "xhigh", "max"] as const;
export type Effort = (typeof EFFORTS)[number];

/** The app settings (`settings.json`). The API key is not here: it is in the OS keychain. */
export type Settings = {
  provider: Provider;
  /** Used exactly as typed. */
  model: string;
  /** The OpenAI-compatible service's address. */
  base_url: string | null;
  effort: Effort;
  mcp_enabled: boolean;
  mcp_port: number;
  /** 32 hex characters, made once. */
  mcp_token: string;
  language: "en" | "ar";
  theme: "system" | "light" | "dark";
  /** The user's command line aliases, alias to command, over the built-in ones. */
  aliases: Record<string, string>;
  /** The tool palette; changed only with `setToolPalette`. */
  tool_palette?: PaletteItem[];
  /** Minutes between autosaves of drawings with unsaved changes; 0 off (5 when left out). */
  autosave_minutes?: number;
};

/** What one autosave round did: the `.sv$` files written, and why others could not be. */
export type AutosaveRun = { written: string[]; failed: string[] };

/** Autosaves every open drawing with unsaved changes (`<name>.sv$` beside it). */
export const autosaveDrawings = () => invoke<AutosaveRun>("autosave_drawings");

/** Who has a drawing open (its `<file>.qlock`): the process, its computer, and when it took the
 *  lock (seconds since 1970). */
export type DrawingLock = { pid: number; host: string; started_at: number };

/** An autosave to offer back: newer than its drawing (or the drawing is gone), or left by a Qiyas
 *  that closed unexpectedly (`crashed`, then whatever its age). Times in seconds since 1970. */
export type Recoverable = {
  drawing: string;
  autosave: string;
  /** When the autosave was written. */
  saved_at: number;
  /** When the drawing was last written; null when it is gone. */
  drawing_at: number | null;
  /** A lock on this computer whose process has ended: Qiyas closed with the drawing open. */
  crashed: boolean;
  /** The drawing is open in another Qiyas (a running process's lock, or another computer's). */
  locked_elsewhere: DrawingLock | null;
};

/** The autosaves of `paths` to offer back: newer than their drawings, or left by a crash. */
export const recoverableDrawings = (paths: string[]) =>
  invoke<Recoverable[]>("recoverable_drawings", { paths });

/** Opens the autosave of the drawing `path` in a new tab, as a new unsaved drawing beside it. */
export const recoverAutosave = (path: string) => invoke<DrawingInfo>("recover_autosave", { path });

/** Deletes the autosave of the drawing `path`. */
export const discardAutosave = (path: string) => invoke<void>("discard_autosave", { path });

/** What the failsafe reader made of a file (`qiyas_core::ReadReport`). */
export type ReadReport = {
  records: number;
  decoded: number;
  skipped: number;
  entities: number;
  recovered_errors: number;
  dropped: { kind: string; count: number }[];
};

/** RECOVER: opens a damaged drawing in a new tab and says what its reader dropped. */
export const recoverDrawing = (path: string) =>
  invoke<{ info: DrawingInfo; report: ReadReport }>("recover_drawing", { path });

/** A sheet set (`.qss`): drawings by path relative to it, and their layouts (none: all). */
export type SheetSet = { name: string; drawings: { path: string; layouts: string[] }[] };

/** A sheet set drawing as listed: its path on this computer, and whether the file is missing. */
export type SheetSetEntry = { path: string; layouts: string[]; full: string; missing: boolean };

export type SheetSetView = { set: SheetSet; entries: SheetSetEntry[] };

export const sheetSetOpen = (path: string) => invoke<SheetSetView>("sheet_set_open", { path });

export const sheetSetSave = (path: string, set: SheetSet) =>
  invoke<SheetSetView>("sheet_set_save", { path, set });

/** The sheet set drawing for `drawing` (inside the set's folder) with all its layouts. */
export const sheetSetEntry = (qss: string, drawing: string) =>
  invoke<SheetSet["drawings"][number]>("sheet_set_entry", { qss, drawing });

/** A new sheet set of the open drawings in the folder of `qss` (not written yet); `skipped` names
 *  those outside it. */
export const sheetSetFromTabs = (qss: string, name: string) =>
  invoke<{ set: SheetSet; skipped: string[] }>("sheet_set_from_tabs", { qss, name });

export const getSettings = () => invoke<Settings>("get_settings");

/** One button of the tool palette: a block to insert or a command line to run. */
export type PaletteItem = { kind: "block" | "command"; label: string; value: string };

/** Keeps the tool palette in the settings file. */
export const setToolPalette = (items: PaletteItem[]) => invoke<void>("set_tool_palette", { items });

/** An attribute definition: its tag, the prompt asked on insert, its default value, and whether
 *  it is constant (never asked). */
export type AttDefInfo = { tag: string; prompt: string; default: string; constant: boolean };

/** A named block of the drawing, with its attribute definitions. */
export type BlockInfo = { name: string; attdefs: AttDefInfo[] };

export const blocks = () => invoke<BlockInfo[]>("blocks");

/** A PNG of the block, `px` wide. */
export const blockThumbnail = (name: string, px: number) =>
  invoke<ArrayBuffer>("block_thumbnail", { name, px });

/** The named blocks of another drawing, opened read-only, with their attribute definitions
 *  (the Design Center). */
export const drawingBlocks = (path: string) => invoke<BlockInfo[]>("drawing_blocks", { path });
/** Another drawing's layers, read-only (the layer translator's standard). */
export const drawingLayers = (path: string) => invoke<LayerInfo[]>("drawing_layers", { path });

/** WBLOCK to a .dwg the user chose in the Save dialog. */
export const writeBlock = (name: string, path: string) => invoke<void>("write_block", { name, path });

/** Rejects a port outside 1024–65535 or an unknown effort. */
export const setSettings = (settings: Settings) => invoke<void>("set_settings", { settings });

/** The user's command line aliases (alias to command). */
export const aliases = () => invoke<Record<string, string>>("aliases");

/** Stores the key in the OS keychain; it can't be read back from the window. */
export const setApiKey = (provider: Provider, key: string) =>
  invoke<void>("set_api_key", { provider, key });

export const hasApiKey = (provider: Provider) => invoke<boolean>("has_api_key", { provider });

export const deleteApiKey = (provider: Provider) => invoke<void>("delete_api_key", { provider });

/** The running app's folder, where the `qiyas-mcp` bridge sits. */
export const appDir = () => invoke<string>("app_dir");

/** The MCP server for outside AI apps: on, listening, or why it could not start. */
export type McpStatus = {
  enabled: boolean;
  listening: boolean;
  /** "127.0.0.1:<port>" while listening. */
  addr: string | null;
  error: string | null;
};

export const mcpStatus = () => invoke<McpStatus>("mcp_status");

/** What "Check for updates" found. */
export type UpdateCheck =
  | { status: "up_to_date" }
  | { status: "available"; version: string; notes: string | null };

/** Refusals of `checkUpdate` and `installUpdate` the About box says in its own words; other
 *  errors are the updater's own text. Not configured: this build's public key or feed address
 *  is still the placeholder. */
export const UPDATES_NOT_CONFIGURED = "updates_not_configured";
export const UNSAVED_DRAWINGS = "unsaved_drawings";
export const NO_UPDATE = "no_update";

/** Asks the update feed whether a newer Qiyas is out. */
export const checkUpdate = () => invoke<UpdateCheck>("check_update");

/** Downloads the update the last check found, checks its signature, installs it and restarts
 *  Qiyas; it only comes back, rejected, when that failed. */
export const installUpdate = () => invoke<void>("install_update");

/** Sent when the OS asked Qiyas to open files (a double click on a drawing, or a second launch
 *  with one); `takeOpenRequests` gives them. */
export const OPEN_REQUESTED = "open:requested";

/** The drawings the OS asked to open, oldest first; the queue is empty after. */
export const takeOpenRequests = () => invoke<string[]>("take_open_requests");

/** Sent when an outside AI app changed the take-off through the MCP server. */
export const TAKEOFF_CHANGED = "takeoff:changed";
/** Sent (with the tool result's `ui`) after an MCP tool showed another sheet, hid or showed layers,
 *  or asked for a zoom. */
export const SESSION_CHANGED = "session:changed";

/** What the window should do after a tool, from the tool's result. Bounds in drawing coordinates. */
export type AiUi = {
  /** "zoom", "show_sheet", "show_markup" or "redraw". */
  action: string;
  bounds?: [[number, number], [number, number]];
  handles?: number[];
  sheet?: string;
  /** After `redraw` from show_layers: the layers hidden now. */
  hidden?: string[];
  id?: string;
};

/** One event of the assistant's turn (`qiyas_ai::AgentEvent`), sent as `ai:event`. */
export type AgentEvent =
  | { type: "text"; text: string }
  | { type: "thinking"; text: string }
  | { type: "tool_call"; id: string; name: string; input: unknown; mutating: boolean }
  | {
      type: "tool_result";
      id: string;
      summary: string;
      handles: number[];
      ui: AiUi | null;
      is_error: boolean;
    }
  | { type: "refused"; category: string | null; text: string }
  | { type: "stopped"; reason: string }
  | { type: "done"; usage: Record<string, number> };

/** Sent as `ai:done` when a turn has ended. */
export type AiDone = { error: string | null };

export type AiStatus = {
  /** "anthropic", "openai_compatible", or "mock" in a debug build with QIYAS_AI_MOCK=1. */
  provider: string;
  model: string;
  has_key: boolean;
  busy: boolean;
  message_count: number;
};

/** Starts a turn and returns at once; events follow as `ai:event`, then `ai:done`. Rejects with
 *  "busy", "no_api_key" or "no_base_url". */
export const aiSend = (text: string) => invoke<void>("ai_send", { text });

export const aiCancel = () => invoke<void>("ai_cancel");

/** Starts a new conversation; `ai:cleared` follows. */
export const aiClear = () => invoke<void>("ai_clear");

export const aiStatus = () => invoke<AiStatus>("ai_status");

/** The agent log: where the file is and its newest part (empty before the first conversation). */
export type AiLog = { path: string; text: string };

export const aiLog = () => invoke<AiLog>("ai_log");

/** Undoes the assistant's newest change, only while it is still the newest change to the
 *  take-off; false when it is not (the user changed the take-off since). */
export const aiUndo = () => invoke<boolean>("ai_undo");

/** The assistant answers in this language; a change starts a new conversation. */
export const aiSetLanguage = (lang: "en" | "ar") => invoke<void>("ai_set_language", { lang });

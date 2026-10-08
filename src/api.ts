/** Thin wrappers over the Tauri commands of qiyas-desktop (crates/qiyas-desktop/src/commands.rs). */
import { invoke } from "@tauri-apps/api/core";

export type Sheet = { name: string; block: string; is_model: boolean };

export type LayerInfo = {
  name: string;
  color: [number, number, number];
  off: boolean;
  frozen: boolean;
  locked: boolean;
};

export type Units = "inches" | "feet" | "millimetres" | "centimetres" | "metres" | "unknown";

export type DrawingInfo = {
  path: string;
  sheets: Sheet[];
  layers: LayerInfo[];
  units: Units;
  problems: string[];
};

export type EntityInfo = {
  handle: number;
  kind: string;
  /** The layer it is drawn on (layer "0" inside a block takes its insert's). */
  layer: string;
  /** In drawing units, where its block references place it. */
  length: number | null;
  area: number | null;
  /** For a block reference, its block (a dynamic block by its own name). */
  block_name: string | null;
  text: string | null;
  /** The block the entity sits in, when it is inside one. */
  inside_block: string | null;
  /** The block references it sits inside, outermost first; empty on the sheet. */
  chain: number[];
  /** The block names along `chain`, outermost first. */
  block_path: string[];
};

/** What is under the cursor (`qiyas_core::Picked`): the entity, the block references it sits inside
 *  (outermost first, empty on the sheet) and the top-level entity on the sheet. */
export type Picked = { top: number; entity: number; chain: number[]; xref: string | null };

/** Filter for the session's `find_entities` query (no IPC command yet). Names match case-insensitively. */
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
  /** Rows per page, at most 500; 100 when left out. */
  limit?: number;
};

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
};

/** One page of results; `total` counts every match. */
export type Page<T> = { rows: T[]; total: number; next_offset: number | null };

export type TextRow = {
  handle: number;
  text: string;
  at: [number, number];
  height: number;
  inside_block: string | null;
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

export const openDrawing = (path: string) => invoke<DrawingInfo>("open_drawing", { path });

export const drawingInfo = () => invoke<DrawingInfo>("drawing_info");

export const setSheet = (block: string) => invoke<void>("set_sheet", { block });

export const setLayerHidden = (name: string, hidden: boolean) =>
  invoke<void>("set_layer_hidden", { name, hidden });

/** Names the current sheet's batch: the same number means the same batch. */
export const batchVersion = () => invoke<number>("batch_version");

/** What `render_batch` sends instead of a batch that is still `ifVersion`: "QINM" and four zeros. */
export function isNotModified(bytes: ArrayBuffer): boolean {
  const marker = [0x51, 0x49, 0x4e, 0x4d, 0, 0, 0, 0];
  const got = new Uint8Array(bytes);
  return got.length === marker.length && marker.every((b, i) => got[i] === b);
}

/** Raw bytes of the current sheet's batch; decode with `parseBatch`. `null` when the batch is
 *  still `ifVersion` (from `batchVersion`), so the caller keeps the one it has. */
export async function renderBatch(ifVersion?: number): Promise<ArrayBuffer | null> {
  const bytes = await invoke<ArrayBuffer>("render_batch", { ifVersion });
  return isNotModified(bytes) ? null : bytes;
}

/** The entity nearest to the drawing point (x, y) within `tol` drawing units, down to the entity
 *  inside any blocks. */
export const pick = (x: number, y: number, tol: number) =>
  invoke<Picked | null>("pick", { x, y, tol });

/** One entity; give `chain` for an entity inside blocks. */
export const entityInfo = (handle: number, chain: number[] = []) =>
  invoke<EntityInfo>("entity_info", { handle, chain });

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
  | { type: "resolve_conflict"; id: string; status: ConflictStatus; note: string };

export type Outcome =
  | { type: "measured"; measurement: Measurement }
  | { type: "removed"; id: string }
  | { type: "changed" }
  | { type: "nothing" };

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
};

export const getSettings = () => invoke<Settings>("get_settings");

/** Rejects a port outside 1024–65535 or an unknown effort. */
export const setSettings = (settings: Settings) => invoke<void>("set_settings", { settings });

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

/** Undoes the assistant's newest change, only while it is still the newest change to the
 *  take-off; false when it is not (the user changed the take-off since). */
export const aiUndo = () => invoke<boolean>("ai_undo");

/** The assistant answers in this language; a change starts a new conversation. */
export const aiSetLanguage = (lang: "en" | "ar") => invoke<void>("ai_set_language", { lang });

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
  layer: string;
  length: number | null;
  area: number | null;
  block_name: string | null;
  text: string | null;
};

export const openDrawing = (path: string) => invoke<DrawingInfo>("open_drawing", { path });

export const drawingInfo = () => invoke<DrawingInfo>("drawing_info");

export const setSheet = (block: string) => invoke<void>("set_sheet", { block });

export const setLayerHidden = (name: string, hidden: boolean) =>
  invoke<void>("set_layer_hidden", { name, hidden });

/** Raw bytes of the current sheet's batch; decode with `parseBatch`. */
export const renderBatch = () => invoke<ArrayBuffer>("render_batch");

/** Handle of the entity nearest to the drawing point (x, y) within `tol` drawing units. */
export const pick = (x: number, y: number, tol: number) =>
  invoke<number | null>("pick", { x, y, tol });

export const entityInfo = (handle: number) => invoke<EntityInfo>("entity_info", { handle });

export type Kind = "length" | "area" | "count";

/** One take-off line. `value` is in drawing units (units² for areas); `shown` is in `unit`. */
export type Measurement = {
  id: string;
  kind: Kind;
  value: number;
  unit: "m" | "m2" | "pcs";
  shown: number;
  description: string;
  layer: string;
  sheet: string;
  handles: number[];
  points: [number, number][];
  block_name: string | null;
  self_intersecting: boolean;
  source: "user" | "ai";
  note: string;
  created: string;
};

export type Total = { kind: Kind; unit: Measurement["unit"]; count: number; sum: number };

/** The command bus (`qiyas_core::Command`). Points are in drawing coordinates. */
export type Command =
  | { type: "measure_distance"; points: [number, number][] }
  | { type: "measure_area"; points: [number, number][] }
  | { type: "measure_entity"; handle: number }
  | { type: "count_block"; block_name: string; layer: string | null }
  | { type: "remove_measurement"; id: string }
  | { type: "set_description"; id: string; text: string }
  | { type: "set_layer_hidden"; name: string; hidden: boolean }
  | { type: "set_sheet"; block: string };

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

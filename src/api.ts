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

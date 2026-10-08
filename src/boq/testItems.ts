/** Small BOQ items, take-off rows and conflicts for the BOQ tests. */
import type { BoqItem, Conflict, ConflictKind, Measurement } from "../api";

export const boqItem = (
  code: string,
  description: string,
  unit: string,
  quantity: number,
  more: Partial<BoqItem> = {},
): BoqItem => ({
  code,
  description,
  unit,
  quantity,
  rate: null,
  section: "",
  tolerance: null,
  row: 1,
  ...more,
});

export const row = (
  id: string,
  shown: number,
  unit: string,
  boq_code: string | null,
  more: Partial<Measurement> = {},
): Measurement => ({
  id,
  kind: unit === "m2" ? "area" : unit === "pcs" ? "count" : "length",
  value: shown,
  unit,
  shown,
  description: id,
  layer: "A-WALL",
  sheet: "*Model_Space",
  handles: [],
  points: [],
  block_name: null,
  self_intersecting: false,
  source: "user",
  note: "",
  created: "2026-10-08T00:00:00Z",
  height: null,
  boq_code,
  chain: [],
  block_path: [],
  ...more,
});

export const conflict = (
  id: string,
  kind: ConflictKind,
  reason: string,
  more: Partial<Conflict> = {},
): Conflict => ({
  id,
  kind,
  boq_code: null,
  measurements: [],
  schedule: null,
  sheets: [],
  expected: null,
  found: null,
  unit: "",
  reason,
  reason_ar: `${reason} (ar)`,
  status: "open",
  note: "",
  created: "2026-10-08T00:00:00Z",
  hash_key: id,
  ...more,
});

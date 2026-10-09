/** The edit tools (modify, part two): trim, extend, break, join, lengthen, reverse, offset,
 *  fillet, chamfer, blend, polyline edit, overkill and flatten. Pure state machines with their
 *  prompts (i18n keys), as the modify tools are; `useTool` feeds them clicks on entities (with
 *  where the click was: the piece, end or side meant), boxes, points and typed input, and runs
 *  each finished `command`.
 *
 *  Trim and extend take their cutting edges first (Enter with none: everything drawn), then a
 *  click per piece until Esc; Shift+click does the other one. Join, reverse, overkill and flatten
 *  take a selection. The rest click entities one after another and stay on for more. */
import type { Command, EditSpec, EntityTarget, PEditOp } from "../api";
import type { Point } from "../viewport/camera";

export const EDIT_TOOLS = [
  "trim",
  "extend",
  "break",
  "join",
  "lengthen",
  "reverse",
  "offset",
  "fillet",
  "chamfer",
  "blend",
  "pedit",
  "overkill",
  "flatten",
] as const;

export type EditKind = (typeof EDIT_TOOLS)[number];

export const isEditTool = (tool: string): tool is EditKind =>
  (EDIT_TOOLS as readonly string[]).includes(tool);

/** The tools that work on a selection, made before or picked now, then Enter. */
const ON_SELECTION = new Set<EditKind>(["join", "reverse", "overkill", "flatten"]);

/** Overkill's tolerance in drawing units, AutoCAD's default. */
export const OVERKILL_TOL = 1e-6;

/** What a typed number or option answers now. */
type Asking =
  | "offset"
  | "radius"
  | "d1"
  | "d2"
  | "lengthen"
  | "width"
  /** Pedit: turn a line or an arc into a polyline? Y or N. */
  | "convert"
  /** Pedit: which change. */
  | "op";

type Picked = { handle: number; at: Point };

export type EditState = {
  kind: EditKind;
  /** "edges": trim's and extend's cutting edges, until Enter; "select": the entities join,
   *  reverse, overkill and flatten work on, until Enter; "pick": a click on an entity; "point":
   *  a point (offset's side, the second break point, a vertex). */
  phase: "edges" | "select" | "pick" | "point";
  /** The edges or the selection: top-level handles. */
  handles: number[];
  /** The entity clicked first, and where: offset's, break's, the first of a fillet, chamfer or
   *  blend, the polyline pedit changes. */
  first: Picked | null;
  asking: Asking | null;
  /** Pedit: the point is for a new vertex ("add") or the vertex to remove ("remove"). */
  vertex: "add" | "remove" | null;
  /** Offset's distance, once typed. */
  distance: number | null;
  radius: number;
  d1: number;
  d2: number;
  /** Lengthen: by a change, to a total, or to a percentage; and the number, once typed. */
  lengthen: { by: "delta" | "total" | "percent"; value: number | null };
  /** Set by the input that finished a change; the hook runs it. */
  command: Command | null;
  hint: string;
  hintArgs: Record<string, string> | null;
};

export type EditInput =
  /** A click on an entity (its top-level handle and kind) at `at`; `shift` swaps trim and
   *  extend. */
  | { type: "pick"; handle: number; kind?: string | null; at: Point; shift?: boolean }
  /** Entities a box picked (top-level handles). */
  | { type: "picked"; handles: number[] }
  | { type: "point"; p: Point }
  | { type: "number"; value: number }
  | { type: "option"; option: string }
  /** Enter, or a double click. */
  | { type: "finish" };

const target = (handle: number): EntityTarget => ({ handle, chain: [] });
const targets = (handles: number[]) => handles.map(target);
const edit = (spec: EditSpec): Command => ({ type: "edit", ...spec });
const fmt = (v: number) => String(Number(v.toPrecision(6)));

/** The tool waiting for a click on an entity, with its prompt. */
function picking(s: EditState): EditState {
  const p: EditState = { ...s, phase: "pick", first: null, asking: null, vertex: null, hintArgs: null };
  switch (s.kind) {
    case "trim":
    case "extend":
      return { ...p, hint: `edit.prompt.${s.kind}` };
    case "break":
      return { ...p, hint: "edit.prompt.breakPick" };
    case "lengthen":
      return { ...p, hint: "edit.prompt.lengthenPick", hintArgs: { value: fmt(s.lengthen.value ?? 0) } };
    case "offset":
      return { ...p, hint: "edit.prompt.offsetPick", hintArgs: { d: fmt(s.distance ?? 0) } };
    case "fillet":
      return { ...p, hint: "edit.prompt.filletFirst", hintArgs: { r: fmt(s.radius) } };
    case "chamfer":
      return { ...p, hint: "edit.prompt.chamferFirst", hintArgs: { d1: fmt(s.d1), d2: fmt(s.d2) } };
    case "blend":
      return { ...p, hint: "edit.prompt.blendFirst" };
    case "pedit":
      return { ...p, hint: "edit.prompt.peditPick" };
    default:
      return selecting(s);
  }
}

function selecting(s: EditState): EditState {
  return { ...s, phase: "select", hint: "edit.prompt.select", hintArgs: { n: String(s.handles.length) } };
}

const lengthenHint = (s: EditState): EditState => ({
  ...s,
  asking: "lengthen",
  hint: `edit.prompt.lengthen.${s.lengthen.by}`,
  hintArgs: null,
});

/** A fresh tool; trim and extend take `selection` (made before) as their edges, join, reverse,
 *  overkill and flatten run on it at once. */
export function initialEditState(kind: EditKind, selection: number[] = []): EditState {
  const s: EditState = {
    kind,
    phase: "pick",
    handles: [],
    first: null,
    asking: null,
    vertex: null,
    distance: null,
    radius: 0,
    d1: 0,
    d2: 0,
    lengthen: { by: "delta", value: null },
    command: null,
    hint: "",
    hintArgs: null,
  };
  const handles = [...new Set(selection)];
  if (kind === "trim" || kind === "extend") {
    if (handles.length > 0) return picking({ ...s, handles });
    return { ...s, phase: "edges", hint: "edit.prompt.edges", hintArgs: { n: "0" } };
  }
  if (ON_SELECTION.has(kind)) {
    const fresh = selecting(s);
    return handles.length > 0 ? run(fresh, handles) : fresh;
  }
  if (kind === "offset") return { ...s, asking: "offset", hint: "edit.prompt.offsetDistance" };
  if (kind === "lengthen") return lengthenHint(s);
  return picking(s);
}

/** Join, reverse, overkill or flatten on `handles`; then a new selection. */
function run(s: EditState, handles: number[]): EditState {
  const t = targets(handles);
  const spec: EditSpec =
    s.kind === "join"
      ? { kind: "join", targets: t }
      : s.kind === "reverse"
        ? { kind: "reverse", targets: t }
        : s.kind === "overkill"
          ? { kind: "overkill", targets: t, tol: OVERKILL_TOL }
          : { kind: "flatten", targets: t };
  return { ...selecting({ ...s, handles: [] }), command: edit(spec) };
}

/** The option words the command line takes now. */
export function editOptions(s: EditState): string[] {
  switch (s.asking) {
    case "lengthen":
      return ["DE", "T", "P"];
    case "convert":
      return ["Y", "N"];
    case "op":
      return ["C", "O", "W", "F", "D", "I", "R"];
    default:
      break;
  }
  if (s.phase !== "pick" || s.first) return [];
  if (s.kind === "fillet") return ["R"];
  if (s.kind === "chamfer") return ["D"];
  if (s.kind === "lengthen") return ["DE", "T", "P"];
  return [];
}

/** The tool takes a typed number now: "distance" (above 0: an offset, a radius above or at 0…),
 *  "angle" (any sign: a lengthen change) or false. */
export function takesEditNumber(s: EditState): "distance" | "angle" | false {
  switch (s.asking) {
    case "offset":
    case "radius":
    case "d1":
    case "d2":
    case "width":
      return "distance";
    case "lengthen":
      return s.lengthen.by === "delta" ? "angle" : "distance";
    default:
      return false;
  }
}

function typedNumber(s: EditState, v: number): EditState {
  const takes = takesEditNumber(s);
  if (!takes || !Number.isFinite(v)) return s;
  switch (s.asking) {
    case "offset":
      return v > 0 ? picking({ ...s, distance: v }) : s;
    case "radius":
      return v >= 0 ? picking({ ...s, radius: v }) : s;
    case "d1":
      return v >= 0 ? { ...s, d1: v, asking: "d2", hint: "edit.prompt.d2", hintArgs: null } : s;
    case "d2":
      return v >= 0 ? picking({ ...s, d2: v }) : s;
    case "lengthen":
      if (s.lengthen.by !== "delta" && !(v > 0)) return s;
      return picking({ ...s, lengthen: { ...s.lengthen, value: v } });
    case "width": {
      if (!(v >= 0) || !s.first) return s;
      return peditOp(s, { width: v });
    }
    default:
      return s;
  }
}

/** Runs a pedit change on the polyline picked and stays on it for more. */
function peditOp(s: EditState, op: PEditOp): EditState {
  const first = s.first!;
  return {
    ...s,
    phase: "pick",
    asking: "op",
    vertex: null,
    hint: "edit.prompt.peditOp",
    hintArgs: null,
    command: edit({ kind: "pedit", target: target(first.handle), op }),
  };
}

function option(s: EditState, word: string): EditState {
  if (!editOptions(s).includes(word)) return s;
  switch (s.asking ?? s.kind) {
    case "lengthen": {
      const by = word === "T" ? "total" : word === "P" ? "percent" : "delta";
      return lengthenHint({ ...s, lengthen: { by, value: null } });
    }
    case "fillet":
      return { ...s, asking: "radius", hint: "edit.prompt.radius", hintArgs: null };
    case "chamfer":
      return { ...s, asking: "d1", hint: "edit.prompt.d1", hintArgs: null };
    case "convert": {
      if (word === "N") return picking(s);
      const first = s.first!;
      return {
        ...s,
        asking: "op",
        hint: "edit.prompt.peditOp",
        hintArgs: null,
        command: edit({ kind: "pedit", target: target(first.handle), op: "convert_to_polyline" }),
      };
    }
    case "op":
      switch (word) {
        case "C":
          return peditOp(s, "close");
        case "O":
          return peditOp(s, "open");
        case "F":
          return peditOp(s, "fit_curve");
        case "D":
          return peditOp(s, "decurve");
        case "W":
          return { ...s, asking: "width", hint: "edit.prompt.width" };
        case "I":
          return { ...s, phase: "point", asking: null, vertex: "add", hint: "edit.prompt.addVertex" };
        case "R":
          return { ...s, phase: "point", asking: null, vertex: "remove", hint: "edit.prompt.removeVertex" };
        default:
          return s;
      }
    default:
      return s;
  }
}

/** Kinds pedit takes as they are, or turns into a polyline first. */
const POLYLINE = new Set(["LwPolyline"]);
const BECOMES_POLYLINE = new Set(["Line", "Arc"]);

function pick(s: EditState, handle: number, kind: string | null | undefined, at: Point, shift: boolean): EditState {
  if (s.phase === "edges" || s.phase === "select") {
    return addHandles(s, [handle]);
  }
  if (s.phase !== "pick" || (s.asking && s.asking !== "op")) return s;
  const here = { handle, at };
  const first = s.first;
  switch (s.kind) {
    case "trim":
    case "extend": {
      const trim = (s.kind === "trim") !== shift;
      const edges = targets(s.handles);
      const spec: EditSpec = trim
        ? { kind: "trim", cutters: edges, target: target(handle), at_point: at }
        : { kind: "extend", boundaries: edges, target: target(handle), at_end: at };
      return { ...s, command: edit(spec) };
    }
    case "lengthen": {
      const value = s.lengthen.value ?? 0;
      const by =
        s.lengthen.by === "total" ? { total: value } : s.lengthen.by === "percent" ? { percent: value } : { delta: value };
      return { ...s, command: edit({ kind: "lengthen", target: target(handle), at_end: at, by }) };
    }
    case "break":
      return { ...s, first: here, phase: "point", hint: "edit.prompt.breakSecond", hintArgs: null };
    case "offset":
      return { ...s, first: here, phase: "point", hint: "edit.prompt.offsetSide", hintArgs: null };
    case "fillet":
    case "chamfer":
    case "blend": {
      if (!first) return { ...s, first: here, hint: `edit.prompt.${s.kind}Second`, hintArgs: null };
      const [a, b, at_a, at_b] = [target(first.handle), target(handle), first.at, at];
      const spec: EditSpec =
        s.kind === "fillet"
          ? { kind: "fillet", a, b, at_a, at_b, radius: s.radius }
          : s.kind === "chamfer"
            ? { kind: "chamfer", a, b, at_a, at_b, d1: s.d1, d2: s.d2 }
            : { kind: "blend", a, b, at_a, at_b };
      return { ...picking(s), command: edit(spec) };
    }
    case "pedit": {
      if (kind && POLYLINE.has(kind)) {
        return { ...s, first: here, asking: "op", hint: "edit.prompt.peditOp", hintArgs: null };
      }
      if (kind && BECOMES_POLYLINE.has(kind)) {
        return { ...s, first: here, asking: "convert", hint: "edit.prompt.peditConvert", hintArgs: null };
      }
      return { ...picking(s), hint: "edit.prompt.notPolyline" };
    }
    default:
      return s;
  }
}

function addHandles(s: EditState, handles: number[]): EditState {
  const all = [...new Set([...s.handles, ...handles])];
  if (s.phase === "edges") return { ...s, handles: all, hintArgs: { n: String(all.length) } };
  if (s.phase === "select") return selecting({ ...s, handles: all });
  return s;
}

function point(s: EditState, p: Point): EditState {
  if (s.phase !== "point" || !s.first) return s;
  const first = s.first;
  switch (s.kind) {
    case "break":
      return { ...picking(s), command: edit({ kind: "break", target: target(first.handle), p1: first.at, p2: p }) };
    case "offset":
      return {
        ...picking(s),
        command: edit({ kind: "offset", target: target(first.handle), distance: s.distance ?? 0, side_point: p }),
      };
    case "pedit":
      return peditOp(s, s.vertex === "remove" ? { delete_vertex: p } : { add_vertex: p });
    default:
      return s;
  }
}

function finishStep(s: EditState): EditState {
  if (s.phase === "edges") return picking(s);
  if (s.phase === "select") return s.handles.length > 0 ? run(s, s.handles) : s;
  // Break at a single point.
  if (s.kind === "break" && s.phase === "point" && s.first) return point(s, s.first.at);
  if (s.asking === "convert") return option(s, "Y");
  if (s.asking === "lengthen" && s.lengthen.value !== null) return picking(s);
  return s;
}

export function editReducer(state: EditState, input: EditInput): EditState {
  // A finished change is pending for one state only: the hook sees it once.
  const s = { ...state, command: null };
  switch (input.type) {
    case "pick":
      return pick(s, input.handle, input.kind, input.at, !!input.shift);
    case "picked":
      return s.phase === "edges" || s.phase === "select" ? addHandles(s, input.handles) : s;
    case "point":
      return point(s, input.p);
    case "number":
      return typedNumber(s, input.value);
    case "option":
      return option(s, input.option);
    case "finish":
      return finishStep(s);
  }
}

/** Clicks pick an entity now (else they place a snapped point). */
export const editPicks = (s: EditState) => s.phase !== "point";

/** A left drag draws a selection box now. */
export const editBoxes = (s: EditState) => s.phase === "edges" || s.phase === "select";

/** The entities to draw highlighted: the edges or selection, and the first entity picked. */
export const editHighlight = (s: EditState): number[] =>
  s.first && !s.handles.includes(s.first.handle) ? [...s.handles, s.first.handle] : s.handles;

/** The annotate tools: dimensions (linear, aligned, angular, radius, diameter, arc length,
 *  ordinate, baseline, continue, quick), dimension edits (text, flip, move text; break, space,
 *  jog), leaders, multileaders and tolerances. Pure state machines with their prompts (i18n keys),
 *  as the edit tools are; `useTool` feeds them clicks on entities, points, boxes and typed input,
 *  and runs each finished `command`. A leader's, a comment cloud's or a tolerance's text, and a
 *  dimension's new text, are asked for by the app (`request`). Every tool stays on for the next
 *  one. A comment cloud (COMMENT) goes round the points clicked, closed by Enter. */
import type { Command, DimEditOp, DimSpec, EntityTarget } from "../api";
import type { Point } from "../viewport/camera";

export const DIM_TOOLS = [
  "dimLinear",
  "dimAligned",
  "dimAngular",
  "dimRadius",
  "dimDiameter",
  "dimArc",
  "dimOrdinate",
  "dimBaseline",
  "dimContinue",
  "qdim",
  "dimEdit",
  "dimBreak",
  "dimSpace",
  "dimJogLine",
  "leader",
  "mleader",
  "tolerance",
  "commentCloud",
] as const;

export type DimKind = (typeof DIM_TOOLS)[number];

export const isDimTool = (tool: string): tool is DimKind => (DIM_TOOLS as readonly string[]).includes(tool);

/** What the app asks the user to type: a leader's text, a tolerance's frame, a dimension's text. */
export type DimRequest =
  | { kind: "leader" | "mleader"; points: Point[] }
  | { kind: "comment"; points: Point[] }
  | { kind: "tolerance"; at: Point }
  | { kind: "override"; handle: number };

/** How many points each point tool takes, in order (their prompts are `dim.prompt.<kind><i>`). */
const POINTS: Partial<Record<DimKind, number>> = {
  dimLinear: 3,
  dimAligned: 3,
  dimAngular: 4,
  dimOrdinate: 2,
  tolerance: 1,
};

/** The tools that pick an entity first (and which kinds they take). */
const PICKS: Partial<Record<DimKind, string[]>> = {
  dimRadius: ["Circle", "Arc"],
  dimDiameter: ["Circle", "Arc"],
  dimArc: ["Arc"],
  dimBaseline: ["Dimension"],
  dimContinue: ["Dimension"],
  dimEdit: ["Dimension"],
  dimBreak: ["Dimension"],
  dimSpace: ["Dimension"],
  dimJogLine: ["Dimension"],
};

export type DimState = {
  kind: DimKind;
  /** "point": a point; "pick": a click on an entity; "select": entities until Enter. */
  phase: "point" | "pick" | "select";
  points: Point[];
  /** The entity picked first: the circle, the base dimension, the dimension to edit. */
  first: number | null;
  /** QDIM's entities, DIMSPACE's dimensions. */
  handles: number[];
  /** DIMLINEAR: H or V typed (degrees), else null (by where the line point is). */
  rotation: number | null;
  /** DIMEDIT: which change, once picked; DIMSPACE: the spacing, typed or Enter. */
  asking: "op" | "spacing" | null;
  /** DIMEDIT M: the next point is the text's. */
  moving: boolean;
  /** Set by the input that finished a change; the hook runs it. */
  command: Command | null;
  /** Set by the input that needs typed text; the hook hands it to the app. */
  request: DimRequest | null;
  hint: string;
  hintArgs: Record<string, string> | null;
};

export type DimInput =
  | { type: "pick"; handle: number; kind?: string | null; at: Point }
  | { type: "picked"; handles: number[] }
  | { type: "point"; p: Point }
  | { type: "number"; value: number }
  | { type: "option"; option: string }
  | { type: "finish" };

const target = (handle: number): EntityTarget => ({ handle, chain: [] });
const dimension = (spec: DimSpec): Command => ({ type: "dimension", ...spec });

/** The tool waiting for its first input again. */
function restart(s: DimState): DimState {
  const base = { ...s, points: [], first: null, handles: [], asking: null, moving: false, hintArgs: null };
  if (s.kind === "qdim") return { ...base, phase: "select", hint: "dim.prompt.qdim", hintArgs: { n: "0" } };
  if (PICKS[s.kind]) return { ...base, phase: "pick", hint: `dim.prompt.${s.kind}Pick` };
  if (s.kind === "leader" || s.kind === "mleader") return { ...base, phase: "point", hint: "dim.prompt.leader0" };
  if (s.kind === "commentCloud") return { ...base, phase: "point", hint: "dim.prompt.comment0" };
  return { ...base, phase: "point", hint: `dim.prompt.${s.kind}0` };
}

/** A fresh tool; QDIM starts on `selection` (made before). */
export function initialDimState(kind: DimKind, selection: number[] = []): DimState {
  const s = restart({
    kind,
    phase: "point",
    points: [],
    first: null,
    handles: [],
    rotation: null,
    asking: null,
    moving: false,
    command: null,
    request: null,
    hint: "",
    hintArgs: null,
  });
  const handles = [...new Set(selection)];
  return kind === "qdim" && handles.length > 0 ? { ...s, handles, hintArgs: { n: String(handles.length) } } : s;
}

/** The option words the command line takes now. */
export function dimOptions(s: DimState): string[] {
  if (s.asking === "op") return ["T", "F", "M"];
  if (s.kind === "dimLinear" && s.phase === "point") return ["H", "V"];
  return [];
}

/** DIMSPACE takes a typed spacing (a distance) now. */
export const takesDimNumber = (s: DimState): "distance" | false => (s.asking === "spacing" ? "distance" : false);

/** The signed distance of `p` to the left of the line from `a` to `b`. */
function leftOf(a: Point, b: Point, p: Point): number {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const len = Math.hypot(dx, dy);
  return len > 0 ? (dx * (p[1] - a[1]) - dy * (p[0] - a[0])) / len : 0;
}

/** The command the points of a point tool make, once it has them all. */
function pointsDone(s: DimState, pts: Point[]): DimState {
  const next = restart(s);
  switch (s.kind) {
    case "dimLinear":
      return {
        ...next,
        command: dimension({ kind: "linear", p1: pts[0], p2: pts[1], line_point: pts[2], rotation: s.rotation }),
      };
    case "dimAligned":
      return {
        ...next,
        command: dimension({ kind: "aligned", p1: pts[0], p2: pts[1], offset: leftOf(pts[0], pts[1], pts[2]) }),
      };
    case "dimAngular":
      return { ...next, command: dimension({ kind: "angular", vertex: pts[0], p1: pts[1], p2: pts[2], arc_point: pts[3] }) };
    case "dimOrdinate":
      return { ...next, command: dimension({ kind: "ordinate", feature: pts[0], leader_end: pts[1] }) };
    case "tolerance":
      return { ...next, request: { kind: "tolerance", at: pts[0] } };
    default:
      return next;
  }
}

function point(s: DimState, p: Point): DimState {
  if (s.phase !== "point") return s;
  const first = s.first;
  if (first !== null) {
    const t = target(first);
    switch (s.kind) {
      case "dimRadius":
        return { ...restart(s), command: dimension({ kind: "radius", target: t, point: p }) };
      case "dimDiameter":
        return { ...restart(s), command: dimension({ kind: "diameter", target: t, point: p }) };
      case "dimArc":
        return { ...restart(s), command: dimension({ kind: "arc_length", target: t, point: p }) };
      case "dimJogLine":
        return { ...restart(s), command: { type: "dim_jog_line", target: t, at: p } };
      case "dimEdit":
        if (!s.moving) return s;
        return { ...restart(s), command: { type: "dim_edit", target: t, op: { move_text: p } } };
      case "dimBaseline":
      case "dimContinue": {
        const points = [...s.points, p];
        return { ...s, points, hint: "dim.prompt.chainNext", hintArgs: { n: String(points.length) } };
      }
      default:
        return s;
    }
  }
  const points = [...s.points, p];
  if (s.kind === "leader" || s.kind === "mleader") {
    return { ...s, points, hint: points.length >= 2 ? "dim.prompt.leaderMore" : "dim.prompt.leader1" };
  }
  if (s.kind === "commentCloud") {
    return { ...s, points, hint: points.length >= 3 ? "dim.prompt.commentMore" : "dim.prompt.comment1" };
  }
  if (s.kind === "qdim") return { ...restart(s), command: dimension({ kind: "quick", targets: s.handles.map(target), line_point: p }) };
  const need = POINTS[s.kind] ?? 0;
  if (points.length >= need) return pointsDone(s, points);
  return { ...s, points, hint: `dim.prompt.${s.kind}${points.length}` };
}

function pick(s: DimState, handle: number, kind: string | null | undefined): DimState {
  if (s.phase === "select") return addHandles(s, [handle]);
  if (s.phase !== "pick") return s;
  const takes = PICKS[s.kind] ?? [];
  if (s.first === null) {
    if (!kind || !takes.includes(kind)) return { ...s, hint: `dim.prompt.${s.kind}Wrong` };
    switch (s.kind) {
      case "dimEdit":
        return { ...s, first: handle, asking: "op", hint: "dim.prompt.dimEditOp" };
      case "dimBreak":
        return { ...s, first: handle, hint: "dim.prompt.dimBreakCutter" };
      case "dimSpace":
        return { ...s, first: handle, phase: "select", hint: "dim.prompt.dimSpaceSelect", hintArgs: { n: "0" } };
      case "dimBaseline":
      case "dimContinue":
        return { ...s, first: handle, phase: "point", hint: "dim.prompt.chainNext", hintArgs: { n: "0" } };
      default:
        return { ...s, first: handle, phase: "point", hint: `dim.prompt.${s.kind}Point` };
    }
  }
  if (s.kind === "dimBreak") {
    return { ...restart(s), command: { type: "dim_break", target: target(s.first), cutter: target(handle) } };
  }
  return s;
}

function addHandles(s: DimState, handles: number[]): DimState {
  if (s.phase !== "select") return s;
  const all = [...new Set([...s.handles, ...handles])].filter((h) => h !== s.first);
  return { ...s, handles: all, hintArgs: { n: String(all.length) } };
}

function option(s: DimState, word: string): DimState {
  if (!dimOptions(s).includes(word)) return s;
  if (s.asking === "op" && s.first !== null) {
    const t = target(s.first);
    if (word === "F") return { ...restart(s), command: { type: "dim_edit", target: t, op: "flip" as DimEditOp } };
    if (word === "T") return { ...restart(s), request: { kind: "override", handle: s.first } };
    return { ...s, asking: null, phase: "point", moving: true, hint: "dim.prompt.dimEditMove" };
  }
  // DIMLINEAR: H horizontal, V vertical.
  return { ...s, rotation: word === "V" ? 90 : 0 };
}

function finishStep(s: DimState): DimState {
  if (s.kind === "qdim" && s.phase === "select") {
    return s.handles.length > 0 ? { ...s, phase: "point", hint: "dim.prompt.qdimLine", hintArgs: null } : s;
  }
  if (s.kind === "dimSpace") {
    if (s.phase === "select" && s.handles.length > 0) {
      return { ...s, phase: "pick", asking: "spacing", hint: "dim.prompt.dimSpaceGap", hintArgs: null };
    }
    if (s.asking === "spacing") return space(s, null);
    return s;
  }
  if ((s.kind === "dimBaseline" || s.kind === "dimContinue") && s.first !== null) {
    if (s.points.length === 0) return restart(s);
    const kind = s.kind === "dimBaseline" ? "baseline" : "continue";
    return { ...restart(s), command: dimension({ kind, from: target(s.first), points: s.points }) };
  }
  if ((s.kind === "leader" || s.kind === "mleader") && s.points.length >= 2) {
    return { ...restart(s), request: { kind: s.kind, points: s.points } };
  }
  if (s.kind === "commentCloud" && s.points.length >= 3) {
    return { ...restart(s), request: { kind: "comment", points: s.points } };
  }
  return s;
}

function space(s: DimState, spacing: number | null): DimState {
  if (s.first === null) return s;
  return {
    ...restart(s),
    command: { type: "dim_space", base: target(s.first), targets: s.handles.map(target), spacing },
  };
}

export function dimReducer(state: DimState, input: DimInput): DimState {
  // A finished change or a request is pending for one state only: the hook sees it once.
  const s = { ...state, command: null, request: null };
  switch (input.type) {
    case "pick":
      return pick(s, input.handle, input.kind);
    case "picked":
      return addHandles(s, input.handles);
    case "point":
      return point(s, input.p);
    case "number":
      return s.asking === "spacing" && input.value > 0 && Number.isFinite(input.value) ? space(s, input.value) : s;
    case "option":
      return option(s, input.option);
    case "finish":
      return finishStep(s);
  }
}

/** Clicks pick an entity now (else they place a snapped point). */
export const dimPicks = (s: DimState) => s.phase !== "point";

/** A left drag draws a selection box now. */
export const dimBoxes = (s: DimState) => s.phase === "select";

/** The entities to draw highlighted: the one picked first and the selection. */
export const dimHighlight = (s: DimState): number[] => (s.first !== null ? [s.first, ...s.handles] : s.handles);

/** The last point placed, for tracking and `@`. */
export const lastDimPoint = (s: DimState): Point | null => (s.points.length > 0 ? s.points[s.points.length - 1] : null);

/** What the points so far draw with the cursor, as segments `x0 y0 x1 y1`. */
export function dimPreview(s: DimState, cursor: Point | null): Float64Array {
  if (s.phase !== "point" || s.first !== null) return new Float64Array();
  const pts = cursor ? [...s.points, cursor] : s.points;
  const out: number[] = [];
  for (let i = 1; i < pts.length; i++) out.push(pts[i - 1][0], pts[i - 1][1], pts[i][0], pts[i][1]);
  // A comment cloud's ring closes back to its first point.
  if (s.kind === "commentCloud" && pts.length >= 3) {
    const [a, b] = [pts[pts.length - 1], pts[0]];
    out.push(a[0], a[1], b[0], b[1]);
  }
  return Float64Array.from(out);
}

/** The frame AutoCAD writes for a tolerance: a symbol (a GDT letter, or none), the tolerance and
 *  up to three datums, fields split by `%%v`. */
export function toleranceFrame(symbol: string, value: string, datums: string[]): string {
  const sym = symbol ? `{\\Fgdt;${symbol}}` : "";
  const d = [...datums, "", "", ""].slice(0, 3);
  return [sym, value.trim(), "", ...d.map((x) => x.trim())].join("%%v");
}

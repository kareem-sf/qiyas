/** The modify tools: select the entities, then the points, typed numbers and options that become
 *  one modify command. Pure state machines with their prompts (i18n keys), as the draw tools are;
 *  `useTool` feeds them picks, boxes, clicks and typed input and runs each finished `command`.
 *
 *  A selection made before the tool starts is used at once (stretch excepted: it needs its own
 *  crossing window). After a command the tool asks for a new selection; copy keeps placing copies
 *  until Enter. */
import type { Command, ModifySpec } from "../api";
import type { Bounds, Point } from "../viewport/camera";

export const MODIFY_TOOLS = [
  "move",
  "copy",
  "rotate",
  "scale",
  "mirror",
  "stretch",
  "align",
  "array",
  "explode",
  "erase",
] as const;

export type ModifyKind = (typeof MODIFY_TOOLS)[number];

export const isModifyTool = (tool: string): tool is ModifyKind =>
  (MODIFY_TOOLS as readonly string[]).includes(tool);

/** The number typed next: copies in a row, a rotation angle, a scale factor, the array's rows,
 *  columns, gaps, items or angle between items. */
type Asking = "count" | "angle" | "factor" | "rows" | "cols" | "dy" | "dx" | "items" | "between" | "erase" | "arrayKind";

export type ModifyState = {
  kind: ModifyKind;
  /** "select": picking entities until Enter; "window": stretch's crossing window; "points": the
   *  points and numbers of the change. */
  phase: "select" | "window" | "points";
  /** The entities picked, top-level handles. */
  handles: number[];
  /** Points placed in this phase: a base, a mirror line, align's pairs, a window's corner, a
   *  polar array's centre. */
  points: Point[];
  /** What a typed number or option answers now; null while a point is wanted. */
  asking: Asking | null;
  /** Rotate and scale: change a copy, keeping the originals (option C). */
  copy: boolean;
  /** Copy: copies in a row each time (option A). */
  count: number;
  /** Array: "R" rectangular or "P" polar, and the numbers typed so far. */
  array: { kind: "R" | "P"; rows: number; cols: number; dy: number; items: number };
  /** Stretch: the window, [lower left, upper right]. */
  window: Bounds | null;
  /** Stretch: the window to look through, waiting for what it touches (a `picked` input). */
  seek: Bounds | null;
  /** Set by the input that finished a change; the hook runs it. */
  command: Command | null;
  /** Dimensions and tables the last input left out (this tool can't change them yet), to say so
   *  once. */
  skipped: number;
  hint: string;
  hintArgs: Record<string, string> | null;
};

export type ModifyInput =
  | { type: "point"; p: Point }
  | { type: "number"; value: number }
  | { type: "option"; option: string }
  /** Enter, or a double click. */
  | { type: "finish" }
  /** Entities picked by a click or a box (top-level handles), or what a stretch window touches;
   *  `kinds` gives each one's entity kind ("Line", "Dimension"…) when known. */
  | { type: "picked"; handles: number[]; kinds?: (string | null | undefined)[] }
  /** A box dragged in the viewport, for stretch's window. */
  | { type: "box"; bounds: Bounds };

/** Kinds drawn from a block of their own, which isn't made again yet: only move and copy change
 *  them (erase and explode take them as they are). */
const BLOCK_DRAWN = new Set(["Dimension", "Table"]);
const TAKES_BLOCK_DRAWN = new Set<ModifyKind>(["move", "copy", "erase", "explode"]);

/** `handles` less the dimensions and tables `kind` can't change, and how many those were. */
function admit(
  kind: ModifyKind,
  handles: number[],
  kindOf: (h: number, i: number) => string | null | undefined,
): { handles: number[]; skipped: number } {
  if (TAKES_BLOCK_DRAWN.has(kind)) return { handles, skipped: 0 };
  const kept = handles.filter((h, i) => !BLOCK_DRAWN.has(kindOf(h, i) ?? ""));
  return { handles: kept, skipped: handles.length - kept.length };
}

const NO_ARRAY = { kind: "R" as const, rows: 1, cols: 1, dy: 0, items: 0 };

const selectHint = (s: ModifyState): ModifyState => ({
  ...s,
  hint: "modify.prompt.select",
  hintArgs: { n: String(s.handles.length) },
});

/** A fresh tool: with `selection` (entities picked before, their kinds in `kinds`), straight to
 *  the points. */
export function initialModifyState(
  kind: ModifyKind,
  selection: number[],
  kinds: Record<number, string> = {},
): ModifyState {
  const s: ModifyState = {
    kind,
    phase: kind === "stretch" ? "window" : "select",
    handles: [],
    points: [],
    asking: null,
    copy: false,
    count: 1,
    array: NO_ARRAY,
    window: null,
    seek: null,
    command: null,
    skipped: 0,
    hint: kind === "stretch" ? "modify.prompt.stretchWindow" : "modify.prompt.select",
    hintArgs: kind === "stretch" ? null : { n: "0" },
  };
  if (kind === "stretch" || selection.length === 0) return s;
  const { handles, skipped } = admit(kind, [...new Set(selection)], (h) => kinds[h]);
  if (handles.length === 0) return { ...s, skipped };
  return { ...startPoints({ ...s, handles }), skipped };
}

const targets = (s: ModifyState) => s.handles.map((handle) => ({ handle, chain: [] }));

const modify = (spec: ModifySpec): Command => ({ type: "modify", ...spec });

/** The change is made: back to a new selection. */
const done = (s: ModifyState, command: Command): ModifyState => ({
  ...initialModifyState(s.kind, []),
  command,
});

/** The first prompt once the entities are picked; erase and explode need nothing more. */
function startPoints(s: ModifyState): ModifyState {
  const p: ModifyState = { ...s, phase: "points", points: [], asking: null, hintArgs: null };
  switch (s.kind) {
    case "erase":
      return done(s, { type: "delete_entities", targets: targets(s) });
    case "explode":
      return done(s, modify({ kind: "explode", targets: targets(s) }));
    case "mirror":
      return { ...p, hint: "modify.prompt.mirrorFirst" };
    case "align":
      return { ...p, hint: "modify.prompt.source1" };
    case "array":
      return { ...p, asking: "arrayKind", hint: "modify.prompt.arrayKind" };
    default:
      return { ...p, hint: "modify.prompt.base" };
  }
}

/** The option words the command line takes now. */
export function modifyOptions(s: ModifyState): string[] {
  if (s.phase !== "points") return [];
  if (s.asking === "erase") return ["Y", "N"];
  if (s.asking === "arrayKind") return ["R", "P"];
  if (s.kind === "copy" && s.points.length === 1 && !s.asking) return ["A"];
  if ((s.kind === "rotate" || s.kind === "scale") && s.points.length === 1) return ["C"];
  return [];
}

/** The tool takes a typed number now: "distance" (above 0: a count, a factor), "angle" (any
 *  sign: an angle, an array's gap) or false. */
export function takesModifyNumber(s: ModifyState): "distance" | "angle" | false {
  if (s.phase !== "points") return false;
  if (s.kind === "rotate" && s.points.length === 1) return "angle";
  if (s.kind === "scale" && s.points.length === 1) return "distance";
  switch (s.asking) {
    case "count":
    case "rows":
    case "cols":
    case "items":
      return "distance";
    case "dy":
    case "dx":
    case "between":
      return "angle";
    default:
      return false;
  }
}

const sub = (a: Point, b: Point): Point => [a[0] - b[0], a[1] - b[1]];
const whole = (v: number, min: number) => Number.isInteger(v) && v >= min;

/** The box from corner `a` to corner `b`, as [lower left, upper right]. */
const box = (a: Point, b: Point): Bounds => [
  [Math.min(a[0], b[0]), Math.min(a[1], b[1])],
  [Math.max(a[0], b[0]), Math.max(a[1], b[1])],
];

/** Stretch's window is known: look up what it touches. */
function seekWindow(s: ModifyState, window: Bounds): ModifyState {
  if (window[0][0] === window[1][0] || window[0][1] === window[1][1]) return s;
  return { ...s, window, seek: window, points: [], hint: "modify.prompt.finding", hintArgs: null };
}

function addPoint(s: ModifyState, q: Point): ModifyState {
  if (s.phase === "select") return s;
  if (s.phase === "window") {
    if (s.seek) return s;
    if (s.points.length === 0) return { ...s, points: [q], hint: "modify.prompt.stretchCorner" };
    return seekWindow(s, box(s.points[0], q));
  }
  // A number or an option is wanted, not a point.
  if (s.asking) return s;
  const points = [...s.points, q];
  const [a] = points;
  const t = targets(s);
  switch (s.kind) {
    case "move":
    case "copy":
    case "stretch": {
      if (points.length === 1) return { ...s, points, hint: "modify.prompt.second", hintArgs: null };
      const delta = sub(q, a);
      if (s.kind === "move") return done(s, modify({ kind: "move", targets: t, delta }));
      if (s.kind === "stretch") {
        return done(s, modify({ kind: "stretch", targets: t, window: s.window!, delta }));
      }
      // Copy: another copy from the same base next.
      const command = modify({ kind: "copy", targets: t, delta, count: s.count });
      return { ...s, points: [a], command, hint: "modify.prompt.copyMore", hintArgs: { count: String(s.count) } };
    }
    case "rotate": {
      if (points.length === 1) return { ...s, points, hint: angleHint(s) };
      const angle = (Math.atan2(q[1] - a[1], q[0] - a[0]) * 180) / Math.PI;
      return done(s, modify({ kind: "rotate", targets: t, base: a, angle, copy: s.copy }));
    }
    case "scale": {
      if (points.length === 1) return { ...s, points, hint: factorHint(s) };
      const factor = Math.hypot(q[0] - a[0], q[1] - a[1]);
      if (!(factor > 0)) return s;
      return done(s, modify({ kind: "scale", targets: t, base: a, factor, copy: s.copy }));
    }
    case "mirror":
      if (points.length === 1) return { ...s, points, hint: "modify.prompt.mirrorSecond" };
      if (q[0] === a[0] && q[1] === a[1]) return s;
      return { ...s, points, asking: "erase", hint: "modify.prompt.mirrorErase" };
    case "align": {
      const hints = ["modify.prompt.dest1", "modify.prompt.source2", "modify.prompt.dest2"];
      if (points.length < 4) return { ...s, points, hint: hints[points.length - 1] };
      return done(s, modify({ kind: "align", targets: t, pairs: [[points[0], points[1]], [points[2], points[3]]] }));
    }
    case "array":
      // The polar centre; then the number of items.
      return { ...s, points, asking: "items", hint: "modify.prompt.items" };
    default:
      return s;
  }
}

const angleHint = (s: ModifyState) => (s.copy ? "modify.prompt.angleCopy" : "modify.prompt.angle");
const factorHint = (s: ModifyState) => (s.copy ? "modify.prompt.factorCopy" : "modify.prompt.factor");

function typedNumber(s: ModifyState, v: number): ModifyState {
  const takes = takesModifyNumber(s);
  if (!takes || !Number.isFinite(v) || (takes === "distance" && !(v > 0))) return s;
  const t = targets(s);
  const base = s.points[0];
  if (s.kind === "rotate" && !s.asking) {
    return done(s, modify({ kind: "rotate", targets: t, base, angle: v, copy: s.copy }));
  }
  if (s.kind === "scale" && !s.asking) {
    return done(s, modify({ kind: "scale", targets: t, base, factor: v, copy: s.copy }));
  }
  const a = s.array;
  switch (s.asking) {
    case "count":
      if (!whole(v, 1)) return s;
      return { ...s, count: v, asking: null, hint: "modify.prompt.second", hintArgs: null };
    case "rows":
      if (!whole(v, 1)) return s;
      return { ...s, array: { ...a, rows: v }, asking: "cols", hint: "modify.prompt.cols" };
    case "cols":
      if (!whole(v, 1) || a.rows * v < 2) return s;
      if (a.rows === 1) return { ...s, array: { ...a, cols: v }, asking: "dx", hint: "modify.prompt.colGap" };
      return { ...s, array: { ...a, cols: v }, asking: "dy", hint: "modify.prompt.rowGap" };
    case "dy":
      if (a.cols === 1) return done(s, modify({ kind: "array_rect", targets: t, rows: a.rows, cols: 1, dy: v, dx: 0 }));
      return { ...s, array: { ...a, dy: v }, asking: "dx", hint: "modify.prompt.colGap" };
    case "dx":
      return done(s, modify({ kind: "array_rect", targets: t, rows: a.rows, cols: a.cols, dy: a.dy, dx: v }));
    case "items":
      if (!whole(v, 2)) return s;
      return { ...s, array: { ...a, items: v }, asking: "between", hint: "modify.prompt.between" };
    case "between":
      if (v === 0) return s;
      return polar(s, v);
    default:
      return s;
  }
}

const polar = (s: ModifyState, angle: number): ModifyState =>
  done(
    s,
    modify({ kind: "array_polar", targets: targets(s), centre: s.points[0], count: s.array.items, angle }),
  );

function option(s: ModifyState, word: string): ModifyState {
  if (!modifyOptions(s).includes(word)) return s;
  switch (word) {
    case "A":
      return { ...s, asking: "count", hint: "modify.prompt.copyCount", hintArgs: null };
    case "C": {
      const next = { ...s, copy: !s.copy };
      return { ...next, hint: s.kind === "rotate" ? angleHint(next) : factorHint(next) };
    }
    case "Y":
    case "N": {
      const [p1, p2] = s.points;
      return done(s, modify({ kind: "mirror", targets: targets(s), p1, p2, keep: word === "N" }));
    }
    case "R":
      return { ...s, array: { ...NO_ARRAY, kind: "R" }, asking: "rows", hint: "modify.prompt.rows" };
    case "P":
      return { ...s, array: { ...NO_ARRAY, kind: "P" }, asking: null, hint: "modify.prompt.centre" };
    default:
      return s;
  }
}

function finishStep(s: ModifyState): ModifyState {
  if (s.phase === "select") return s.handles.length > 0 ? startPoints(s) : selectHint(s);
  if (s.phase !== "points") return s;
  switch (s.asking) {
    case "erase":
      return option(s, "N");
    case "arrayKind":
      return option(s, "R");
    case "between":
      return polar(s, 360 / s.array.items);
    default:
      break;
  }
  // Align with one pair moves only.
  if (s.kind === "align" && s.points.length === 2) {
    return done(s, modify({ kind: "align", targets: targets(s), pairs: [[s.points[0], s.points[1]]] }));
  }
  // Copy is done placing copies.
  if (s.kind === "copy" && s.points.length === 1) return { ...initialModifyState(s.kind, []) };
  return s;
}

export function modifyReducer(state: ModifyState, input: ModifyInput): ModifyState {
  // A finished change, and what was left out, is pending for one state only: the hook sees it once.
  const s = { ...state, command: null, skipped: 0 };
  switch (input.type) {
    case "point":
      return addPoint(s, input.p);
    case "number":
      return typedNumber(s, input.value);
    case "option":
      return option(s, input.option);
    case "finish":
      return finishStep(s);
    case "box":
      return s.phase === "window" && !s.seek ? seekWindow(s, input.bounds) : s;
    case "picked": {
      const { handles: admitted, skipped } = admit(s.kind, input.handles, (_, i) => input.kinds?.[i]);
      if (s.phase === "window") {
        if (!s.seek) return s;
        if (admitted.length === 0) {
          return { ...s, seek: null, window: null, points: [], skipped, hint: "modify.prompt.nothing", hintArgs: null };
        }
        return { ...s, seek: null, handles: admitted, phase: "points", points: [], skipped, hint: "modify.prompt.base" };
      }
      if (s.phase !== "select") return s;
      const handles = [...new Set([...s.handles, ...admitted])];
      return selectHint({ ...s, handles, skipped });
    }
  }
}

/** Segments `x0 y0 x1 y1` through `path`. */
function segments(path: Point[]): Float64Array {
  const out = new Float64Array(4 * Math.max(0, path.length - 1));
  for (let i = 1; i < path.length; i++) out.set([...path[i - 1], ...path[i]], 4 * (i - 1));
  return out;
}

/** What the tool shows with the cursor at `cursor`: a line from its last point, or stretch's
 *  window being drawn; segments in drawing coordinates for the overlay. */
export function modifyPreview(s: ModifyState, cursor: Point | null): Float64Array {
  const last = s.points[s.points.length - 1];
  if (!cursor || !last || s.seek) return new Float64Array();
  if (s.phase === "window") {
    const [a, b] = [last, cursor];
    return segments([a, [b[0], a[1]], b, [a[0], b[1]], a]);
  }
  if (s.asking || (s.kind === "array" && s.array.kind === "P")) return new Float64Array();
  return segments([last, cursor]);
}

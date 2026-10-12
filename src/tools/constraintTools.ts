/** The parametric tools: geometric constraints (GEOMCONSTRAINT: coincident … equal), dimensional
 *  ones (DIMCONSTRAINT: linear, aligned, angular, radius, diameter), AUTOCONSTRAIN and
 *  DELCONSTRAINT. A pure state machine with its prompts (i18n keys), as the annotate tools are:
 *  `useTool` feeds it clicks on entities (with where they were, so the core takes the vertex or
 *  edge nearest the click), points, boxes and Enter, and runs each finished `command`. Every tool
 *  stays on for the next one. */
import type { Command, ConstraintKind, EntityTarget } from "../api";
import type { Point } from "../viewport/camera";

/** Each geometric tool and the constraint it adds, with how many picks it takes. */
const GEOMETRIC: Record<string, [ConstraintKind, number]> = {
  gcCoincident: ["coincident", 2],
  gcCollinear: ["collinear", 2],
  gcConcentric: ["concentric", 2],
  gcFixed: ["fixed", 1],
  gcParallel: ["parallel", 2],
  gcPerpendicular: ["perpendicular", 2],
  gcHorizontal: ["horizontal", 1],
  gcVertical: ["vertical", 1],
  gcTangent: ["tangent", 2],
  gcSmooth: ["smooth", 2],
  gcSymmetric: ["symmetric", 3],
  gcEqual: ["equal", 2],
};

/** Each dimensional tool: its kind and how many picks before the dimension's place. */
const DIMENSIONAL: Record<string, [ConstraintKind, number]> = {
  dcLinear: ["linear", 1],
  dcAligned: ["aligned", 1],
  dcAngular: ["angular", 2],
  dcRadius: ["radius", 1],
  dcDiameter: ["diameter", 1],
};

export const CONSTRAINT_TOOLS = [
  "gcCoincident",
  "gcCollinear",
  "gcConcentric",
  "gcFixed",
  "gcParallel",
  "gcPerpendicular",
  "gcHorizontal",
  "gcVertical",
  "gcTangent",
  "gcSmooth",
  "gcSymmetric",
  "gcEqual",
  "autoConstrain",
  "dcLinear",
  "dcAligned",
  "dcAngular",
  "dcRadius",
  "dcDiameter",
  "delConstraint",
] as const;

export type ConstraintTool = (typeof CONSTRAINT_TOOLS)[number];

export const isConstraintTool = (tool: string): tool is ConstraintTool =>
  (CONSTRAINT_TOOLS as readonly string[]).includes(tool);

/** The tool for a constraint kind (the bar's and the command line's words). */
export function toolOf(kind: ConstraintKind): ConstraintTool {
  const all = { ...GEOMETRIC, ...DIMENSIONAL };
  return Object.keys(all).find((t) => all[t][0] === kind) as ConstraintTool;
}

export type ConstraintState = {
  kind: ConstraintTool;
  /** "pick": a click on an entity; "point": the dimension's place; "select": entities until Enter. */
  phase: "pick" | "point" | "select";
  /** The entities picked so far and where each click was. */
  picks: { handle: number; at: Point }[];
  /** AUTOCONSTRAIN's and DELCONSTRAINT's entities. */
  handles: number[];
  /** Set by the input that finished one; the hook runs it. */
  command: Command | null;
  hint: string;
  hintArgs: Record<string, string> | null;
};

export type ConstraintInput =
  | { type: "pick"; handle: number; at: Point }
  | { type: "picked"; handles: number[] }
  | { type: "point"; p: Point }
  | { type: "finish" };

const target = (handle: number): EntityTarget => ({ handle, chain: [] });
const selects = (kind: ConstraintTool) => kind === "autoConstrain" || kind === "delConstraint";

/** How many entities the tool picks before it finishes (or asks for the place). */
const needs = (kind: ConstraintTool) => (GEOMETRIC[kind] ?? DIMENSIONAL[kind])?.[1] ?? 0;

function pickHint(s: ConstraintState, picked: number): ConstraintState {
  return { ...s, hint: "param.prompt.pick", hintArgs: { n: String(picked + 1), of: String(needs(s.kind)) } };
}

/** The tool waiting for its first input again. */
function restart(s: ConstraintState): ConstraintState {
  const base = { ...s, picks: [], handles: [], hintArgs: null };
  if (selects(s.kind)) return { ...base, phase: "select", hint: "param.prompt.select", hintArgs: { n: "0" } };
  return pickHint({ ...base, phase: "pick" }, 0);
}

/** The command the select tools make on `handles`. */
function selected(kind: ConstraintTool, handles: number[]): Command {
  const targets = handles.map(target);
  return kind === "autoConstrain"
    ? { type: "auto_constrain", targets }
    : { type: "delete_constraints_on", targets };
}

/** A fresh tool; AUTOCONSTRAIN and DELCONSTRAINT run at once on a `selection` made before. */
export function initialConstraintState(kind: ConstraintTool, selection: number[] = []): ConstraintState {
  const s = restart({ kind, phase: "pick", picks: [], handles: [], command: null, hint: "", hintArgs: null });
  const handles = [...new Set(selection)];
  return selects(kind) && handles.length > 0 ? { ...s, command: selected(kind, handles) } : s;
}

function pick(s: ConstraintState, handle: number, at: Point): ConstraintState {
  if (s.phase === "select") return addHandles(s, [handle]);
  if (s.phase !== "pick") return s;
  const picks = [...s.picks, { handle, at }];
  if (picks.length < needs(s.kind)) return pickHint({ ...s, picks }, picks.length);
  const geometric = GEOMETRIC[s.kind];
  if (geometric) {
    return {
      ...restart(s),
      command: {
        type: "add_constraint",
        kind: geometric[0],
        targets: picks.map((p) => target(p.handle)),
        points: picks.map((p) => p.at),
      },
    };
  }
  return { ...s, picks, phase: "point", hint: "param.prompt.place", hintArgs: null };
}

function place(s: ConstraintState, p: Point): ConstraintState {
  const dimensional = DIMENSIONAL[s.kind];
  if (s.phase !== "point" || !dimensional) return s;
  return {
    ...restart(s),
    command: {
      type: "add_dim_constraint",
      kind: dimensional[0],
      targets: s.picks.map((x) => target(x.handle)),
      points: s.picks.map((x) => x.at),
      line_point: p,
      expression: null,
    },
  };
}

function addHandles(s: ConstraintState, handles: number[]): ConstraintState {
  if (s.phase !== "select") return s;
  const all = [...new Set([...s.handles, ...handles])];
  return { ...s, handles: all, hintArgs: { n: String(all.length) } };
}

export function constraintReducer(state: ConstraintState, input: ConstraintInput): ConstraintState {
  // A finished command is pending for one state only: the hook sees it once.
  const s = { ...state, command: null };
  switch (input.type) {
    case "pick":
      return pick(s, input.handle, input.at);
    case "picked":
      return addHandles(s, input.handles);
    case "point":
      return place(s, input.p);
    case "finish":
      if (s.phase === "select" && s.handles.length > 0) return { ...restart(s), command: selected(s.kind, s.handles) };
      return s;
  }
}

/** Clicks pick an entity now (else they place the dimension). */
export const constraintPicks = (s: ConstraintState) => s.phase !== "point";

/** A left drag draws a selection box now. */
export const constraintBoxes = (s: ConstraintState) => s.phase === "select";

/** The entities to draw highlighted: those picked so far. */
export const constraintHighlight = (s: ConstraintState): number[] => [
  ...new Set([...s.picks.map((p) => p.handle), ...s.handles]),
];

import { describe, expect, it } from "vitest";
import type { Command } from "../api";
import type { Point } from "../viewport/camera";
import {
  editOptions,
  editReducer,
  initialEditState,
  OVERKILL_TOL,
  takesEditNumber,
  type EditInput,
  type EditState,
} from "./editTools";

const t = (handle: number) => ({ handle, chain: [] });

/** Feeds `inputs` in order; returns the last state and every command made on the way. */
function feed(s: EditState, inputs: EditInput[]): { s: EditState; commands: Command[] } {
  const commands: Command[] = [];
  for (const input of inputs) {
    s = editReducer(s, input);
    if (s.command) commands.push(s.command);
  }
  return { s, commands };
}

const pick = (handle: number, at: Point, kind = "Line", shift = false): EditInput => ({
  type: "pick",
  handle,
  kind,
  at,
  shift,
});
const pt = (p: Point): EditInput => ({ type: "point", p });
const finish: EditInput = { type: "finish" };
const num = (value: number): EditInput => ({ type: "number", value });
const opt = (option: string): EditInput => ({ type: "option", option });

describe("trim and extend", () => {
  it("takes the cutting edges, then trims each piece clicked until Esc", () => {
    const { s, commands } = feed(initialEditState("trim"), [
      pick(1, [0, 0]),
      { type: "picked", handles: [2, 1] },
      finish,
      pick(5, [3, 4]),
      pick(6, [7, 8]),
    ]);
    expect(s.phase).toBe("pick");
    expect(s.hint).toBe("edit.prompt.trim");
    expect(commands).toEqual([
      { type: "edit", kind: "trim", cutters: [t(1), t(2)], target: t(5), at_point: [3, 4] },
      { type: "edit", kind: "trim", cutters: [t(1), t(2)], target: t(6), at_point: [7, 8] },
    ]);
  });

  it("Enter with no edges cuts against everything; Shift+click extends", () => {
    const { commands } = feed(initialEditState("trim"), [finish, pick(5, [3, 4], "Line", true)]);
    expect(commands).toEqual([{ type: "edit", kind: "extend", boundaries: [], target: t(5), at_end: [3, 4] }]);
  });

  it("extend starts on a selection made before as its boundaries", () => {
    const s = initialEditState("extend", [9]);
    expect(s.phase).toBe("pick");
    const { commands } = feed(s, [pick(4, [1, 1])]);
    expect(commands).toEqual([{ type: "edit", kind: "extend", boundaries: [t(9)], target: t(4), at_end: [1, 1] }]);
  });
});

describe("tools on a selection", () => {
  it("join, reverse, overkill and flatten run on Enter, or at once on a selection made before", () => {
    const { commands } = feed(initialEditState("join"), [pick(1, [0, 0]), pick(2, [1, 0]), finish]);
    expect(commands).toEqual([{ type: "edit", kind: "join", targets: [t(1), t(2)] }]);
    expect(initialEditState("overkill", [3, 3, 4]).command).toEqual({
      type: "edit",
      kind: "overkill",
      targets: [t(3), t(4)],
      tol: OVERKILL_TOL,
    });
    expect(initialEditState("flatten", [5]).command).toEqual({ type: "edit", kind: "flatten", targets: [t(5)] });
  });
});

describe("one entity after another", () => {
  it("break: the click is the first point, then the second; Enter splits at the first", () => {
    const { commands } = feed(initialEditState("break"), [
      pick(1, [2, 0]),
      pt([5, 0]),
      pick(1, [8, 0]),
      finish,
    ]);
    expect(commands).toEqual([
      { type: "edit", kind: "break", target: t(1), p1: [2, 0], p2: [5, 0] },
      { type: "edit", kind: "break", target: t(1), p1: [8, 0], p2: [8, 0] },
    ]);
  });

  it("offset: the distance, then the object and its side, again and again", () => {
    let s = initialEditState("offset");
    expect(takesEditNumber(s)).toBe("distance");
    const { commands } = feed(s, [num(0.2), pick(1, [0, 0]), pt([0, -1]), pick(2, [5, 5]), pt([6, 6])]);
    expect(commands).toEqual([
      { type: "edit", kind: "offset", target: t(1), distance: 0.2, side_point: [0, -1] },
      { type: "edit", kind: "offset", target: t(2), distance: 0.2, side_point: [6, 6] },
    ]);
    s = editReducer(initialEditState("offset"), num(-1));
    expect(s.asking).toBe("offset");
  });

  it("fillet: R sets the radius, then two clicks", () => {
    let s = initialEditState("fillet");
    expect(editOptions(s)).toEqual(["R"]);
    const { commands, s: after } = feed(s, [opt("R"), num(0.5), pick(1, [1, 0]), pick(2, [4, 2])]);
    expect(commands).toEqual([
      { type: "edit", kind: "fillet", a: t(1), b: t(2), at_a: [1, 0], at_b: [4, 2], radius: 0.5 },
    ]);
    expect(after.hintArgs).toEqual({ r: "0.5" });
    // One polyline's two sides.
    s = feed(after, [pick(3, [2, 0], "LwPolyline"), pick(3, [4, 1], "LwPolyline")]).s;
    expect(s.first).toBeNull();
  });

  it("chamfer: D sets both distances", () => {
    const { commands } = feed(initialEditState("chamfer"), [
      opt("D"),
      num(0.5),
      num(0.25),
      pick(7, [2, 0], "LwPolyline"),
      pick(7, [4, 1.5], "LwPolyline"),
    ]);
    expect(commands).toEqual([
      { type: "edit", kind: "chamfer", a: t(7), b: t(7), at_a: [2, 0], at_b: [4, 1.5], d1: 0.5, d2: 0.25 },
    ]);
  });

  it("blend: near the end of each", () => {
    const { commands } = feed(initialEditState("blend"), [pick(1, [1.9, 0]), pick(2, [4, 2.1])]);
    expect(commands).toEqual([{ type: "edit", kind: "blend", a: t(1), b: t(2), at_a: [1.9, 0], at_b: [4, 2.1] }]);
  });

  it("lengthen: by a change (any sign), a total or a percentage, then the ends clicked", () => {
    let s = initialEditState("lengthen");
    expect(takesEditNumber(s)).toBe("angle");
    const { commands } = feed(s, [num(-0.5), pick(1, [9, 0]), opt("T"), num(3), pick(1, [0, 0])]);
    expect(commands).toEqual([
      { type: "edit", kind: "lengthen", target: t(1), at_end: [9, 0], by: { delta: -0.5 } },
      { type: "edit", kind: "lengthen", target: t(1), at_end: [0, 0], by: { total: 3 } },
    ]);
    s = feed(initialEditState("lengthen"), [opt("P")]).s;
    expect(s.hint).toBe("edit.prompt.lengthen.percent");
    expect(takesEditNumber(s)).toBe("distance");
  });
});

describe("pedit", () => {
  it("picks a polyline, then runs each change on it", () => {
    const { commands, s } = feed(initialEditState("pedit"), [
      pick(4, [0, 0], "LwPolyline"),
      opt("C"),
      opt("W"),
      num(0.2),
      opt("I"),
      pt([2, 0.1]),
      opt("R"),
      pt([2, 0]),
      opt("F"),
      opt("D"),
    ]);
    const target = t(4);
    expect(commands).toEqual([
      { type: "edit", kind: "pedit", target, op: "close" },
      { type: "edit", kind: "pedit", target, op: { width: 0.2 } },
      { type: "edit", kind: "pedit", target, op: { add_vertex: [2, 0.1] } },
      { type: "edit", kind: "pedit", target, op: { delete_vertex: [2, 0] } },
      { type: "edit", kind: "pedit", target, op: "fit_curve" },
      { type: "edit", kind: "pedit", target, op: "decurve" },
    ]);
    expect(s.hint).toBe("edit.prompt.peditOp");
  });

  it("offers to turn a line into a polyline (Enter: yes), and refuses a circle", () => {
    const { commands, s } = feed(initialEditState("pedit"), [pick(5, [0, 0], "Line"), finish, opt("O")]);
    expect(commands).toEqual([
      { type: "edit", kind: "pedit", target: t(5), op: "convert_to_polyline" },
      { type: "edit", kind: "pedit", target: t(5), op: "open" },
    ]);
    expect(s.asking).toBe("op");
    const circle = editReducer(initialEditState("pedit"), pick(6, [0, 0], "Circle"));
    expect(circle.hint).toBe("edit.prompt.notPolyline");
    expect(circle.first).toBeNull();
  });
});

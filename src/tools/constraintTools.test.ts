import { describe, expect, it } from "vitest";
import type { Command } from "../api";
import type { Point } from "../viewport/camera";
import {
  constraintBoxes,
  constraintHighlight,
  constraintPicks,
  constraintReducer,
  initialConstraintState,
  isConstraintTool,
  toolOf,
  type ConstraintInput,
  type ConstraintState,
} from "./constraintTools";

const t = (handle: number) => ({ handle, chain: [] });

function feed(s: ConstraintState, inputs: ConstraintInput[]) {
  const commands: Command[] = [];
  for (const input of inputs) {
    s = constraintReducer(s, input);
    if (s.command) commands.push(s.command);
  }
  return { s, commands };
}

const pick = (handle: number, at: Point): ConstraintInput => ({ type: "pick", handle, at });

describe("geometric constraints", () => {
  it("Horizontal takes one pick and stays on for the next one", () => {
    const { s, commands } = feed(initialConstraintState("gcHorizontal"), [pick(7, [2, 0.1])]);
    expect(commands).toEqual([{ type: "add_constraint", kind: "horizontal", targets: [t(7)], points: [[2, 0.1]] }]);
    expect(s.phase).toBe("pick");
    expect(s.hint).toBe("param.prompt.pick");
    expect(s.hintArgs).toEqual({ n: "1", of: "1" });
  });

  it("Perpendicular takes two picks, first one first; the same entity twice is allowed", () => {
    let s = initialConstraintState("gcPerpendicular");
    s = constraintReducer(s, pick(5, [4, 1.5]));
    expect(s.command).toBeNull();
    expect(s.hintArgs).toEqual({ n: "2", of: "2" });
    expect(constraintHighlight(s)).toEqual([5]);
    const { commands } = feed(s, [pick(5, [2, 0])]);
    expect(commands).toEqual([
      { type: "add_constraint", kind: "perpendicular", targets: [t(5), t(5)], points: [[4, 1.5], [2, 0]] },
    ]);
  });

  it("Symmetric takes two points and the mirror line", () => {
    const { commands } = feed(initialConstraintState("gcSymmetric"), [pick(1, [0, 0]), pick(2, [4, 0]), pick(3, [2, 5])]);
    expect(commands[0]).toMatchObject({ kind: "symmetric", targets: [t(1), t(2), t(3)] });
  });
});

describe("dimensional constraints", () => {
  it("Linear picks the edge, then where the dimension goes", () => {
    let s = initialConstraintState("dcLinear");
    s = constraintReducer(s, pick(9, [2, 0]));
    expect(s.phase).toBe("point");
    expect(constraintPicks(s)).toBe(false);
    expect(s.hint).toBe("param.prompt.place");
    const { s: after, commands } = feed(s, [{ type: "point", p: [2, -1] }]);
    expect(commands).toEqual([
      {
        type: "add_dim_constraint",
        kind: "linear",
        targets: [t(9)],
        points: [[2, 0]],
        line_point: [2, -1],
        expression: null,
      },
    ]);
    expect(after.phase).toBe("pick");
  });

  it("Angular takes two lines before its place", () => {
    const { s } = feed(initialConstraintState("dcAngular"), [pick(1, [1, 0]), pick(2, [0, 1])]);
    expect(s.phase).toBe("point");
    const { commands } = feed(s, [{ type: "point", p: [1, 1] }]);
    expect(commands[0]).toMatchObject({ type: "add_dim_constraint", kind: "angular", targets: [t(1), t(2)] });
  });
});

describe("AUTOCONSTRAIN and DELCONSTRAINT", () => {
  it("run at once on a selection made before", () => {
    expect(initialConstraintState("autoConstrain", [3, 3, 4]).command).toEqual({
      type: "auto_constrain",
      targets: [t(3), t(4)],
    });
    expect(initialConstraintState("delConstraint", [8]).command).toEqual({
      type: "delete_constraints_on",
      targets: [t(8)],
    });
  });

  it("else pick by clicks and boxes until Enter", () => {
    let s = initialConstraintState("autoConstrain");
    expect(constraintBoxes(s)).toBe(true);
    expect(feed(s, [{ type: "finish" }]).commands).toEqual([]);
    s = constraintReducer(s, pick(1, [0, 0]));
    s = constraintReducer(s, { type: "picked", handles: [2, 1] });
    expect(s.hintArgs).toEqual({ n: "2" });
    const { commands } = feed(s, [{ type: "finish" }]);
    expect(commands).toEqual([{ type: "auto_constrain", targets: [t(1), t(2)] }]);
  });
});

it("names its tools and finds the tool of a kind", () => {
  expect(isConstraintTool("gcEqual")).toBe(true);
  expect(isConstraintTool("dimLinear")).toBe(false);
  expect(toolOf("diameter")).toBe("dcDiameter");
  expect(toolOf("smooth")).toBe("gcSmooth");
});

import { describe, expect, it } from "vitest";
import type { Command } from "../api";
import type { Point } from "../viewport/camera";
import {
  dimOptions,
  dimPicks,
  dimPreview,
  dimReducer,
  initialDimState,
  takesDimNumber,
  toleranceFrame,
  type DimInput,
  type DimRequest,
  type DimState,
} from "./dimTools";

const t = (handle: number) => ({ handle, chain: [] });

/** Feeds `inputs` in order; returns the last state, the commands and the requests made. */
function feed(s: DimState, inputs: DimInput[]) {
  const commands: Command[] = [];
  const requests: DimRequest[] = [];
  for (const input of inputs) {
    s = dimReducer(s, input);
    if (s.command) commands.push(s.command);
    if (s.request) requests.push(s.request);
  }
  return { s, commands, requests };
}

const pt = (p: Point): DimInput => ({ type: "point", p });
const pick = (handle: number, kind: string, at: Point = [0, 0]): DimInput => ({ type: "pick", handle, kind, at });
const finish: DimInput = { type: "finish" };
const opt = (option: string): DimInput => ({ type: "option", option });

describe("point dimensions", () => {
  it("DIMLINEAR takes two points and the line's, then starts again; V makes it vertical", () => {
    const { s, commands } = feed(initialDimState("dimLinear"), [
      pt([0, 0]),
      pt([10, 0]),
      pt([5, 2]),
      opt("V"),
      pt([0, 0]),
      pt([0, 4]),
      pt([-1, 2]),
    ]);
    expect(commands).toEqual([
      { type: "dimension", kind: "linear", p1: [0, 0], p2: [10, 0], line_point: [5, 2], rotation: null },
      { type: "dimension", kind: "linear", p1: [0, 0], p2: [0, 4], line_point: [-1, 2], rotation: 90 },
    ]);
    expect(s.hint).toBe("dim.prompt.dimLinear0");
    expect(dimOptions(s)).toEqual(["H", "V"]);
  });

  it("DIMALIGNED offsets to the side clicked; DIMANGULAR takes four points; DIMORDINATE two", () => {
    expect(feed(initialDimState("dimAligned"), [pt([0, 0]), pt([4, 0]), pt([2, -1])]).commands).toEqual([
      { type: "dimension", kind: "aligned", p1: [0, 0], p2: [4, 0], offset: -1 },
    ]);
    expect(feed(initialDimState("dimAngular"), [pt([0, 0]), pt([1, 0]), pt([0, 1]), pt([1, 1])]).commands).toEqual([
      { type: "dimension", kind: "angular", vertex: [0, 0], p1: [1, 0], p2: [0, 1], arc_point: [1, 1] },
    ]);
    expect(feed(initialDimState("dimOrdinate"), [pt([4, 3]), pt([4, 6])]).commands).toEqual([
      { type: "dimension", kind: "ordinate", feature: [4, 3], leader_end: [4, 6] },
    ]);
  });
});

describe("dimensions of entities", () => {
  it("DIMRADIUS picks a circle or an arc, then where the text goes; a line is refused", () => {
    const { s, commands } = feed(initialDimState("dimRadius"), [pick(3, "Line"), pick(5, "Arc"), pt([2, 2])]);
    expect(commands).toEqual([{ type: "dimension", kind: "radius", target: t(5), point: [2, 2] }]);
    expect(s.phase).toBe("pick");
    expect(feed(initialDimState("dimRadius"), [pick(3, "Line")]).s.hint).toBe("dim.prompt.dimRadiusWrong");
    expect(feed(initialDimState("dimArc"), [pick(5, "Circle")]).s.first).toBeNull();
  });

  it("DIMBASELINE and DIMCONTINUE take a dimension, then points until Enter", () => {
    const { commands } = feed(initialDimState("dimContinue"), [
      pick(7, "Dimension"),
      pt([12, 0]),
      pt([15, 0]),
      finish,
    ]);
    expect(commands).toEqual([{ type: "dimension", kind: "continue", from: t(7), points: [[12, 0], [15, 0]] }]);
  });

  it("QDIM takes a selection, Enter, then the line's position", () => {
    const { commands } = feed(initialDimState("qdim", [1]), [
      { type: "picked", handles: [2, 3] },
      finish,
      pt([0, 4]),
    ]);
    expect(commands).toEqual([{ type: "dimension", kind: "quick", targets: [t(1), t(2), t(3)], line_point: [0, 4] }]);
  });
});

describe("dimension edits", () => {
  it("DIMEDIT: T asks for the text, F flips, M moves the text to a point", () => {
    const start = initialDimState("dimEdit");
    const asked = feed(start, [pick(7, "Dimension"), opt("T")]);
    expect(asked.requests).toEqual([{ kind: "override", handle: 7 }]);
    expect(feed(start, [pick(7, "Dimension"), opt("F")]).commands).toEqual([
      { type: "dim_edit", target: t(7), op: "flip" },
    ]);
    const moved = feed(start, [pick(7, "Dimension"), opt("M"), pt([14, 2])]);
    expect(moved.commands).toEqual([{ type: "dim_edit", target: t(7), op: { move_text: [14, 2] } }]);
    expect(dimPicks(moved.s)).toBe(true);
  });

  it("DIMBREAK, DIMJOGLINE and DIMSPACE", () => {
    expect(feed(initialDimState("dimBreak"), [pick(7, "Dimension"), pick(9, "Line")]).commands).toEqual([
      { type: "dim_break", target: t(7), cutter: t(9) },
    ]);
    expect(feed(initialDimState("dimJogLine"), [pick(7, "Dimension"), pt([2, 2])]).commands).toEqual([
      { type: "dim_jog_line", target: t(7), at: [2, 2] },
    ]);
    const spaced = feed(initialDimState("dimSpace"), [
      pick(7, "Dimension"),
      pick(8, "Dimension"),
      { type: "picked", handles: [9, 7] },
      finish,
    ]);
    expect(takesDimNumber(spaced.s)).toBe("distance");
    expect(feed(spaced.s, [{ type: "number", value: 1.5 }]).commands).toEqual([
      { type: "dim_space", base: t(7), targets: [t(8), t(9)], spacing: 1.5 },
    ]);
    expect(feed(spaced.s, [finish]).commands).toEqual([
      { type: "dim_space", base: t(7), targets: [t(8), t(9)], spacing: null },
    ]);
  });
});

describe("leaders and tolerances", () => {
  it("LEADER takes points until Enter and asks for its text; one point is not enough", () => {
    expect(feed(initialDimState("leader"), [pt([0, 0]), finish]).requests).toEqual([]);
    const { requests, s } = feed(initialDimState("mleader"), [pt([0, 0]), pt([2, 2]), pt([4, 2]), finish]);
    expect(requests).toEqual([{ kind: "mleader", points: [[0, 0], [2, 2], [4, 2]] }]);
    expect(s.points).toEqual([]);
  });

  it("TOLERANCE asks for its frame where it is clicked, written in AutoCAD's codes", () => {
    expect(feed(initialDimState("tolerance"), [pt([1, 2])]).requests).toEqual([{ kind: "tolerance", at: [1, 2] }]);
    expect(toleranceFrame("p", "0.5", ["A"])).toBe("{\\Fgdt;p}%%v0.5%%v%%vA%%v%%v");
    expect(toleranceFrame("", "0.1", [])).toBe("%%v0.1%%v%%v%%v%%v");
  });
});

describe("comment cloud", () => {
  it("asks for the comment once Enter closes a ring of 3 corners or more", () => {
    const early = feed(initialDimState("commentCloud"), [pt([0, 0]), pt([4, 0]), finish]);
    expect(early.requests).toEqual([]);
    expect(early.s.hint).toBe("dim.prompt.comment1");
    const { requests, s } = feed(initialDimState("commentCloud"), [pt([0, 0]), pt([4, 0]), pt([4, 3]), finish]);
    expect(requests).toEqual([{ kind: "comment", points: [[0, 0], [4, 0], [4, 3]] }]);
    // Ready for the next one.
    expect(s.points).toEqual([]);
    expect(s.hint).toBe("dim.prompt.comment0");
  });

  it("previews the ring closed back to its first corner", () => {
    const { s } = feed(initialDimState("commentCloud"), [pt([0, 0]), pt([4, 0])]);
    expect(Array.from(dimPreview(s, [4, 3]))).toEqual([0, 0, 4, 0, 4, 0, 4, 3, 4, 3, 0, 0]);
  });
});

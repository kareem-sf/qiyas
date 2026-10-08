import { expect, test } from "vitest";
import type { EntityInfo, Picked } from "../api";
import type { Point } from "../viewport/camera";
import {
  initialToolState,
  selectedTarget,
  toolReducer,
  type ToolEvent,
  type ToolState,
} from "./useTool";

/** An entity on the sheet. */
const onSheet = (handle: number): Picked => ({ top: handle, entity: handle, chain: [], xref: null });

/** A click at `world`, drawn at the same screen point, `time` ms apart from the others. */
const click = (
  world: Point,
  time: number,
  picked: Picked | number | null = null,
  info: EntityInfo | null = null,
): ToolEvent => ({
  type: "click",
  world,
  screen: [world[0] * 10, world[1] * 10],
  time,
  picked: typeof picked === "number" ? onSheet(picked) : picked,
  info,
});

const run = (tool: ToolState["tool"], events: ToolEvent[]) =>
  events.reduce(toolReducer, initialToolState(tool));

const door: EntityInfo = {
  handle: 42,
  kind: "Insert",
  layer: "A-DOOR",
  length: null,
  area: null,
  block_name: "DOOR",
  text: null,
  inside_block: null,
  chain: [],
  block_path: [],
};

test("distance tool needs two points then a finish", () => {
  const one = run("distance", [click([0, 0], 0), { type: "finish" }]);
  expect(one.pendingCommand).toBeNull();
  expect(one.hint).toBe("measure.needTwo");

  const two = run("distance", [click([0, 0], 0), click([10, 0], 1000), { type: "finish" }]);
  expect(two.pendingCommand).toEqual({
    type: "measure_distance",
    points: [
      [0, 0],
      [10, 0],
    ],
  });
  expect(two.points).toEqual([]);
});

test("area tool rejects finish with two points", () => {
  const s = run("area", [click([0, 0], 0), click([4, 0], 1000), { type: "finish" }]);
  expect(s.pendingCommand).toBeNull();
  expect(s.hint).toBe("measure.needThree");
  expect(s.points).toHaveLength(2);
});

test("escape clears points", () => {
  const s = run("distance", [click([0, 0], 0), click([10, 0], 1000), { type: "escape" }]);
  expect(s.points).toEqual([]);
  expect(s.pendingCommand).toBeNull();
});

test("a double click finishes the area without a duplicate point", () => {
  const s = run("area", [
    click([0, 0], 0),
    click([4, 0], 1000),
    click([4, 3], 2000),
    click([4, 3], 2200),
  ]);
  expect(s.pendingCommand).toEqual({
    type: "measure_area",
    points: [
      [0, 0],
      [4, 0],
      [4, 3],
    ],
  });
  expect(s.points).toEqual([]);
  // A slow second click is just another point.
  expect(run("area", [click([0, 0], 0), click([0, 0], 400)]).points).toHaveLength(2);
});

test("count tool counts blocks and refuses other entities", () => {
  const counted = run("count", [click([1, 1], 0, 42, door)]);
  expect(counted.pendingCommand).toEqual({ type: "count_block", block_name: "DOOR", layer: null });

  const line = { ...door, kind: "Line", block_name: null };
  const refused = run("count", [click([1, 1], 0, 7, line)]);
  expect(refused.pendingCommand).toBeNull();
  expect(refused.hint).toBe("measure.notABlock");
});

test("select highlights, then a second click on the same entity measures it", () => {
  const first = run("select", [click([0, 0], 0, 7)]);
  expect(selectedTarget(first.selected)).toEqual({ handle: 7, chain: [] });
  expect(first.pendingCommand).toBeNull();
  expect(first.hint).toBe("");

  const second = toolReducer(first, click([5, 0], 1000, 7));
  expect(second.pendingCommand).toEqual({ type: "measure_entity", handle: 7, chain: [] });

  const other = toolReducer(first, click([5, 0], 1000, 8));
  expect(selectedTarget(other.selected)).toEqual({ handle: 8, chain: [] });
  expect(other.pendingCommand).toBeNull();
  expect(toolReducer(first, { type: "escape" }).selected).toBeNull();
});

test("inside a block, a second click toggles between the entity and the block", () => {
  // The leaf line 9 inside DOOR reference 8, which sits inside PLAN reference 5.
  const leaf: Picked = { top: 5, entity: 9, chain: [5, 8], xref: null };
  const info: EntityInfo = {
    ...door,
    handle: 9,
    kind: "Line",
    block_name: null,
    inside_block: "DOOR",
    chain: [5, 8],
    block_path: ["PLAN", "DOOR"],
  };
  const first = run("select", [click([0, 0], 0, leaf, info)]);
  expect(selectedTarget(first.selected)).toEqual({ handle: 9, chain: [5, 8] });
  expect([first.hint, first.hintArgs]).toEqual(["measure.insideBlock", { name: "DOOR" }]);

  const whole = toolReducer(first, click([0, 0], 1000, leaf, info));
  expect(whole.pendingCommand).toBeNull();
  expect(selectedTarget(whole.selected)).toEqual({ handle: 5, chain: [] });
  expect([whole.hint, whole.hintArgs]).toEqual(["measure.wholeBlock", { name: "PLAN" }]);

  const back = toolReducer(whole, click([0, 0], 2000, leaf, info));
  expect(selectedTarget(back.selected)).toEqual({ handle: 9, chain: [5, 8] });

  // Enter measures what is selected: the entity in world units, through its chain.
  expect(toolReducer(back, { type: "finish" }).pendingCommand).toEqual({
    type: "measure_entity",
    handle: 9,
    chain: [5, 8],
  });
  expect(toolReducer(whole, { type: "finish" }).pendingCommand).toEqual({
    type: "measure_entity",
    handle: 5,
    chain: [],
  });
  // So does a double click.
  const double = toolReducer(first, click([0, 0], 100, leaf, info));
  expect(double.pendingCommand).toEqual({ type: "measure_entity", handle: 9, chain: [5, 8] });
});

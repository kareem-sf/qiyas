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

test("a double click measures what the status bar showed before it", () => {
  const leaf: Picked = { top: 5, entity: 9, chain: [5, 8], xref: null };
  const info: EntityInfo = { ...door, handle: 9, chain: [5, 8], block_path: ["PLAN", "DOOR"] };
  const block = { type: "measure_entity", handle: 5, chain: [] };
  const entity = { type: "measure_entity", handle: 9, chain: [5, 8] };
  const inner = run("select", [click([0, 0], 0, leaf, info)]);
  const whole = toolReducer(inner, click([0, 0], 1000, leaf, info));
  expect(whole.hint).toBe("measure.wholeBlock");

  // Showing the whole block: the pair's first click swaps to the entity, the second takes it back.
  const fromWhole = [click([0, 0], 3000, leaf, info), click([0, 0], 3100, leaf, info)].reduce(
    toolReducer,
    whole,
  );
  expect(fromWhole.pendingCommand).toEqual(block);
  // Showing the entity, after a pause.
  const fromInner = [click([0, 0], 3000, leaf, info), click([0, 0], 3100, leaf, info)].reduce(
    toolReducer,
    inner,
  );
  expect(fromInner.pendingCommand).toEqual(entity);
  // A swap back and then a double click still measures what is shown.
  const back = toolReducer(whole, click([0, 0], 2000, leaf, info));
  expect(back.hint).toBe("measure.insideBlock");
  const fromBack = [click([0, 0], 4000, leaf, info), click([0, 0], 4100, leaf, info)].reduce(
    toolReducer,
    back,
  );
  expect(fromBack.pendingCommand).toEqual(entity);

  // Enter measures what the status shows, in both states.
  expect(toolReducer(whole, { type: "finish" }).pendingCommand).toEqual(block);
  expect(toolReducer(inner, { type: "finish" }).pendingCommand).toEqual(entity);
  expect(toolReducer(back, { type: "finish" }).pendingCommand).toEqual(entity);
});

const ORTHO = { ortho: true, polar: false, polarStepDeg: 45 };
const POLAR = { ortho: false, polar: true, polarStepDeg: 45 };

/** A click placing a point with tracking, `snapped` when it is an object snap's point. */
const tracked = (world: Point, time: number, track: typeof ORTHO, snapped = false): ToolEvent => ({
  type: "click",
  world,
  screen: [world[0] * 10, world[1] * 10],
  time,
  picked: null,
  info: null,
  track,
  snapped,
});

test("ortho keeps the second point level with the first", () => {
  const s = run("distance", [
    tracked([1, 1], 0, ORTHO),
    tracked([6, 1.8], 1000, ORTHO),
  ]);
  expect(s.points).toEqual([
    [1, 1],
    [6, 1],
  ]);
});

test("ortho goes up when the cursor is more up than across", () => {
  const s = run("area", [
    tracked([0, 0], 0, ORTHO),
    tracked([0.5, 4], 1000, ORTHO),
  ]);
  expect(s.points[1]).toEqual([0, 4]);
});

test("polar snaps the direction to 45 degrees when within 5 degrees", () => {
  const s = run("distance", [
    tracked([0, 0], 0, POLAR),
    // About 47 degrees.
    tracked([3, 3.2], 1000, POLAR),
  ]);
  const [x, y] = s.points[1];
  expect(x).toBeCloseTo(y, 9);
  expect(Math.hypot(x, y)).toBeCloseTo((3 + 3.2) / Math.SQRT2, 9);

  // 30 degrees is 15 away from both 0 and 45: left alone.
  const free = run("distance", [
    tracked([0, 0], 0, POLAR),
    tracked([Math.cos(Math.PI / 6), Math.sin(Math.PI / 6)], 1000, POLAR),
  ]);
  expect(free.points[1][0]).toBeCloseTo(Math.cos(Math.PI / 6), 12);
});

test("a snapped point is placed as it is, ortho or not", () => {
  const s = run("distance", [
    tracked([0, 0], 0, ORTHO),
    tracked([4, 3], 1000, ORTHO, true),
  ]);
  expect(s.points[1]).toEqual([4, 3]);
});

test("geometry asks about the entity clicked, inside its blocks, with the click point", () => {
  const inBlock: Picked = { top: 7, entity: 42, chain: [7], xref: null };
  const s = run("geometry", [click([3, 4], 0, inBlock)]);
  expect(s.pendingCommand).toEqual({ type: "measure_geometry", handle: 42, chain: [7], at: [3, 4] });
  const nothing = run("geometry", [click([3, 4], 0, null)]);
  expect(nothing.pendingCommand).toBeNull();
  expect(nothing.hint).toBe("inquiry.clickGeometry");
});

test("divide waits for the number of parts, then divides", () => {
  const asked = run("divide", [click([5, 0], 0, 9)]);
  expect(asked.pendingCommand).toBeNull();
  expect(asked.asking).toEqual({ kind: "divide", target: { handle: 9, chain: [] } });
  expect(asked.hint).toBe("inquiry.askCount");
  const done = toolReducer(asked, { type: "answer", value: 5 });
  expect(done.pendingCommand).toEqual({ type: "divide_along", handle: 9, chain: [], count: 5 });
  expect(done.asking).toBeNull();
  // Nothing typed yet, or a bad number: still waiting.
  expect(toolReducer(asked, { type: "answer", value: 0 }).asking).not.toBeNull();
  expect(toolReducer(asked, { type: "escape" }).asking).toBeNull();
  // A click on nothing lets go of the entity.
  expect(toolReducer(asked, click([50, 50], 1000, null)).asking).toBeNull();
});

test("measure along waits for the step, given in drawing units", () => {
  const asked = run("measureAlong", [click([5, 0], 0, 9)]);
  expect(asked.hint).toBe("inquiry.askStep");
  const done = toolReducer(asked, { type: "answer", value: 2.5 });
  expect(done.pendingCommand).toEqual({ type: "measure_along", handle: 9, chain: [], step: 2.5 });
});

test("ID point names the point clicked and the snap that found it", () => {
  const onEnd: ToolEvent = {
    type: "click",
    world: [1, 2],
    screen: [10, 20],
    time: 0,
    picked: null,
    info: null,
    snapped: true,
    snapKind: "end",
  };
  const snapped = run("idPoint", [onEnd]);
  expect(snapped.pendingCommand).toEqual({ type: "id_point", p: [1, 2], kind: "end" });
  const loose = run("idPoint", [click([1.25, 2], 0)]);
  expect(loose.pendingCommand).toEqual({ type: "id_point", p: [1.25, 2], kind: null });
  expect(initialToolState("idPoint").hint).toBe("inquiry.clickPoint");
});

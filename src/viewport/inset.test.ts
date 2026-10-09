import { expect, test } from "vitest";
import type { PaperView } from "../api";
import { worldToScreen, type Camera } from "./camera";
import { scissorBox, viewCamera } from "./inset";

const view: PaperView = {
  handle: 7,
  rect: [
    [10, 10],
    [110, 90],
  ],
  centre: [500, 300],
  scale: 0.1,
  lines: null,
  fills: null,
  texts: null,
};
/** A layout 200 units across on a 400 x 300 canvas. */
const paper: Camera = { cx: 100, cy: 75, scale: 0.5, width: 400, height: 300 };

test("model space sits in the frame: its centre on the frame's, scaled", () => {
  const model = viewCamera(paper, view);
  const at = worldToScreen(model, [500, 300]);
  const frame = worldToScreen(paper, [60, 50]);
  expect(at[0]).toBeCloseTo(frame[0], 9);
  expect(at[1]).toBeCloseTo(frame[1], 9);
  // 10 model units are 1 layout unit.
  const right = worldToScreen(model, [510, 300]);
  const oneOver = worldToScreen(paper, [61, 50]);
  expect(right[0]).toBeCloseTo(oneOver[0], 9);
});

test("the frame clips in device pixels from the bottom, and off screen it is null", () => {
  // Device pixels at a ratio of 2.
  expect(scissorBox(paper, view, 800, 600)).toEqual([40, 40, 400, 320]);
  expect(scissorBox({ ...paper, cx: 10_000 }, view, 800, 600)).toBeNull();
});

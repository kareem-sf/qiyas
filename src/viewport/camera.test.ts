import { expect, test } from "vitest";
import { fitBounds, resized, visibleBounds, screenToWorld, worldToScreen, zoomAt, type Camera } from "./camera";

test("round trip world/screen", () => {
  const c: Camera = { cx: 0, cy: 0, scale: 0.5, width: 800, height: 600 };
  const p: [number, number] = [12.5, -3];
  expect(screenToWorld(c, worldToScreen(c, p))).toEqual([12.5, -3]);
});

test("zoomAt keeps the point under the cursor", () => {
  const c: Camera = { cx: 3, cy: -7, scale: 0.25, width: 800, height: 600 };
  const before = screenToWorld(c, [100, 100]);
  const zoomed = zoomAt(c, [100, 100], 0.5);
  const after = screenToWorld(zoomed, [100, 100]);
  expect(zoomed.scale).toBeCloseTo(0.125, 12);
  expect(Math.abs(after[0] - before[0])).toBeLessThan(1e-9);
  expect(Math.abs(after[1] - before[1])).toBeLessThan(1e-9);
});

test("fitBounds centres and fits the longer side", () => {
  const c: Camera = { cx: 0, cy: 0, scale: 1, width: 800, height: 600 };
  const f = fitBounds(
    c,
    [
      [0, 0],
      [40, 10],
    ],
    20,
  );
  expect(f.cx).toBe(20);
  expect(f.cy).toBe(5);
  expect(f.scale).toBe(40 / (800 - 40));
});

test("a narrower canvas keeps the whole view, and widening again restores it", () => {
  const wide: Camera = { cx: 50, cy: 20, scale: 0.1, width: 1000, height: 600 };
  // What the user last looked at: 100 x 60 world units round (50, 20).
  const view = visibleBounds(wide);
  const narrow = resized(wide, 720, 600, view);
  expect(narrow.width).toBe(720);
  expect([narrow.cx, narrow.cy]).toEqual([50, 20]);
  // Nothing on either side is cut off.
  expect(screenToWorld(narrow, [0, 0])[0]).toBeLessThanOrEqual(0 + 1e-9);
  expect(screenToWorld(narrow, [720, 0])[0]).toBeGreaterThanOrEqual(100 - 1e-9);
  const back = resized(narrow, 1000, 600, view);
  expect(back.scale).toBeCloseTo(0.1, 12);
  expect([back.cx, back.cy]).toEqual([50, 20]);
});

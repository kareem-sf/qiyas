import { expect, test } from "vitest";
import { fitBounds, screenToWorld, worldToScreen, zoomAt, type Camera } from "./camera";

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

import { expect, test } from "vitest";
import type { SnapKind } from "../api";
import { MARKER_PX, MARKER_SHAPE, markerSegments } from "./snapMarker";

test("each snap kind has its own shape", () => {
  expect(MARKER_SHAPE).toEqual({
    end: "square",
    mid: "triangle",
    centre: "circle",
    intersection: "cross",
    nearest: "hourglass",
    perpendicular: "perpendicular",
    node: "diamond",
  });
  expect(new Set(Object.values(MARKER_SHAPE)).size).toBe(7);
});

test("markers keep their size on screen and sit around the point", () => {
  const kinds = Object.keys(MARKER_SHAPE) as SnapKind[];
  for (const kind of kinds) {
    for (const wpp of [0.01, 2]) {
      const segs = markerSegments([100, -50], kind, wpp);
      expect(segs.length % 4).toBe(0);
      expect(segs.length).toBeGreaterThan(0);
      const r = MARKER_PX * wpp;
      for (let i = 0; i < segs.length; i += 2) {
        expect(Math.abs(segs[i] - 100)).toBeLessThanOrEqual(r + 1e-9);
        expect(Math.abs(segs[i + 1] + 50)).toBeLessThanOrEqual(r + 1e-9);
      }
    }
  }
  // An end's square: four sides.
  expect(markerSegments([0, 0], "end", 1).length).toBe(16);
  // An intersection's cross: two strokes.
  expect(markerSegments([0, 0], "intersection", 1).length).toBe(8);
});

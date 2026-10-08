import { expect, test } from "vitest";
import type { Measurement } from "../api";
import type { Batch } from "../viewport/batch";
import { measurementBounds, overlaySegments } from "./overlay";

test("distance overlay follows the points to the cursor", () => {
  const s = overlaySegments(
    [
      [0, 0],
      [4, 0],
    ],
    [4, 3],
    "distance",
  );
  expect(Array.from(s)).toEqual([0, 0, 4, 0, 4, 0, 4, 3]);
  expect(overlaySegments([], [4, 3], "distance")).toHaveLength(0);
});

test("area overlay closes the ring", () => {
  const triangle: [number, number][] = [
    [0, 0],
    [4, 0],
    [4, 3],
  ];
  expect(Array.from(overlaySegments(triangle, null, "area"))).toEqual([
    0, 0, 4, 0, 4, 0, 4, 3, 4, 3, 0, 0,
  ]);
  // With the cursor the ring closes from the cursor; coordinates stay absolute and exact.
  const far = overlaySegments(
    [
      [500000.125, 0],
      [500004.125, 0],
    ],
    [500004.125, 3],
    "area",
  );
  expect(Array.from(far)).toEqual([
    500000.125, 0, 500004.125, 0, 500004.125, 0, 500004.125, 3, 500004.125, 3, 500000.125, 0,
  ]);
});

test("bounds come from the points, else from the handles drawn in the batch", () => {
  const m = { points: [[1, 2], [5, -1]], handles: [] } as unknown as Measurement;
  expect(measurementBounds(m, null)).toEqual([
    [1, -1],
    [5, 2],
  ]);

  const batch = {
    origin: [100, 100],
    segments: new Float32Array([0, 0, 1, 0, 2, 2, 3, 5]),
    ranges: [
      { handle: 7n, first: 0, count: 1 },
      { handle: 8n, first: 1, count: 1 },
    ],
  } as unknown as Batch;
  const counted = { points: [], handles: [8] } as unknown as Measurement;
  expect(measurementBounds(counted, batch)).toEqual([
    [102, 102],
    [103, 105],
  ]);
  expect(measurementBounds({ points: [], handles: [9] } as unknown as Measurement, batch)).toBeNull();
});

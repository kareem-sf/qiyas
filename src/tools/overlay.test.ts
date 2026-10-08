import { expect, test } from "vitest";
import type { Measurement } from "../api";
import { NO_CHAIN, Ranges, type Batch, type Range } from "../viewport/batch";
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
  const m = { points: [[1, 2], [5, -1]], handles: [], chain: [] } as unknown as Measurement;
  expect(measurementBounds(m, null)).toEqual([
    [1, -1],
    [5, 2],
  ]);

  // Line 7 on the sheet; line 9 inside block reference 8, drawn twice (two segments).
  const batch = {
    origin: [100, 100],
    segments: new Float32Array([0, 0, 1, 0, 2, 2, 3, 5, 4, 4, 6, 4]),
    ranges: rangesOf([
      { top: 7n, entity: 7n, chain: NO_CHAIN, first: 0, count: 1 },
      { top: 8n, entity: 9n, chain: 0, first: 1, count: 2 },
    ]),
    chains: [[8n]],
  } as unknown as Batch;
  const counted = { points: [], handles: [8], chain: [] } as unknown as Measurement;
  expect(measurementBounds(counted, batch)).toEqual([
    [102, 102],
    [106, 105],
  ]);
  const inner = { points: [], handles: [9], chain: [8] } as unknown as Measurement;
  expect(measurementBounds(inner, batch)).toEqual([
    [102, 102],
    [106, 105],
  ]);
  const missing = { points: [], handles: [9], chain: [] } as unknown as Measurement;
  expect(measurementBounds(missing, batch)).toBeNull();
});

/** `list` laid out as the ranges section of a batch. */
function rangesOf(list: Range[]): Ranges {
  const buf = new ArrayBuffer(32 * list.length);
  const v = new DataView(buf);
  list.forEach((r, i) => {
    v.setBigUint64(32 * i, r.top, true);
    v.setBigUint64(32 * i + 8, r.entity, true);
    v.setUint32(32 * i + 16, r.chain, true);
    v.setUint32(32 * i + 20, r.first, true);
    v.setUint32(32 * i + 24, r.count, true);
  });
  return new Ranges(new BigUint64Array(buf), new Uint32Array(buf));
}

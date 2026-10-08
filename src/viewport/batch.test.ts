import { expect, test } from "vitest";
import { parseBatch } from "./batch";

test("parseBatch reads the header", () => {
  // One segment, one range: 56 + 28 + 16 = 100 bytes, layout of Batch::to_bytes.
  const buf = new ArrayBuffer(100);
  const v = new DataView(buf);
  v.setUint32(0, 1, true);
  v.setUint32(4, 1, true);
  [100, 200, 104, 203].forEach((x, i) => v.setFloat64(8 + 8 * i, x, true));
  v.setFloat64(40, 102, true);
  v.setFloat64(48, 201.5, true);
  v.setBigUint64(56, 7n, true);
  v.setBigUint64(64, 7n, true);
  v.setUint32(72, 0, true);
  v.setUint32(76, 1, true);
  [-2, -1.5, 2, 1.5].forEach((x, i) => v.setFloat32(80 + 4 * i, x, true));
  v.setUint32(96, 0xff0000, true);

  const b = parseBatch(buf);
  expect(b.segments.length).toBe(4);
  expect(Array.from(b.segments)).toEqual([-2, -1.5, 2, 1.5]);
  expect(b.handles[0]).toBe(7n);
  expect(b.ranges).toEqual([{ handle: 7n, first: 0, count: 1 }]);
  expect(b.colors[0]).toBe(0xff0000);
  expect(b.bounds).toEqual([
    [100, 200],
    [104, 203],
  ]);
  expect(b.origin).toEqual([102, 201.5]);
});

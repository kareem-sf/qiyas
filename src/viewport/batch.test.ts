import { expect, test } from "vitest";
import { NO_CHAIN, parseBatch, rangeMatcher } from "./batch";

/** A version 2 batch, laid out as `Batch::to_bytes` does: two segments, the first of a line at the
 *  top level (handle 7), the second of a line (9) inside block reference 8. `extra` adds a section
 *  of an unknown kind before the known ones. */
function v2(extra = false): ArrayBuffer {
  const sections: [number, number][] = [
    ...(extra ? ([[99, 8]] as [number, number][]) : []),
    [1, 16], // handles
    [2, 16], // entity handles
    [3, 64], // ranges
    [4, 12], // chains: one chain of one handle
    [5, 32], // segments
    [6, 8], // colours
  ];
  const align = (n: number) => Math.ceil(n / 8) * 8;
  let at = align(60 + 12 * sections.length);
  const placed = sections.map(([kind, length]) => {
    const entry = { kind, at, length };
    at = align(at + length);
    return entry;
  });
  const buf = new ArrayBuffer(at);
  const v = new DataView(buf);
  v.setUint32(0, 0x51495942, true);
  v.setUint32(4, 2, true);
  [100, 200, 104, 203].forEach((x, i) => v.setFloat64(8 + 8 * i, x, true));
  v.setFloat64(40, 102, true);
  v.setFloat64(48, 201.5, true);
  v.setUint32(56, placed.length, true);
  placed.forEach(({ kind, at, length }, i) => {
    v.setUint32(60 + 12 * i, kind, true);
    v.setUint32(64 + 12 * i, at, true);
    v.setUint32(68 + 12 * i, length, true);
  });
  const of = (kind: number) => placed.find((p) => p.kind === kind)!.at;
  v.setBigUint64(of(1), 7n, true);
  v.setBigUint64(of(1) + 8, 8n, true);
  v.setBigUint64(of(2), 7n, true);
  v.setBigUint64(of(2) + 8, 9n, true);
  const ranges = of(3);
  [
    [7n, 7n, NO_CHAIN, 0],
    [8n, 9n, 0, 1],
  ].forEach(([top, entity, chain, first], i) => {
    const r = ranges + 32 * i;
    v.setBigUint64(r, top as bigint, true);
    v.setBigUint64(r + 8, entity as bigint, true);
    v.setUint32(r + 16, chain as number, true);
    v.setUint32(r + 20, first as number, true);
    v.setUint32(r + 24, 1, true);
  });
  v.setUint32(of(4), 1, true);
  v.setBigUint64(of(4) + 4, 8n, true);
  [-2, -1.5, 2, 1.5, 0, 0, 1, 1].forEach((x, i) => v.setFloat32(of(5) + 4 * i, x, true));
  v.setUint32(of(6), 0xff0000, true);
  v.setUint32(of(6) + 4, 0x00ff00, true);
  return buf;
}

test("parseBatch reads the header and the section table", () => {
  for (const extra of [false, true]) {
    const b = parseBatch(v2(extra));
    expect(Array.from(b.segments)).toEqual([-2, -1.5, 2, 1.5, 0, 0, 1, 1]);
    expect(Array.from(b.handles)).toEqual([7n, 8n]);
    expect(Array.from(b.entityHandles)).toEqual([7n, 9n]);
    expect(b.ranges.length).toBe(2);
    expect([b.ranges.at(0), b.ranges.at(1)]).toEqual([
      { top: 7n, entity: 7n, chain: NO_CHAIN, first: 0, count: 1 },
      { top: 8n, entity: 9n, chain: 0, first: 1, count: 1 },
    ]);
    expect(b.chains).toEqual([[8n]]);
    expect(Array.from(b.colors)).toEqual([0xff0000, 0x00ff00]);
    expect(b.bounds).toEqual([
      [100, 200],
      [104, 203],
    ]);
    expect(b.origin).toEqual([102, 201.5]);
  }
});

test("parseBatch rejects other versions and other data", () => {
  const old = v2();
  new DataView(old).setUint32(4, 1, true);
  expect(() => parseBatch(old)).toThrow(/version 1/);
  // A version 1 batch started with the segment count, not the magic.
  const v1 = new ArrayBuffer(100);
  new DataView(v1).setUint32(0, 1, true);
  expect(() => parseBatch(v1)).toThrow(/not in the Qiyas batch format/);
});

test("rangeMatcher tells a whole block from an entity inside it", () => {
  const b = parseBatch(v2());
  const [line, leaf] = [0, 1];
  const whole = rangeMatcher(b, [{ handle: 8, chain: [] }]);
  expect([whole(line), whole(leaf)]).toEqual([false, true]);
  const inner = rangeMatcher(b, [{ handle: 9, chain: [8] }]);
  expect([inner(line), inner(leaf)]).toEqual([false, true]);
  // The same entity through another chain is another drawn thing.
  expect(rangeMatcher(b, [{ handle: 9, chain: [5] }])(leaf)).toBe(false);
  expect(rangeMatcher(b, [{ handle: 7, chain: [] }])(line)).toBe(true);
});

/** The bytes `Batch::to_bytes` writes for the batch of `v2()` (crates/qiyas-render/tests/batch.rs,
 *  `bytes_match_the_frontend_fixture`). */
const FROM_RUST = [
  "4259495102000000000000000000594000000000000069400000000000005a400000000000606940",
  "00000000008059400000000000306940060000000100000088000000100000000200000098000000",
  "1000000003000000a80000004000000004000000e80000000c00000005000000f800000020000000",
  "06000000180100000800000000000000070000000000000008000000000000000700000000000000",
  "090000000000000007000000000000000700000000000000ffffffff000000000100000000000000",
  "08000000000000000900000000000000000000000100000001000000000000000100000008000000",
  "0000000000000000000000c00000c0bf000000400000c03f00000000000000000000803f0000803f",
  "0000ff0000ff0000",
].join("");

test("parseBatch reads what the Rust side writes", () => {
  const bytes = new Uint8Array(FROM_RUST.length / 2);
  for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(FROM_RUST.slice(2 * i, 2 * i + 2), 16);
  expect(parseBatch(bytes.buffer)).toEqual(parseBatch(v2()));
});

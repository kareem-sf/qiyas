import { expect, test } from "vitest";
import { NO_CHAIN, parseBatch, rangeMatcher } from "./batch";

/** A version 2 batch, laid out as `Batch::to_bytes` does: two segments, the first of a line at the
 *  top level (handle 7), the second of a line (9) inside block reference 8. `extra` adds a section
 *  of an unknown kind before the known ones; `without` leaves a section out. */
function v2(extra = false, without = 0): ArrayBuffer {
  const sections: [number, number][] = [
    ...(extra ? ([[99, 8]] as [number, number][]) : []),
    [3, 64], // ranges
    [4, 12], // chains: one chain of one handle
    [5, 32], // segments
    [6, 8], // colours
  ].filter(([kind]) => kind !== without) as [number, number][];
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
  // A left-out section's data goes nowhere (offset past the end is never written).
  const of = (kind: number) => placed.find((p) => p.kind === kind)?.at ?? -1;
  if (without !== 0) return buf;
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

test("parseBatch rejects a missing section or a cut-short buffer", () => {
  for (const kind of [3, 4, 5, 6]) {
    expect(() => parseBatch(v2(false, kind))).toThrow(`missing section ${kind}`);
  }
  const whole = v2();
  // Cut inside the colours: their section no longer fits.
  expect(() => parseBatch(whole.slice(0, whole.byteLength - 4))).toThrow("section 6 too short");
  // Cut inside the section table.
  expect(() => parseBatch(whole.slice(0, 70))).toThrow("section table too short");
  // Colours for one segment only.
  const short = v2();
  const sv = new DataView(short);
  const colorsEntry = 60 + 12 * 3;
  expect(sv.getUint32(colorsEntry, true)).toBe(6);
  sv.setUint32(colorsEntry + 8, 4, true);
  expect(() => parseBatch(short)).toThrow("section 6 too short");
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
  "000000000080594000000000003069400400000003000000700000004000000004000000b0000000",
  "0c00000005000000c00000002000000006000000e000000008000000000000000700000000000000",
  "0700000000000000ffffffff00000000010000000000000008000000000000000900000000000000",
  "0000000001000000010000000000000001000000080000000000000000000000000000c00000c0bf",
  "000000400000c03f00000000000000000000803f0000803f0000ff0000ff0000",
].join("");

test("parseBatch reads what the Rust side writes", () => {
  const bytes = new Uint8Array(FROM_RUST.length / 2);
  for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(FROM_RUST.slice(2 * i, 2 * i + 2), 16);
  expect(parseBatch(bytes.buffer)).toEqual(parseBatch(v2()));
});

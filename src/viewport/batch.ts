/** Line segments of one sheet, decoded from `render_batch` (`Batch::to_bytes` in qiyas-render). */

/** The segments of one drawn entity. Inside blocks, `top` is the outermost block reference on the
 *  sheet, `entity` the entity itself and `chain` an index into `Batch.chains`; at the top level
 *  `top === entity` and `chain === NO_CHAIN`. */
export type Range = { top: bigint; entity: bigint; chain: number; first: number; count: number };

/** The ranges section read in place: a big drawing has hundreds of thousands of ranges, too many
 *  to turn into objects. Range `i` is the 32-byte record `i`. */
export class Ranges {
  /** `wide` views the section as u64s (4 per record), `narrow` as u32s (8 per record). */
  constructor(
    private readonly wide: BigUint64Array,
    private readonly narrow: Uint32Array,
  ) {}

  get length() {
    return this.wide.length / 4;
  }
  top(i: number) {
    return this.wide[4 * i];
  }
  entity(i: number) {
    return this.wide[4 * i + 1];
  }
  chain(i: number) {
    return this.narrow[8 * i + 4];
  }
  first(i: number) {
    return this.narrow[8 * i + 5];
  }
  count(i: number) {
    return this.narrow[8 * i + 6];
  }
  /** Range `i` as an object, for tests and debugging. */
  at(i: number): Range {
    const [top, entity, chain, first, count] = [
      this.top(i),
      this.entity(i),
      this.chain(i),
      this.first(i),
      this.count(i),
    ];
    return { top, entity, chain, first, count };
  }
}

export type Batch = {
  /** `x0 y0 x1 y1` per segment, relative to `origin`. */
  segments: Float32Array;
  /** `0xRRGGBB` per segment. */
  colors: Uint32Array;
  /** One per drawn entity, in segment order; every segment is in exactly one. */
  ranges: Ranges;
  /** Block-reference handles leading to nested entities, outermost first. */
  chains: bigint[][];
  /** `[[min x, min y], [max x, max y]]` in absolute drawing coordinates; zeros when empty. */
  bounds: [[number, number], [number, number]];
  /** Absolute drawing point that `segments` are relative to. */
  origin: [number, number];
};

/** Something drawn, to highlight or frame: an entity on the sheet by its handle (`chain` empty), or
 *  an entity inside blocks by its own handle and the block references leading to it. */
export type Target = { handle: number; chain: number[] };

/** "BYIQ": `0x51495942` read little-endian. */
const MAGIC = 0x51495942;
const VERSION = 2;
export const NO_CHAIN = 0xffffffff;
const SECTION = { ranges: 3, chains: 4, segments: 5, colors: 6 };

/**
 * Decodes layout version 2 documented on `Batch::to_bytes` (crates/qiyas-render/src/batch.rs): a
 * header, then a table of sections found by kind; unknown kinds are skipped. The typed views share
 * `buf` (every section is 8-byte aligned) and read in platform byte order, which is little-endian on
 * every desktop platform Qiyas targets.
 */
export function parseBatch(buf: ArrayBuffer): Batch {
  const v = new DataView(buf);
  if (buf.byteLength < 60 || v.getUint32(0, true) !== MAGIC) {
    throw new Error("Render batch is not in the Qiyas batch format");
  }
  const version = v.getUint32(4, true);
  if (version !== VERSION) {
    throw new Error(`Render batch is layout version ${version}; this window reads version ${VERSION}`);
  }
  const f = (i: number) => v.getFloat64(8 + 8 * i, true);
  const k = v.getUint32(56, true);
  if (60 + 12 * k > buf.byteLength) throw new Error("Render batch section table too short");
  const sections = new Map<number, { at: number; length: number }>();
  for (let i = 0; i < k; i++) {
    const entry = 60 + 12 * i;
    const kind = v.getUint32(entry, true);
    const at = v.getUint32(entry + 4, true);
    const length = v.getUint32(entry + 8, true);
    if (at + length > buf.byteLength) throw new Error(`Render batch section ${kind} too short`);
    sections.set(kind, { at, length });
  }
  /** A required section; its length must be a whole number of `unit`-byte records. */
  const section = (kind: number, unit = 1) => {
    const found = sections.get(kind);
    if (!found) throw new Error(`Render batch missing section ${kind}`);
    if (found.length % unit !== 0) throw new Error(`Render batch section ${kind} too short`);
    return found;
  };

  const segments = section(SECTION.segments, 16);
  const n = segments.length / 16;
  const colors = section(SECTION.colors, 4);
  if (colors.length !== 4 * n) throw new Error(`Render batch section ${SECTION.colors} too short`);

  const c = section(SECTION.chains);
  const chains: bigint[][] = [];
  for (let at = c.at; at < c.at + c.length; ) {
    const m = at + 4 <= c.at + c.length ? v.getUint32(at, true) : -1;
    if (m < 0 || at + 4 + 8 * m > c.at + c.length) {
      throw new Error(`Render batch section ${SECTION.chains} too short`);
    }
    const chain = [];
    for (let j = 0; j < m; j++) chain.push(v.getBigUint64(at + 4 + 8 * j, true));
    chains.push(chain);
    at += 4 + 8 * m;
  }

  const r = section(SECTION.ranges, 32);
  const ranges = new Ranges(
    new BigUint64Array(buf, r.at, r.length / 8),
    new Uint32Array(buf, r.at, r.length / 4),
  );
  // The ranges cover the segments, and name chains that exist.
  let covered = 0;
  for (let i = 0; i < ranges.length; i++) {
    const chain = ranges.chain(i);
    if (ranges.first(i) !== covered || (chain !== NO_CHAIN && chain >= chains.length)) {
      throw new Error(`Render batch section ${SECTION.ranges} does not match the segments`);
    }
    covered += ranges.count(i);
  }
  if (covered !== n) throw new Error(`Render batch section ${SECTION.ranges} too short`);

  return {
    bounds: [
      [f(0), f(1)],
      [f(2), f(3)],
    ],
    origin: [f(4), f(5)],
    ranges,
    chains,
    segments: new Float32Array(buf, segments.at, 4 * n),
    colors: new Uint32Array(buf, colors.at, n),
  };
}

/** A test for "is range `i` of `batch` one of `targets`": a target with no chain matches its whole
 *  top-level entity; one with a chain matches only that entity reached through that chain. */
export function rangeMatcher(batch: Batch, targets: Target[]): (i: number) => boolean {
  const tops = new Set<bigint>();
  const inner = new Set<string>();
  for (const t of targets) {
    if (t.chain.length === 0) tops.add(BigInt(t.handle));
    else inner.add(`${t.handle}@${t.chain.join(",")}`);
  }
  const { ranges, chains } = batch;
  return (i) => {
    if (tops.has(ranges.top(i))) return true;
    const chain = ranges.chain(i);
    if (inner.size === 0 || chain === NO_CHAIN) return false;
    return inner.has(`${ranges.entity(i)}@${(chains[chain] ?? []).join(",")}`);
  };
}

/** The segments `[first, first + count)` of every range of `batch` that matches one of `targets`. */
export function* matchingRanges(batch: Batch, targets: Target[]): Generator<[number, number]> {
  if (targets.length === 0) return;
  const matches = rangeMatcher(batch, targets);
  const { ranges } = batch;
  for (let i = 0; i < ranges.length; i++) {
    if (matches(i)) yield [ranges.first(i), ranges.count(i)];
  }
}

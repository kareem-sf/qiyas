/** Line segments and text glyph quads of one sheet, decoded from `render_batch` (`Batch::to_bytes`
 *  in qiyas-render), and the glyph atlas pages from `atlas_pages` (`Atlas::to_bytes`). */

/** The segments (or glyph quads) of one drawn entity. Inside blocks, `top` is the outermost block
 *  reference on the sheet, `entity` the entity itself and `chain` an index into `Batch.chains`; at
 *  the top level `top === entity` and `chain === NO_CHAIN`. `flags` bit 0 (`TEXT_BOX`): a text's
 *  box, picked and highlighted but not drawn. */
export type Range = {
  top: bigint;
  entity: bigint;
  chain: number;
  first: number;
  count: number;
  flags: number;
};

/** `Range.flags`: the segments are a text's box (its glyph quads draw it). */
export const TEXT_BOX = 1;

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
  flags(i: number) {
    return this.narrow[8 * i + 7];
  }
  /** Range `i` as an object, for tests and debugging. */
  at(i: number): Range {
    const [top, entity, chain, first, count, flags] = [
      this.top(i),
      this.entity(i),
      this.chain(i),
      this.first(i),
      this.count(i),
      this.flags(i),
    ];
    return { top, entity, chain, first, count, flags };
  }
}

/** One glyph quad: corners relative to the batch origin (top-left, top-right, bottom-right,
 *  bottom-left of the glyph), texel rectangle `u0 v0 u1 v1` on atlas page `page`. */
export type Quad = { corners: number[]; uv: number[]; color: number; page: number; flags: number };

/** `Quad.flags`: the text sits inside a block. */
export const QUAD_IN_BLOCK = 1;

/** The glyph quads section read in place; quad `i` is the 48-byte record `i`. */
export class Quads {
  /** The same bytes as f32 (12 per record), u16 (24) and u32 (12). */
  constructor(
    readonly f32: Float32Array,
    readonly u16: Uint16Array,
    readonly u32: Uint32Array,
  ) {}

  get length() {
    return this.f32.length / 12;
  }
  /** Corner `k` (0 to 3) of quad `i`: x, y relative to the batch origin. */
  x(i: number, k: number) {
    return this.f32[12 * i + 2 * k];
  }
  y(i: number, k: number) {
    return this.f32[12 * i + 2 * k + 1];
  }
  /** `u0 v0 u1 v1` for `k` 0 to 3. */
  uv(i: number, k: number) {
    return this.u16[24 * i + 16 + k];
  }
  color(i: number) {
    return this.u32[12 * i + 10];
  }
  page(i: number) {
    return this.u16[24 * i + 22];
  }
  flags(i: number) {
    return this.u16[24 * i + 23];
  }
  at(i: number): Quad {
    return {
      corners: Array.from(this.f32.subarray(12 * i, 12 * i + 8)),
      uv: [0, 1, 2, 3].map((k) => this.uv(i, k)),
      color: this.color(i),
      page: this.page(i),
      flags: this.flags(i),
    };
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
  /** Absolute drawing point that `segments` and `quads` are relative to. */
  origin: [number, number];
  /** Glyph quads of the texts. */
  quads: Quads;
  /** One per drawn text, over `quads`, in quad order. */
  textRanges: Ranges;
  /** The atlas version and page count the quads need (see `parseAtlas`). */
  atlasVersion: number;
  atlasPages: number;
  /** The zoom the glyphs were drawn for, in screen pixels per drawing unit; 0 without texts. */
  pxPerUnit: number;
};

/** Something drawn, to highlight or frame: an entity on the sheet by its handle (`chain` empty), or
 *  an entity inside blocks by its own handle and the block references leading to it. */
export type Target = { handle: number; chain: number[] };

/** "BYIQ": `0x51495942` read little-endian. */
const MAGIC = 0x51495942;
const VERSION = 2;
export const NO_CHAIN = 0xffffffff;
const SECTION = {
  ranges: 3,
  chains: 4,
  segments: 5,
  colors: 6,
  quads: 7,
  atlas: 8,
  textRanges: 9,
};

/**
 * Decodes layout version 2 documented on `Batch::to_bytes` (crates/qiyas-render/src/batch.rs): a
 * header, then a table of sections found by kind; unknown kinds are skipped, and the text sections
 * (7, 8, 9) may be left out (no texts). The typed views share `buf` (every section is 8-byte
 * aligned) and read in platform byte order, which is little-endian on every desktop platform Qiyas
 * targets.
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

  /** The ranges section `kind`; its ranges must cover `n` records in order and name chains that
   *  exist. */
  const readRanges = (kind: number, found: { at: number; length: number }, n: number) => {
    const ranges = new Ranges(
      new BigUint64Array(buf, found.at, found.length / 8),
      new Uint32Array(buf, found.at, found.length / 4),
    );
    let covered = 0;
    for (let i = 0; i < ranges.length; i++) {
      const chain = ranges.chain(i);
      if (ranges.first(i) !== covered || (chain !== NO_CHAIN && chain >= chains.length)) {
        const what = kind === SECTION.ranges ? "segments" : "quads";
        throw new Error(`Render batch section ${kind} does not match the ${what}`);
      }
      covered += ranges.count(i);
    }
    if (covered !== n) throw new Error(`Render batch section ${kind} too short`);
    return ranges;
  };
  const ranges = readRanges(SECTION.ranges, section(SECTION.ranges, 32), n);

  // The text sections: absent means no texts.
  const empty = { at: 0, length: 0 };
  const q = sections.has(SECTION.quads) ? section(SECTION.quads, 48) : empty;
  const quads = new Quads(
    new Float32Array(buf, q.at, q.length / 4),
    new Uint16Array(buf, q.at, q.length / 2),
    new Uint32Array(buf, q.at, q.length / 4),
  );
  const t = sections.has(SECTION.textRanges) ? section(SECTION.textRanges, 32) : empty;
  const textRanges = readRanges(SECTION.textRanges, t, quads.length);
  let [atlasVersion, atlasPages, pxPerUnit] = [0, 0, 0];
  const a = sections.get(SECTION.atlas);
  if (a) {
    if (a.length < 24) throw new Error(`Render batch section ${SECTION.atlas} too short`);
    atlasVersion = Number(v.getBigUint64(a.at, true));
    atlasPages = v.getUint32(a.at + 8, true);
    pxPerUnit = v.getFloat64(a.at + 16, true);
  }

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
    quads,
    textRanges,
    atlasVersion,
    atlasPages,
    pxPerUnit,
  };
}

/** One glyph atlas page: `width` x `height` coverage bytes (R8), rows from the top. */
export type AtlasPage = { index: number; width: number; height: number; data: Uint8Array };

/** The atlas pages changed since the version asked for, and the atlas version they bring. */
export type AtlasUpdate = { version: number; pages: AtlasPage[] };

/** Decodes `atlas_pages` (`Atlas::to_bytes` in qiyas-render): `u64` version, `u32` page count, then
 *  per page `u32` index, width, height and byte length, and the bytes. Little-endian. */
export function parseAtlas(buf: ArrayBuffer): AtlasUpdate {
  const v = new DataView(buf);
  if (buf.byteLength < 12) throw new Error("Atlas pages too short");
  const version = Number(v.getBigUint64(0, true));
  const count = v.getUint32(8, true);
  const pages: AtlasPage[] = [];
  let at = 12;
  for (let i = 0; i < count; i++) {
    if (at + 16 > buf.byteLength) throw new Error("Atlas pages too short");
    const [index, width, height, length] = [0, 4, 8, 12].map((o) => v.getUint32(at + o, true));
    if (length !== width * height || at + 16 + length > buf.byteLength) {
      throw new Error("Atlas pages too short");
    }
    pages.push({ index, width, height, data: new Uint8Array(buf, at + 16, length) });
    at += 16 + length;
  }
  return { version, pages };
}

/** Whether glyphs drawn for `drawnFor` screen pixels per drawing unit look poor at `now`: more than
 *  twice as big or small. */
export function glyphsNeedRedraw(drawnFor: number, now: number): boolean {
  if (!(drawnFor > 0) || !(now > 0)) return false;
  const ratio = now / drawnFor;
  return ratio > 2 || ratio < 0.5;
}

/** A test for "is range `i` of `batch` one of `targets`": a target with no chain matches its whole
 *  top-level entity; one with a chain matches only that entity reached through that chain. Over the
 *  segment ranges, or `ranges` (the text ranges). */
export function rangeMatcher(
  batch: Batch,
  targets: Target[],
  ranges: Ranges = batch.ranges,
): (i: number) => boolean {
  const tops = new Set<bigint>();
  const inner = new Set<string>();
  for (const t of targets) {
    if (t.chain.length === 0) tops.add(BigInt(t.handle));
    else inner.add(`${t.handle}@${t.chain.join(",")}`);
  }
  const { chains } = batch;
  return (i) => {
    if (tops.has(ranges.top(i))) return true;
    const chain = ranges.chain(i);
    if (inner.size === 0 || chain === NO_CHAIN) return false;
    return inner.has(`${ranges.entity(i)}@${(chains[chain] ?? []).join(",")}`);
  };
}

/** The segments `[first, first + count)` of every range of `batch` that matches one of `targets`;
 *  with `ranges` the text ranges, the glyph quads. */
export function* matchingRanges(
  batch: Batch,
  targets: Target[],
  ranges: Ranges = batch.ranges,
): Generator<[number, number]> {
  if (targets.length === 0) return;
  const matches = rangeMatcher(batch, targets, ranges);
  for (let i = 0; i < ranges.length; i++) {
    if (matches(i)) yield [ranges.first(i), ranges.count(i)];
  }
}

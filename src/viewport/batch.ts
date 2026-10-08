/** Line segments of one sheet, decoded from `render_batch` (`Batch::to_bytes` in qiyas-render). */
export type Batch = {
  /** `x0 y0 x1 y1` per segment, relative to `origin`. */
  segments: Float32Array;
  /** `0xRRGGBB` per segment. */
  colors: Uint32Array;
  /** Entity handle per segment. */
  handles: BigUint64Array;
  /** Segments of each drawn entity. */
  ranges: Array<{ handle: bigint; first: number; count: number }>;
  /** `[[min x, min y], [max x, max y]]` in absolute drawing coordinates; zeros when empty. */
  bounds: [[number, number], [number, number]];
  /** Absolute drawing point that `segments` are relative to. */
  origin: [number, number];
};

/**
 * Decodes the little-endian layout documented on `Batch::to_bytes`
 * (crates/qiyas-render/src/batch.rs). The views share `buf`; every section is aligned for its type,
 * and TypedArrays read in platform byte order, which is little-endian on every desktop platform Qiyas targets.
 */
export function parseBatch(buf: ArrayBuffer): Batch {
  const v = new DataView(buf);
  const n = v.getUint32(0, true);
  const r = v.getUint32(4, true);
  const f = (i: number) => v.getFloat64(8 + 8 * i, true);
  const expected = 56 + 28 * n + 16 * r;
  if (buf.byteLength !== expected) {
    throw new Error(`Render batch is ${buf.byteLength} bytes, expected ${expected}`);
  }
  const rangesAt = 56 + 8 * n;
  const ranges = [];
  for (let i = 0; i < r; i++) {
    const at = rangesAt + 16 * i;
    ranges.push({
      handle: v.getBigUint64(at, true),
      first: v.getUint32(at + 8, true),
      count: v.getUint32(at + 12, true),
    });
  }
  const segmentsAt = rangesAt + 16 * r;
  return {
    bounds: [
      [f(0), f(1)],
      [f(2), f(3)],
    ],
    origin: [f(4), f(5)],
    handles: new BigUint64Array(buf, 56, n),
    ranges,
    segments: new Float32Array(buf, segmentsAt, 4 * n),
    colors: new Uint32Array(buf, segmentsAt + 16 * n, n),
  };
}

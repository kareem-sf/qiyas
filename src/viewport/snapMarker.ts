/** The glyph drawn where an object snap found its point, in the violet overlay: a square for an
 *  end, a triangle for a middle, a circle for a centre, an × for an intersection, an hourglass for
 *  nearest, ⟂ for perpendicular and ◇ for a node. */
import type { SnapKind } from "../api";
import type { Point } from "./camera";

export type MarkerShape = "square" | "triangle" | "circle" | "cross" | "hourglass" | "perpendicular" | "diamond";

export const MARKER_SHAPE: Record<SnapKind, MarkerShape> = {
  end: "square",
  mid: "triangle",
  centre: "circle",
  intersection: "cross",
  nearest: "hourglass",
  perpendicular: "perpendicular",
  node: "diamond",
};

/** Half the marker's size, in CSS pixels. */
export const MARKER_PX = 6;

/** The outline of each shape as a closed or open run of points, in units of `MARKER_PX`, y up. */
const OUTLINES: Record<MarkerShape, Point[][]> = {
  square: [
    [
      [-1, -1],
      [1, -1],
      [1, 1],
      [-1, 1],
      [-1, -1],
    ],
  ],
  triangle: [
    [
      [-1, -0.8],
      [1, -0.8],
      [0, 1],
      [-1, -0.8],
    ],
  ],
  circle: [
    Array.from({ length: 17 }, (_, i): Point => {
      const a = (i / 16) * 2 * Math.PI;
      return [Math.cos(a), Math.sin(a)];
    }),
  ],
  cross: [
    [
      [-1, -1],
      [1, 1],
    ],
    [
      [-1, 1],
      [1, -1],
    ],
  ],
  hourglass: [
    [
      [-1, 1],
      [1, 1],
      [-1, -1],
      [1, -1],
      [-1, 1],
    ],
  ],
  perpendicular: [
    [
      [-1, -1],
      [1, -1],
    ],
    [
      [0, -1],
      [0, 1],
    ],
  ],
  diamond: [
    [
      [0, 1],
      [1, 0],
      [0, -1],
      [-1, 0],
      [0, 1],
    ],
  ],
};

/** Segments `x0 y0 x1 y1` in drawing coordinates of the marker for `kind` at `p`, `worldPerPixel`
 *  drawing units to a CSS pixel, so it keeps its size on screen at any zoom. */
export function markerSegments(p: Point, kind: SnapKind, worldPerPixel: number): Float64Array {
  const r = MARKER_PX * worldPerPixel;
  const out: number[] = [];
  for (const run of OUTLINES[MARKER_SHAPE[kind]]) {
    for (let i = 1; i < run.length; i++) {
      out.push(
        p[0] + run[i - 1][0] * r,
        p[1] + run[i - 1][1] * r,
        p[0] + run[i][0] * r,
        p[1] + run[i][1] * r,
      );
    }
  }
  return new Float64Array(out);
}

/** Violet line work drawn over the drawing: the rubber band of a measure in progress, and the
 *  points of the selected take-off line. */
import type { Measurement } from "../api";
import type { Batch } from "../viewport/batch";
import type { Bounds, Point } from "../viewport/camera";
import type { Tool } from "../viewport/Viewport";

/**
 * Segments `x0 y0 x1 y1` in absolute drawing coordinates: the polyline through `points`, on to the
 * cursor, and for `area` back to the first point once there are two points or more. Float64, so far
 * coordinates stay exact until the renderer makes them relative to the batch origin.
 */
export function overlaySegments(points: Point[], cursor: Point | null, tool: Tool): Float64Array {
  const path = cursor && points.length > 0 ? [...points, cursor] : [...points];
  if (tool === "area" && points.length >= 2) path.push(points[0]);
  const out = new Float64Array(4 * Math.max(0, path.length - 1));
  for (let i = 1; i < path.length; i++) out.set([...path[i - 1], ...path[i]], 4 * (i - 1));
  return out;
}

/** Where a take-off line sits: around its points, else around its entities as drawn in `batch`.
 *  `null` when neither is on the current batch. */
export function measurementBounds(m: Measurement, batch: Batch | null): Bounds | null {
  const min: Point = [Infinity, Infinity];
  const max: Point = [-Infinity, -Infinity];
  const add = (x: number, y: number) => {
    min[0] = Math.min(min[0], x);
    min[1] = Math.min(min[1], y);
    max[0] = Math.max(max[0], x);
    max[1] = Math.max(max[1], y);
  };
  for (const [x, y] of m.points) add(x, y);
  if (m.points.length === 0 && batch) {
    const handles = new Set(m.handles.map(BigInt));
    const [ox, oy] = batch.origin;
    for (const { handle, first, count } of batch.ranges) {
      if (!handles.has(handle)) continue;
      for (let i = 4 * first; i < 4 * (first + count); i += 2) {
        add(batch.segments[i] + ox, batch.segments[i + 1] + oy);
      }
    }
  }
  return min[0] <= max[0] ? [min, max] : null;
}

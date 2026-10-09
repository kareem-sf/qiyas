/** Ortho, polar tracking and grid snap: where a point goes before it is placed. Pure functions. */
import type { Point } from "../viewport/camera";

export type Track = {
  /** Straight across or up from the previous point (F8). Wins over polar. */
  ortho: boolean;
  /** The direction from the previous point snapped to a multiple of `polarStepDeg` (F10). */
  polar: boolean;
  polarStepDeg: number;
};

export const NO_TRACK: Track = { ortho: false, polar: false, polarStepDeg: 45 };

/** Polar tracking takes hold this close to a multiple of the step, in degrees. */
const POLAR_CATCH_DEG = 5;

/** `p` moved by ortho or polar tracking relative to `prev`; `p` itself with no `prev` or neither on. */
export function constrain(prev: Point | null, p: Point, track: Track): Point {
  if (!prev) return p;
  const [dx, dy] = [p[0] - prev[0], p[1] - prev[1]];
  if (track.ortho) return Math.abs(dx) >= Math.abs(dy) ? [p[0], prev[1]] : [prev[0], p[1]];
  if (track.polar && track.polarStepDeg > 0 && (dx !== 0 || dy !== 0)) {
    const step = (track.polarStepDeg * Math.PI) / 180;
    const angle = Math.atan2(dy, dx);
    const snapped = Math.round(angle / step) * step;
    if (Math.abs(angle - snapped) <= (POLAR_CATCH_DEG * Math.PI) / 180) {
      // Onto the tracking ray: the cursor's distance along it.
      const [ux, uy] = [Math.cos(snapped), Math.sin(snapped)];
      const along = dx * ux + dy * uy;
      return [prev[0] + along * ux, prev[1] + along * uy];
    }
  }
  return p;
}

/** The grid dot nearest `p`, the dots `spacing` apart from (0, 0). */
export function gridRound(p: Point, spacing: number): Point {
  if (!(spacing > 0)) return p;
  return [Math.round(p[0] / spacing) * spacing, Math.round(p[1] / spacing) * spacing];
}

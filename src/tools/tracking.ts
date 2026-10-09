/** Ortho, polar tracking and grid snap: where a point goes before it is placed, along the user
 *  coordinate system when there is one. Pure functions. */
import type { Ucs } from "../api";
import type { Point } from "../viewport/camera";
import { isWorld, toUcs, toWorld } from "./ucs";

export type Track = {
  /** Straight across or up from the previous point (F8). Wins over polar. */
  ortho: boolean;
  /** The direction from the previous point snapped to a multiple of `polarStepDeg` (F10). */
  polar: boolean;
  polarStepDeg: number;
  /** The user coordinate system: ortho and polar angles are taken from its x axis. */
  ucs?: Ucs;
};

export const NO_TRACK: Track = { ortho: false, polar: false, polarStepDeg: 45 };

/** Polar tracking takes hold this close to a multiple of the step, in degrees. */
const POLAR_CATCH_DEG = 5;

/** `p` moved by ortho or polar tracking relative to `prev`; `p` itself with no `prev` or neither on. */
export function constrain(prev: Point | null, p: Point, track: Track): Point {
  if (!prev) return p;
  const ucs = track.ucs;
  if (ucs && !isWorld(ucs)) {
    return toWorld(ucs, constrain(toUcs(ucs, prev), toUcs(ucs, p), { ...track, ucs: undefined }));
  }
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

/** The grid dot nearest `p`, the dots `spacing` apart from (0, 0) of the UCS `ucs` (the
 *  world's when left out). */
export function gridRound(p: Point, spacing: number, ucs?: Ucs): Point {
  if (!(spacing > 0)) return p;
  if (ucs && !isWorld(ucs)) return toWorld(ucs, gridRound(toUcs(ucs, p), spacing));
  return [Math.round(p[0] / spacing) * spacing, Math.round(p[1] / spacing) * spacing];
}

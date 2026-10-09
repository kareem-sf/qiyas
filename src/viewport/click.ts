import type { Point } from "./camera";

/** Movement of this many pixels or more between press and release is a drag, not a click. */
export const CLICK_SLOP_PX = 4;

/** True when a press at `down` and a release at `up` (CSS pixels) make a click. */
export function isClick(down: Point, up: Point): boolean {
  return Math.hypot(up[0] - down[0], up[1] - down[1]) < CLICK_SLOP_PX;
}

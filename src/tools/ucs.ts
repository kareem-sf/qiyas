/** The user coordinate system on the window's side: points in and out of it (`qiyas_core::Ucs`).
 *  The status bar shows the cursor in it, typed points are read in it, and ortho, polar and the
 *  grid run along it. Pure functions. */
import type { Ucs } from "../api";
import type { Point } from "../viewport/camera";

export const WORLD: Ucs = { origin: [0, 0], x_axis: [1, 0] };

/** No UCS, or the world's. */
export const isWorld = (u: Ucs | null | undefined): boolean =>
  !u || (u.origin[0] === 0 && u.origin[1] === 0 && u.x_axis[0] === 1 && u.x_axis[1] === 0);

/** The world point `p` in the UCS `u`. */
export function toUcs(u: Ucs, p: Point): Point {
  const [dx, dy] = [p[0] - u.origin[0], p[1] - u.origin[1]];
  const [cx, sx] = u.x_axis;
  return [dx * cx + dy * sx, -dx * sx + dy * cx];
}

/** The point `p` of the UCS `u` in the world. */
export function toWorld(u: Ucs, p: Point): Point {
  const [cx, sx] = u.x_axis;
  return [u.origin[0] + p[0] * cx - p[1] * sx, u.origin[1] + p[0] * sx + p[1] * cx];
}

/** The direction `d` of the UCS `u` in the world (no origin). */
export function dirToWorld(u: Ucs, d: Point): Point {
  const [cx, sx] = u.x_axis;
  return [d[0] * cx - d[1] * sx, d[0] * sx + d[1] * cx];
}

/** The angle of the UCS's x axis, in degrees. */
export const ucsDegrees = (u: Ucs): number => (Math.atan2(u.x_axis[1], u.x_axis[0]) * 180) / Math.PI;

/** The dot grid's arithmetic, kept apart from WebGL so it can be tested. */

/** Dots closer than this many CSS pixels are not drawn. */
export const GRID_MIN_PX = 8;
/** Every this many dots is a major one. */
export const GRID_MAJOR = 10;

/** Which dots show at `worldPerPixel` drawing units per CSS pixel: the minor dots, `spacing` apart,
 *  and the major ones, ten times that. */
export function gridLevels(spacing: number, worldPerPixel: number): { minor: boolean; major: boolean } {
  if (!(spacing > 0) || !(worldPerPixel > 0)) return { minor: false, major: false };
  const px = spacing / worldPerPixel;
  return { minor: px >= GRID_MIN_PX, major: px * GRID_MAJOR >= GRID_MIN_PX };
}

/** `x` less the nearest multiple of `period`, worked out in f64 so the shader only sees a small
 *  number however far the drawing is from (0, 0). */
export function offsetInPeriod(x: number, period: number): number {
  return x - period * Math.round(x / period);
}

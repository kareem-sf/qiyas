/** Wheel pixels that make one classic mouse-wheel notch (Chrome sends 100 for a notch). */
const NOTCH_PX = 100;
/** The zoom of one notch. */
const NOTCH_FACTOR = 1.1;
/** One event never zooms by more than this, in or out. */
const MAX_FACTOR = 1.5;
/** Pixels in a line and in a page of wheel delta (Firefox sends 3 lines for a notch). */
const LINE_PX = NOTCH_PX / 3;
const PAGE_PX = 800;

/** The zoom factor (above 1 zooms out) for one wheel event: 1.1 for a classic notch, smaller for a
 *  touchpad's small deltas, never beyond 1.5 or 1/1.5 for a single event. `deltaMode` is the
 *  event's: 0 pixels, 1 lines, 2 pages. */
export function wheelFactor(deltaY: number, deltaMode = 0): number {
  const px = deltaY * (deltaMode === 1 ? LINE_PX : deltaMode === 2 ? PAGE_PX : 1);
  const factor = NOTCH_FACTOR ** (px / NOTCH_PX);
  return Math.min(MAX_FACTOR, Math.max(1 / MAX_FACTOR, factor));
}

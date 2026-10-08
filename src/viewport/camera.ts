/**
 * 2D camera over the drawing. Pure functions, each returning a new Camera.
 *
 * World coordinates are absolute drawing coordinates (f64) with y pointing up.
 * Screen coordinates are CSS pixels from the top-left of the canvas with y pointing down.
 */
export type Camera = {
  /** World point at the centre of the canvas. */
  cx: number;
  cy: number;
  /** World units per CSS pixel. */
  scale: number;
  /** Canvas size in CSS pixels. */
  width: number;
  height: number;
};

export type Point = [number, number];
export type Bounds = [Point, Point];

export function worldToScreen(c: Camera, [x, y]: Point): Point {
  return [c.width / 2 + (x - c.cx) / c.scale, c.height / 2 - (y - c.cy) / c.scale];
}

export function screenToWorld(c: Camera, [sx, sy]: Point): Point {
  return [c.cx + (sx - c.width / 2) * c.scale, c.cy - (sy - c.height / 2) * c.scale];
}

/** Multiplies `scale` by `factor` (below 1 zooms in) keeping the world point under `p` still. */
export function zoomAt(c: Camera, p: Point, factor: number): Camera {
  const [wx, wy] = screenToWorld(c, p);
  const scale = c.scale * factor;
  return {
    ...c,
    scale,
    cx: wx - (p[0] - c.width / 2) * scale,
    cy: wy + (p[1] - c.height / 2) * scale,
  };
}

/** Moves the drawing by a screen offset in pixels, as when dragging it. */
export function pan(c: Camera, dxPx: number, dyPx: number): Camera {
  return { ...c, cx: c.cx - dxPx * c.scale, cy: c.cy + dyPx * c.scale };
}

/** Centres `bounds` and fits its longer side (relative to the canvas) inside a margin of `marginPx`. */
export function fitBounds(c: Camera, [[x0, y0], [x1, y1]]: Bounds, marginPx: number): Camera {
  const w = Math.max(1, c.width - 2 * marginPx);
  const h = Math.max(1, c.height - 2 * marginPx);
  const scale = Math.max((x1 - x0) / w, (y1 - y0) / h);
  // An empty or single-point drawing has no size to fit; keep the zoom.
  return { ...c, cx: (x0 + x1) / 2, cy: (y0 + y1) / 2, scale: scale > 0 ? scale : c.scale };
}

/** The world box the canvas shows. */
export function visibleBounds(c: Camera): Bounds {
  const [hw, hh] = [(c.width / 2) * c.scale, (c.height / 2) * c.scale];
  return [
    [c.cx - hw, c.cy - hh],
    [c.cx + hw, c.cy + hh],
  ];
}

/** The camera for a canvas now `width` x `height` pixels: `view` (the box last shown on purpose)
 *  stays centred and whole, so a narrower canvas zooms out instead of cutting the drawing off, and
 *  the old size gives the old view back. */
export function resized(c: Camera, width: number, height: number, view: Bounds): Camera {
  return fitBounds({ ...c, width, height }, view, 0);
}

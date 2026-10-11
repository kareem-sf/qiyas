/** Constants, small types and pure helpers the window's hooks share. */
import { EMPTY_MODEL, type DrawingInfo, type Measurement, type ViewportInfo } from "../api";
import type { DraftingToggle } from "../tools/shortcuts";
import { measurementBounds } from "../tools/overlay";
import type { Batch } from "../viewport/batch";
import type { Bounds, Point } from "../viewport/camera";
import type { Theme } from "../viewport/gl";

export const THEME_KEY = "qiyas.theme";

export const MODEL = "*Model_Space";

/** The drafting setting each switch turns on and off. */
export const TOGGLED: Record<DraftingToggle, "osnap" | "grid" | "ortho" | "polar"> = {
  osnap: "osnap",
  grid: "grid",
  ortho: "ortho",
  polar: "polar",
};

/** A click in the drawing the window is waiting for instead of the tool's: the insert point of a
 *  PDF page or an image, or the two points of a calibration. */
export type Pending =
  | { kind: "insert"; of: "pdf" | "image" }
  | { kind: "calibrate"; first: Point | null }
  /** The entity a field reads: a click in the drawing picks it. */
  | { kind: "fieldTarget" };

/** Focus is in a text field: keys are typing, not shortcuts. */
export const typing = (e: KeyboardEvent) =>
  e.target instanceof HTMLElement &&
  (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));

/** The drawing's reading problems: all but the empty-model warning, which is said on its own. */
export const readProblems = (info: DrawingInfo) => info.problems.filter((p) => p !== EMPTY_MODEL);

/** The box around the measurements' bounds on the drawing, or null when none has bounds. */
export function unionBounds(ms: Measurement[], batch: Batch | null): Bounds | null {
  let box: Bounds | null = null;
  for (const m of ms) {
    const b = measurementBounds(m, batch);
    if (!b) continue;
    box = box
      ? [
          [Math.min(box[0][0], b[0][0]), Math.min(box[0][1], b[0][1])],
          [Math.max(box[1][0], b[1][0]), Math.max(box[1][1], b[1][1])],
        ]
      : b;
  }
  return box;
}

/** The stored theme, else the system's. */
export function initialTheme(): Theme {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    if (stored === "light" || stored === "dark") return stored;
  } catch {
    // Storage blocked: fall through to the system theme.
  }
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** No viewports: the layers panel's VP freeze column hides (one array, so the memo holds). */
export const NO_VIEWPORTS: ViewportInfo[] = [];

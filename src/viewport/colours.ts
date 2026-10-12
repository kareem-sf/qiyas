/** The canvas's colours: the theme's Paper or Ink behind the drawing and violet for what is
 *  highlighted, or the user's own (the Options dialog's background and highlight colours). */

export type Rgb = [number, number, number];
export type Theme = "light" | "dark";

/** The user's colours ("#rrggbb"), null for the theme's. */
export type CanvasColours = { background: string | null; highlight: string | null };

/** What the renderer draws with. `light`: the background is light, so white lines (ACI 7) are
 *  drawn in `ink`; on a dark one black lines are drawn in `paper`. Wipeouts take `background`,
 *  which is `paper` when light and `ink` when dark. */
export type Palette = { light: boolean; ink: Rgb; paper: Rgb; background: Rgb; highlight: Rgb };

const rgb = (hex: number): Rgb => [((hex >> 16) & 255) / 255, ((hex >> 8) & 255) / 255, (hex & 255) / 255];
export const INK = rgb(0x0e0f12);
export const PAPER = rgb(0xf9f8f4);
export const VIOLET = rgb(0x6e4a9e);

/** "#rrggbb" as 0–1 channels; null when it is not one. */
function parse(colour: string | null | undefined): Rgb | null {
  return colour && /^#[0-9a-f]{6}$/i.test(colour) ? rgb(parseInt(colour.slice(1), 16)) : null;
}

export function palette(theme: Theme, colours?: CanvasColours | null): Palette {
  const own = parse(colours?.background);
  // Relative luminance, roughly: light backgrounds read dark lines.
  const light = own ? 0.2126 * own[0] + 0.7152 * own[1] + 0.0722 * own[2] > 0.5 : theme === "light";
  const ink = !light && own ? own : INK;
  const paper = light && own ? own : PAPER;
  return { light, ink, paper, background: light ? paper : ink, highlight: parse(colours?.highlight) ?? VIOLET };
}

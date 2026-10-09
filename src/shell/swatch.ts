import type { Theme } from "../viewport/gl";

const INK: [number, number, number] = [0x0e, 0x0f, 0x12];
const PAPER: [number, number, number] = [0xf9, 0xf8, 0xf4];

/** The colour a layer's swatch shows: the one the drawing is drawn in. As the renderer does,
 *  pure white (ACI 7) is Ink on the light theme and pure black is Paper on the dark one. */
export function swatchColor(color: [number, number, number], theme: Theme): [number, number, number] {
  if (theme === "light" && color.every((c) => c === 255)) return INK;
  if (theme === "dark" && color.every((c) => c === 0)) return PAPER;
  return color;
}

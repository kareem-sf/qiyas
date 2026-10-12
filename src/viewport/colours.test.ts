import { expect, test } from "vitest";
import { INK, PAPER, palette, VIOLET } from "./colours";

test("with no colours of the user's, the theme's: Paper or Ink behind, violet highlight", () => {
  expect(palette("light")).toEqual({ light: true, ink: INK, paper: PAPER, background: PAPER, highlight: VIOLET });
  expect(palette("dark", { background: null, highlight: null })).toEqual({
    light: false,
    ink: INK,
    paper: PAPER,
    background: INK,
    highlight: VIOLET,
  });
});

test("a background of the user's is drawn behind, and its lightness decides how white and black lines show", () => {
  // A dark blue in the light theme: drawn as a dark background (black lines turn Paper).
  const blue = palette("light", { background: "#102040", highlight: null });
  expect(blue.light).toBe(false);
  expect(blue.background).toEqual([0x10 / 255, 0x20 / 255, 0x40 / 255]);
  // Wipeouts take the background (ink is the dark side's background), black lines become Paper.
  expect(blue.ink).toEqual(blue.background);
  expect(blue.paper).toEqual(PAPER);
  // A light grey in the dark theme: white lines turn Ink.
  const grey = palette("dark", { background: "#e0e0e0", highlight: "#FF8800" });
  expect(grey.light).toBe(true);
  expect(grey.paper).toEqual(grey.background);
  expect(grey.ink).toEqual(INK);
  expect(grey.highlight).toEqual([1, 0x88 / 255, 0]);
});

test("a colour that is not #rrggbb is the theme's", () => {
  expect(palette("light", { background: "red", highlight: "#12" })).toEqual(palette("light"));
});

import { expect, test } from "vitest";
import { gridLevels, offsetInPeriod } from "./grid";

test("dots closer than 8 pixels hide, the major ones last", () => {
  // 1 unit apart at 0.1 units per pixel: 10 px apart.
  expect(gridLevels(1, 0.1)).toEqual({ minor: true, major: true });
  // 4 px apart: only every tenth dot (40 px).
  expect(gridLevels(1, 0.25)).toEqual({ minor: false, major: true });
  // Even the major dots would be 5 px apart.
  expect(gridLevels(1, 2)).toEqual({ minor: false, major: false });
  expect(gridLevels(0, 0.1)).toEqual({ minor: false, major: false });
});

test("far from the origin the shader still gets a small offset", () => {
  expect(offsetInPeriod(1_000_003.25, 10)).toBeCloseTo(3.25, 6);
  expect(offsetInPeriod(-17, 10)).toBeCloseTo(3, 12);
});

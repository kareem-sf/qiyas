import { expect, test } from "vitest";
import { swatchColor } from "./swatch";

test("white shows Ink on the light theme, black shows Paper on the dark one", () => {
  expect(swatchColor([255, 255, 255], "light")).toEqual([14, 15, 18]);
  expect(swatchColor([255, 255, 255], "dark")).toEqual([255, 255, 255]);
  expect(swatchColor([0, 0, 0], "dark")).toEqual([249, 248, 244]);
  expect(swatchColor([0, 0, 0], "light")).toEqual([0, 0, 0]);
});

test("other colours are shown as they are", () => {
  expect(swatchColor([255, 0, 0], "light")).toEqual([255, 0, 0]);
  expect(swatchColor([254, 255, 255], "light")).toEqual([254, 255, 255]);
  expect(swatchColor([51, 51, 51], "dark")).toEqual([51, 51, 51]);
});

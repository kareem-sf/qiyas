import { expect, test } from "vitest";
import { isWorld, toUcs, toWorld, ucsDegrees, WORLD } from "./ucs";

const turned = { origin: [10, 5] as [number, number], x_axis: [0, 1] as [number, number] };

test("a point goes into a UCS and back", () => {
  const [x, y] = toUcs(turned, [10, 6]);
  expect(x).toBeCloseTo(1, 12);
  expect(y).toBeCloseTo(0, 12);
  const [wx, wy] = toWorld(turned, [1, 2]);
  expect(wx).toBeCloseTo(8, 12);
  expect(wy).toBeCloseTo(6, 12);
  expect(toUcs(WORLD, [3, 4])).toEqual([3, 4]);
});

test("the world is the world, and a UCS has its angle", () => {
  expect(isWorld(WORLD)).toBe(true);
  expect(isWorld(undefined)).toBe(true);
  expect(isWorld(turned)).toBe(false);
  expect(ucsDegrees(turned)).toBeCloseTo(90, 12);
});

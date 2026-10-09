import { expect, test } from "vitest";
import { constrain, gridRound, NO_TRACK } from "./tracking";

test("no previous point or no tracking leaves the point alone", () => {
  expect(constrain(null, [3, 4], { ...NO_TRACK, ortho: true })).toEqual([3, 4]);
  expect(constrain([0, 0], [3, 4], NO_TRACK)).toEqual([3, 4]);
});

test("ortho wins over polar", () => {
  expect(constrain([0, 0], [3, 3.1], { ortho: true, polar: true, polarStepDeg: 45 })).toEqual([0, 3.1]);
});

test("polar works in every quadrant and with other steps", () => {
  const [x, y] = constrain([10, 10], [7, 7.1], { ortho: false, polar: true, polarStepDeg: 45 });
  expect(x - 10).toBeCloseTo(y - 10, 9);
  const [, flat] = constrain([0, 0], [-5, 0.2], { ortho: false, polar: true, polarStepDeg: 90 });
  expect(flat).toBeCloseTo(0, 12);
});

test("grid snap rounds to the nearest dot", () => {
  expect(gridRound([1.26, -0.74], 0.5)).toEqual([1.5, -0.5]);
  expect(gridRound([1.26, -0.74], 0)).toEqual([1.26, -0.74]);
});

test("ortho and the grid follow the UCS", () => {
  const ucs = { origin: [1, 1] as [number, number], x_axis: [Math.SQRT1_2, Math.SQRT1_2] as [number, number] };
  // Along the UCS x axis (45 degrees in the world), from (1, 1).
  const [x, y] = constrain([1, 1], [3, 3.2], { ...NO_TRACK, ortho: true, ucs });
  expect(x - 1).toBeCloseTo(y - 1, 9);
  // A grid dot of the UCS: 1 along its x axis from its origin.
  const [gx, gy] = gridRound([1.75, 1.65], 1, ucs);
  expect(gx).toBeCloseTo(1 + Math.SQRT1_2, 9);
  expect(gy).toBeCloseTo(1 + Math.SQRT1_2, 9);
});

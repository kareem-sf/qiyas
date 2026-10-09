import { expect, test } from "vitest";
import { wheelFactor } from "./wheel";

test("a classic notch zooms by 1.1, either way", () => {
  expect(wheelFactor(100)).toBeCloseTo(1.1, 12);
  expect(wheelFactor(-100)).toBeCloseTo(1 / 1.1, 12);
  // Firefox: three lines for a notch.
  expect(wheelFactor(3, 1)).toBeCloseTo(1.1, 12);
});

test("a touchpad's small deltas zoom a little", () => {
  const f = wheelFactor(4);
  expect(f).toBeGreaterThan(1);
  expect(f).toBeLessThan(1.01);
  expect(wheelFactor(-4)).toBeCloseTo(1 / f, 12);
});

test("one event never zooms beyond 1.5", () => {
  expect(wheelFactor(5000)).toBe(1.5);
  expect(wheelFactor(-5000)).toBeCloseTo(1 / 1.5, 12);
  expect(wheelFactor(1, 2)).toBeLessThanOrEqual(1.5);
  expect(wheelFactor(0)).toBe(1);
});

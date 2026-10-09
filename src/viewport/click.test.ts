import { expect, test } from "vitest";
import { isClick } from "./click";

test("a release within 4 px of the press is a click", () => {
  expect(isClick([10, 10], [10, 10])).toBe(true);
  expect(isClick([10, 10], [12, 12])).toBe(true);
});

test("a drag of 4 px or more is not", () => {
  expect(isClick([10, 10], [14, 10])).toBe(false);
  expect(isClick([10, 10], [13, 13])).toBe(false);
  expect(isClick([10, 10], [10, 40])).toBe(false);
});

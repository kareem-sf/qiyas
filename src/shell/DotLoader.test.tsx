// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { DotLoader, dotsFor, type DotPattern } from "./DotLoader";
import css from "./theme.css?raw";

afterEach(cleanup);

const PATTERNS: DotPattern[] = ["scan", "spiral", "pulse", "chase"];

test("a small loader is a 3 by 3 grid, a medium one 5 by 5", () => {
  const small = render(<DotLoader size="sm" />);
  expect(small.container.querySelectorAll("i")).toHaveLength(9);
  small.unmount();
  const medium = render(<DotLoader size="md" />);
  expect(medium.container.querySelectorAll("i")).toHaveLength(25);
  expect(medium.container.firstElementChild?.classList.contains("dot-md")).toBe(true);
});

test("the pattern is a class, scan and small by default", () => {
  const { container, rerender } = render(<DotLoader />);
  const el = container.firstElementChild as HTMLElement;
  expect(el.classList.contains("dot-scan")).toBe(true);
  expect(el.classList.contains("dot-sm")).toBe(true);
  rerender(<DotLoader pattern="spiral" />);
  expect(container.firstElementChild?.classList.contains("dot-spiral")).toBe(true);
  expect(container.firstElementChild?.classList.contains("dot-scan")).toBe(false);
});

test("every dot moving has its place in the motion, 0 up to 1, set by index (no timers)", () => {
  for (const pattern of PATTERNS) {
    for (const size of ["sm", "md"] as const) {
      for (const dot of dotsFor(pattern, size)) {
        if (dot.p !== null) {
          expect(dot.p).toBeGreaterThanOrEqual(0);
          expect(dot.p).toBeLessThan(1);
        }
      }
    }
  }
  const { container } = render(<DotLoader pattern="scan" />);
  const dots = [...container.querySelectorAll<HTMLElement>("i")];
  // Scan runs column by column: the dots of a column share a place, the next column is later.
  const p = dots.map((d) => Number(d.style.getPropertyValue("--p")));
  expect(p).toEqual([0, 1 / 3, 2 / 3, 0, 1 / 3, 2 / 3, 0, 1 / 3, 2 / 3].map((v) => Number(v.toFixed(3))));
});

test("the patterns differ from one another", () => {
  const seen = new Set(PATTERNS.map((pattern) => JSON.stringify(dotsFor(pattern, "sm"))));
  expect(seen.size).toBe(PATTERNS.length);
});

test("the spiral visits every cell once and ends in the middle; the chase rests the inner dots", () => {
  const spiral = dotsFor("spiral", "md");
  expect(new Set(spiral.map((d) => d.p)).size).toBe(25);
  // The last cell of the spiral is the centre, row 2 column 2.
  expect(spiral[12].p).toBeCloseTo(24 / 25, 3);
  const chase = dotsFor("chase", "md");
  expect(chase.filter((d) => d.p === null)).toHaveLength(9);
  expect(dotsFor("chase", "sm")[4].p).toBeNull();
  // The pulse starts at the centre and the corners come last.
  const pulse = dotsFor("pulse", "sm");
  expect(pulse[4].p).toBeLessThan(pulse[0].p as number);
});

test("with no label the loader is hidden from assistive technology", () => {
  const { container } = render(<DotLoader />);
  const el = container.firstElementChild as HTMLElement;
  expect(el.getAttribute("aria-hidden")).toBe("true");
  expect(el.getAttribute("role")).toBeNull();
});

test("with a label it is a named image, not a live region", () => {
  const { getByRole, container } = render(<DotLoader label="Opening the drawing" />);
  expect(getByRole("img", { name: "Opening the drawing" })).toBeTruthy();
  expect(container.querySelector("[aria-live]")).toBeNull();
  expect(container.querySelector('[role="status"]')).toBeNull();
  expect(container.firstElementChild?.getAttribute("aria-hidden")).toBeNull();
});

test("reduced motion stops the animation and leaves the dots still and visible", () => {
  const block = css.slice(css.indexOf("/* Still: no motion"));
  const reduced = block.slice(0, block.indexOf("}\n}") + 3);
  expect(reduced).toContain("prefers-reduced-motion: reduce");
  expect(reduced).toMatch(/\.dot-loader i \{[^}]*animation: none;/);
  // A still dot is never fully gone: the dimmest keeps a quarter of the full opacity.
  expect(reduced).toMatch(/opacity: calc\(1 - var\(--p\) \* 0\.75\)/);
});

test("the motion is CSS only, using the theme colours and the text direction", () => {
  expect(css).toContain("@keyframes dot-glow");
  expect(css).toMatch(/\.dot-loader \{[^}]*color: var\(--accent\)/);
  expect(css).toMatch(/\.dot-loader i \{[^}]*background: currentColor/);
  // Logical margin so the gap sits on the right side in Arabic too.
  expect(css).toMatch(/\.dot-loader \{\s*margin-inline-end: 6px/);
});

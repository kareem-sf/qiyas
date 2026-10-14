// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { hasIcon, ICON_BODIES, MIRRORED, RibbonIcon } from "./ribbonIcons";
import { DEFAULT_RIBBON, QUICK_ACCESS } from "./ribbonLayout";

afterEach(cleanup);

const commands = () => [
  ...new Set([...QUICK_ACCESS, ...DEFAULT_RIBBON.tabs.flatMap((t) => t.groups.flatMap((g) => g.buttons))].map((b) => b.command)),
];

test("every button of the default ribbon and the quick-access bar has an icon", () => {
  expect(commands().length).toBeGreaterThan(100);
  expect(commands().filter((c) => !hasIcon(c))).toEqual([]);
});

test("the chrome has its icons too, and the revision cloud with a comment is ready for its button", () => {
  for (const name of ["more", "chevron", "sun", "moon", "info", "commentCloud"]) expect(hasIcon(name)).toBe(true);
});

test("one family: 24 grid, 1.5 stroke, currentColor, drawn inline", () => {
  const { container } = render(
    <>
      {Object.keys(ICON_BODIES).map((n) => (
        <RibbonIcon key={n} name={n} />
      ))}
    </>,
  );
  const all = Array.from(container.querySelectorAll("svg"));
  expect(all).toHaveLength(Object.keys(ICON_BODIES).length);
  for (const svg of all) {
    expect(svg.getAttribute("viewBox")).toBe("0 0 24 24");
    expect(svg.getAttribute("stroke")).toBe("currentColor");
    expect(svg.getAttribute("stroke-width")).toBe("1.5");
    expect(svg.getAttribute("aria-hidden")).toBe("true");
    expect(svg.children.length).toBeGreaterThan(0);
  }
  // No colour of its own in any icon: it takes the button's.
  for (const body of Object.values(ICON_BODIES)) expect(body).not.toMatch(/#[0-9a-f]{3,6}|rgb\(/i);
});

test("icons that point along the text mirror in Arabic", () => {
  expect([...MIRRORED].sort()).toEqual(["extend", "lengthen", "redo", "runScript", "undo"]);
  const { container } = render(
    <>
      <RibbonIcon name="undo" />
      <RibbonIcon name="line" />
    </>,
  );
  const [undo, line] = Array.from(container.querySelectorAll("svg"));
  expect(undo.getAttribute("class")).toContain("ic-mirrored");
  expect(line.getAttribute("class")).not.toContain("ic-mirrored");
});

test("Trim is not a plus: it crosses the cutting edge at a slant and dashes what is cut", () => {
  expect(ICON_BODIES.trim).toContain("stroke-dasharray");
  expect(ICON_BODIES.trim).not.toContain("M12 3v11");
});

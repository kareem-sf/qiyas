// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import brand from "../../public/brand/qiyas-mark.svg?raw";
import css from "../shell/theme.css?raw";
import { MizanMark } from "./MizanMark";

afterEach(cleanup);

test("the mark is the brand drawing: the sphere's mesh and its face, turning together", () => {
  const { container } = render(<MizanMark />);
  const svg = container.querySelector("svg.mizan-mark.mizan-mark-sm")!;
  expect(svg.getAttribute("aria-hidden")).toBe("true");
  const mesh = svg.querySelector(".mm-sphere")!.getAttribute("d")!;
  expect(brand).toContain(mesh);
  expect(svg.querySelector(".mm-turn .mm-face")).toBeTruthy();
  expect(brand).toContain(svg.querySelector(".mm-face")!.getAttribute("points")!);
});

test("two marks on one page keep their own gradient", () => {
  const { container } = render(
    <>
      <MizanMark />
      <MizanMark size="md" />
    </>,
  );
  const ids = [...container.querySelectorAll("linearGradient")].map((g) => g.id);
  expect(new Set(ids).size).toBe(2);
  for (const face of container.querySelectorAll(".mm-face")) {
    expect(ids).toContain(face.getAttribute("fill")!.slice(5, -1));
  }
});

test("with reduced motion the mark stands still", () => {
  expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\) \{\s*\.mizan-mark \.mm-turn,\s*\.mizan-mark \.mm-face \{\s*animation: none;/);
});

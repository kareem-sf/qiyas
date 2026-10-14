// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { Progress } from "./Progress";
import { StatusBar } from "../shell/StatusBar";

afterEach(cleanup);

test("a labelled bar: the role and value for assistive technology, the percentage at the end of the label's line", () => {
  render(<Progress value={42.4} label="Drawing the page…" />);
  const bar = screen.getByRole("progressbar", { name: "Drawing the page…" });
  expect(bar.getAttribute("aria-valuenow")).toBe("42");
  expect(bar.getAttribute("aria-valuemin")).toBe("0");
  expect(bar.getAttribute("aria-valuemax")).toBe("100");
  expect(bar.querySelector(".progress-value")!.textContent).toBe("42 %");
  expect(bar.querySelector<HTMLElement>(".progress-fill")!.style.width).toBe("42%");
});

test("the value stays between 0 and 100", () => {
  const { rerender } = render(<Progress value={140} label="x" />);
  expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("100");
  rerender(<Progress value={-3} label="x" />);
  expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("0");
});

test("the status bar shows a PDF page being drawn as a bar with its words, and dots for the waits with no percentage", () => {
  const props = { cursor: null, units: "", unitsWarning: false, problems: [] };
  const { container, rerender } = render(<StatusBar {...props} message="Drawing the PDF page…" progress={30} />);
  const bar = screen.getByRole("progressbar", { name: "Drawing the PDF page…" });
  expect(bar.getAttribute("aria-valuenow")).toBe("30");
  expect(container.querySelector(".dot-loader")).toBeNull();
  rerender(<StatusBar {...props} message="Opening…" waiting="scan" progress={null} />);
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(container.querySelector(".dot-loader.dot-scan")).toBeTruthy();
  expect(container.querySelector(".message")!.textContent).toBe("Opening…");
});

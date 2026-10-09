// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { Ribbon } from "./Ribbon";

afterEach(cleanup);

const props = { active: "select" as const, onTool() {}, onOpen() {}, onZoomExtents() {} };

test("the View tab says whether the drawing's Arabic text is reversed", () => {
  const onArabicReversed = vi.fn();
  const { rerender } = render(
    <Ribbon {...props} arabicReversed={false} onArabicReversed={onArabicReversed} />,
  );
  fireEvent.click(screen.getByRole("tab", { name: "View" }));
  const box = screen.getByRole("checkbox", { name: "Arabic text is reversed" }) as HTMLInputElement;
  expect(box.checked).toBe(false);
  fireEvent.click(box);
  expect(onArabicReversed).toHaveBeenCalledWith(true);

  rerender(<Ribbon {...props} arabicReversed />);
  const off = screen.getByRole("checkbox", { name: "Arabic text is reversed" }) as HTMLInputElement;
  expect(off.checked).toBe(true);
  // No drawing open: nothing to set.
  expect(off.disabled).toBe(true);
});

test("the Measure tab has the inquiry tools and Extract data", () => {
  const onTool = vi.fn();
  const onExtract = vi.fn();
  render(<Ribbon {...props} onTool={onTool} onExtract={onExtract} />);
  fireEvent.click(screen.getByRole("tab", { name: "Measure" }));
  for (const [name, tool] of [
    ["Geometry", "geometry"],
    ["Divide", "divide"],
    ["Measure along", "measureAlong"],
    ["ID point", "idPoint"],
  ]) {
    fireEvent.click(screen.getByRole("button", { name }));
    expect(onTool).toHaveBeenLastCalledWith(tool);
  }
  fireEvent.click(screen.getByRole("button", { name: "Extract data" }));
  expect(onExtract).toHaveBeenCalled();
});

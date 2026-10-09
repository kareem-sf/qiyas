// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { Ribbon } from "./Ribbon";

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

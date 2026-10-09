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

test("Open has the recent drawings beside it, and View has the named views", async () => {
  const loadViews = vi.fn(async () => []);
  render(
    <Ribbon
      {...props}
      loadRecents={async () => []}
      onOpenRecent={() => {}}
      onClearRecents={async () => {}}
      loadViews={loadViews}
      onGoToView={() => {}}
      onSaveView={async () => {}}
      onDeleteView={async () => {}}
    />,
  );
  expect(screen.getByRole("button", { name: "Open…" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Recent drawings" })).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "View" }));
  fireEvent.click(screen.getByRole("button", { name: /Views/ }));
  expect(await screen.findByRole("menu", { name: "Views" })).toBeTruthy();
  expect(loadViews).toHaveBeenCalled();
});

test("without a drawing on the Model sheet, Views is disabled", () => {
  render(<Ribbon {...props} />);
  fireEvent.click(screen.getByRole("tab", { name: "View" }));
  expect((screen.getByRole("button", { name: "Views" }) as HTMLButtonElement).disabled).toBe(true);
});

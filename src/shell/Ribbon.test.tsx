// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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

test("the ribbon tabs have a roving tab stop and Space presses a tab", () => {
  render(<Ribbon {...props} />);
  const tab = (name: string) => screen.getByRole("tab", { name });
  expect(["Home", "Measure", "View"].map((n) => tab(n).tabIndex)).toEqual([0, -1, -1]);
  tab("Home").focus();
  fireEvent.keyDown(tab("Home"), { key: "ArrowRight" });
  expect(document.activeElement).toBe(tab("Measure"));
  fireEvent.keyDown(tab("Measure"), { key: " " });
  expect(tab("Measure").getAttribute("aria-selected")).toBe("true");
  expect(["Home", "Measure", "View"].map((n) => tab(n).tabIndex)).toEqual([-1, 0, -1]);
});

test("the Home tab has the Draw group", () => {
  const onTool = vi.fn();
  const { rerender } = render(<Ribbon {...props} onTool={onTool} />);
  for (const [name, tool] of [
    ["Line", "line"],
    ["Polyline", "polyline"],
    ["Rectangle", "rectangle"],
    ["Circle", "circle"],
    ["Arc", "arc"],
    ["Ellipse", "ellipse"],
    ["Spline", "spline"],
    ["Point", "point"],
    ["Hatch", "hatch"],
    ["Revision cloud", "revcloud"],
    ["Wipeout", "wipeout"],
  ]) {
    fireEvent.click(screen.getByRole("button", { name }));
    expect(onTool).toHaveBeenLastCalledWith(tool);
  }
  // The tool on goes back to Select when pressed again.
  rerender(<Ribbon {...props} active="line" onTool={onTool} />);
  const line = screen.getByRole("button", { name: "Line" });
  expect(line.getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(line);
  expect(onTool).toHaveBeenLastCalledWith("select");
});

test("the Home tab has the Modify group", () => {
  const onTool = vi.fn();
  render(<Ribbon {...props} onTool={onTool} />);
  const group = screen.getByRole("group", { name: "Modify" });
  expect(group).toBeTruthy();
  for (const [name, tool] of [
    ["Move", "move"],
    ["Copy", "copy"],
    ["Rotate", "rotate"],
    ["Scale", "scale"],
    ["Mirror", "mirror"],
    ["Stretch", "stretch"],
    ["Align", "align"],
    ["Array", "array"],
    ["Explode", "explode"],
    ["Erase", "erase"],
  ]) {
    fireEvent.click(screen.getByRole("button", { name }));
    expect(onTool).toHaveBeenLastCalledWith(tool);
  }
});

test("the Home tab has the Text group with Find and replace", () => {
  const onTool = vi.fn();
  const onFind = vi.fn();
  render(<Ribbon {...props} onTool={onTool} onFind={onFind} />);
  const group = screen.getByRole("group", { name: "Text" });
  for (const [name, tool] of [
    ["Text", "text"],
    ["MText", "mtext"],
    ["Edit text", "textEdit"],
  ]) {
    fireEvent.click(within(group).getByRole("button", { name }));
    expect(onTool).toHaveBeenLastCalledWith(tool);
  }
  fireEvent.click(within(group).getByRole("button", { name: "Find and replace" }));
  expect(onFind).toHaveBeenCalled();
});

test("the Annotate tab has the Dimensions group, the dimension style and the Leaders group", () => {
  const onTool = vi.fn();
  const onDimStyle = vi.fn();
  render(<Ribbon {...props} onTool={onTool} onDimStyle={onDimStyle} />);
  fireEvent.click(screen.getByRole("tab", { name: "Annotate" }));
  const dims = screen.getByRole("group", { name: "Dimensions" });
  for (const [name, tool] of [
    ["Linear", "dimLinear"],
    ["Aligned", "dimAligned"],
    ["Angular", "dimAngular"],
    ["Radius", "dimRadius"],
    ["Diameter", "dimDiameter"],
    ["Arc length", "dimArc"],
    ["Ordinate", "dimOrdinate"],
    ["Baseline", "dimBaseline"],
    ["Continue", "dimContinue"],
    ["Quick", "qdim"],
    ["Edit dimension", "dimEdit"],
    ["Break", "dimBreak"],
    ["Space", "dimSpace"],
    ["Jog line", "dimJogLine"],
  ]) {
    fireEvent.click(within(dims).getByRole("button", { name }));
    expect(onTool).toHaveBeenLastCalledWith(tool);
  }
  fireEvent.click(within(dims).getByRole("button", { name: "Dimension style…" }));
  expect(onDimStyle).toHaveBeenCalled();
  const notes = screen.getByRole("group", { name: "Leaders" });
  for (const [name, tool] of [
    ["Leader", "leader"],
    ["Multileader", "mleader"],
    ["Tolerance", "tolerance"],
  ]) {
    fireEvent.click(within(notes).getByRole("button", { name }));
    expect(onTool).toHaveBeenLastCalledWith(tool);
  }
});

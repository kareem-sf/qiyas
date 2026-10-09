// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import type { Command, TableInfo } from "../api";
import { LanguageProvider } from "../i18n";
import { NewTableDialog, TableEditor } from "./TableEditor";

beforeAll(() => {
  // jsdom may not implement modal dialogs.
  HTMLDialogElement.prototype.showModal ??= function (this: HTMLDialogElement) {
    this.open = true;
  };
  HTMLDialogElement.prototype.close ??= function (this: HTMLDialogElement) {
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
});

afterEach(cleanup);

const plain: TableInfo = {
  cells: [
    ["Room", "Area"],
    ["Hall", "12.000"],
  ],
  widths: [4, 4],
  heights: [1, 1],
  merges: [],
  fields: [[1, 1]],
};

function editor(table: TableInfo = plain, cell: [number, number] = [0, 0]) {
  const onRun = vi.fn(async (_cmd: Command) => {});
  const onField = vi.fn();
  render(
    <LanguageProvider>
      <TableEditor handle={42} table={table} cell={cell} onRun={onRun} onField={onField} onClose={() => {}} />
    </LanguageProvider>,
  );
  return { onRun, onField };
}

const cellInput = (row: number, col: number) =>
  screen.getByRole("textbox", { name: `Cell ${row + 1}, ${col + 1}` }) as HTMLInputElement;
const target = { handle: 42, chain: [] };

test("shows the cells in the drawing's column order, whatever the language's direction", () => {
  editor();
  const grid = screen.getByRole("grid");
  // Column 1 is the drawing's first, on the left, in Arabic too.
  expect(grid.getAttribute("dir")).toBe("ltr");
  expect(cellInput(0, 0).value).toBe("Room");
  expect(cellInput(1, 1).value).toBe("12.000");
  // Arabic or English, each cell's text runs its own way.
  expect(cellInput(0, 0).getAttribute("dir")).toBe("auto");
  // A field's cell says so.
  expect(cellInput(1, 1).getAttribute("title")).toBe("Field");
});

test("a changed cell is written when Enter is pressed or it loses focus; an unchanged one is not", async () => {
  const { onRun } = editor();
  fireEvent.focus(cellInput(1, 0));
  fireEvent.blur(cellInput(1, 0));
  expect(onRun).not.toHaveBeenCalled();
  fireEvent.change(cellInput(1, 0), { target: { value: "غرفة" } });
  fireEvent.keyDown(cellInput(1, 0), { key: "Enter" });
  await waitFor(() =>
    expect(onRun).toHaveBeenCalledWith({ type: "edit_cell", target, row: 1, col: 0, text: "غرفة" }),
  );
});

test("rows and columns go in beside the chosen cell and come out with it", async () => {
  const { onRun } = editor(plain, [1, 0]);
  const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
  click("Insert row above");
  click("Insert row below");
  click("Insert column left");
  click("Insert column right");
  click("Delete row");
  click("Delete column");
  await waitFor(() => expect(onRun).toHaveBeenCalledTimes(6));
  expect(onRun.mock.calls.map((c) => c[0])).toEqual([
    { type: "insert_row", target, at: 1 },
    { type: "insert_row", target, at: 2 },
    { type: "insert_col", target, at: 0 },
    { type: "insert_col", target, at: 1 },
    { type: "delete_row", target, row: 1 },
    { type: "delete_col", target, col: 0 },
  ]);
});

test("Shift+click chooses cells to merge; a merged cell spans them and unmerges", async () => {
  const { onRun } = editor();
  const merge = screen.getByRole("button", { name: "Merge cells" }) as HTMLButtonElement;
  expect(merge.disabled).toBe(true);
  fireEvent.click(cellInput(0, 1), { shiftKey: true });
  expect(merge.disabled).toBe(false);
  fireEvent.click(merge);
  await waitFor(() =>
    expect(onRun).toHaveBeenCalledWith({ type: "merge_cells", target, first: [0, 0], last: [0, 1] }),
  );
  cleanup();

  const merged = { ...plain, merges: [[0, 0, 0, 1]] as [number, number, number, number][] };
  const second = editor(merged);
  expect(screen.queryByRole("textbox", { name: "Cell 1, 2" })).toBeNull();
  expect(cellInput(0, 0).closest("td")?.getAttribute("colspan")).toBe("2");
  fireEvent.click(screen.getByRole("button", { name: "Unmerge" }));
  await waitFor(() =>
    expect(second.onRun).toHaveBeenCalledWith({ type: "merge_cells", target, first: [0, 0], last: [0, 0] }),
  );
});

test("Field… asks for a field in the chosen cell; a table keeps one row and one column", () => {
  const { onField } = editor(plain, [1, 1]);
  fireEvent.click(screen.getByRole("button", { name: "Field…" }));
  expect(onField).toHaveBeenCalledWith([1, 1]);
  cleanup();
  editor({ cells: [["x"]], widths: [1], heights: [1], merges: [], fields: [] });
  expect((screen.getByRole("button", { name: "Delete row" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Delete column" }) as HTMLButtonElement).disabled).toBe(true);
});

test("a new table's rows, columns and sizes", async () => {
  const onSubmit = vi.fn(async () => {});
  render(
    <LanguageProvider>
      <NewTableDialog textHeight={2.5} onSubmit={onSubmit} onClose={() => {}} />
    </LanguageProvider>,
  );
  const dialog = screen.getByRole("dialog");
  const field = (name: string) => within(dialog).getByRole("spinbutton", { name }) as HTMLInputElement;
  // Rows twice the text height, columns ten times.
  expect(field("Row height").value).toBe("5");
  expect(field("Column width").value).toBe("25");
  fireEvent.change(field("Rows"), { target: { value: "4" } });
  fireEvent.change(field("Columns"), { target: { value: "3" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Insert" }));
  await waitFor(() =>
    expect(onSubmit).toHaveBeenCalledWith({ rows: 4, cols: 3, col_width: 25, row_height: 5 }),
  );
});

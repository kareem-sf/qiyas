// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { LanguageProvider } from "../i18n";
import { GroupNameDialog, LayerStatesDialog } from "./LayerDialogs";
import { LayerTranslator } from "./LayerTranslator";

beforeAll(() => {
  HTMLDialogElement.prototype.showModal ??= function (this: HTMLDialogElement) {
    this.open = true;
  };
  HTMLDialogElement.prototype.close ??= function (this: HTMLDialogElement) {
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
});

afterEach(cleanup);

const wrap = (ui: React.ReactNode) => render(<LanguageProvider>{ui}</LanguageProvider>);

test("layer states: save one, restore, rename and delete the others", async () => {
  const actions = {
    onSave: vi.fn(async () => {}),
    onRestore: vi.fn(async () => {}),
    onRename: vi.fn(async () => {}),
    onDelete: vi.fn(async () => {}),
  };
  wrap(<LayerStatesDialog states={["Plan", "Ceiling"]} {...actions} onClose={() => {}} />);
  fireEvent.change(screen.getByRole("textbox", { name: "New state" }), { target: { value: "Doors" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(actions.onSave).toHaveBeenCalledWith("Doors"));
  const rename = screen.getByRole("textbox", { name: "Rename Ceiling" });
  fireEvent.change(rename, { target: { value: "RCP" } });
  fireEvent.keyDown(rename, { key: "Enter" });
  await waitFor(() => expect(actions.onRename).toHaveBeenCalledWith("Ceiling", "RCP"));
  fireEvent.click(screen.getByRole("button", { name: "Delete Ceiling" }));
  await waitFor(() => expect(actions.onDelete).toHaveBeenCalledWith("Ceiling"));
  // Restoring one closes the dialog.
  fireEvent.click(screen.getByRole("button", { name: "Restore Plan" }));
  await waitFor(() => expect(actions.onRestore).toHaveBeenCalledWith("Plan"));
});

test("a refused layer state action says why in the dialog", async () => {
  const onSave = vi.fn(async () => {
    throw "Give the layer state a name";
  });
  wrap(
    <LayerStatesDialog
      states={[]}
      onSave={onSave}
      onRestore={async () => {}}
      onRename={async () => {}}
      onDelete={async () => {}}
      onClose={() => {}}
    />,
  );
  expect(screen.getByText("This drawing has no layer states.")).toBeTruthy();
  fireEvent.change(screen.getByRole("textbox", { name: "New state" }), { target: { value: "x" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("alert")).toBeTruthy();
});

test("a group needs a name no other group has", async () => {
  const onSubmit = vi.fn(async () => {});
  wrap(<GroupNameDialog names={["Room 1"]} onSubmit={onSubmit} onClose={() => {}} />);
  const name = screen.getByRole("textbox", { name: "Group name" });
  const ok = screen.getByRole("button", { name: "Group" }) as HTMLButtonElement;
  fireEvent.change(name, { target: { value: "room 1" } });
  expect(ok.disabled).toBe(true);
  fireEvent.change(name, { target: { value: "Room 2" } });
  fireEvent.click(ok);
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("Room 2"));
});

test("the translator maps this drawing's layers onto a standard's, same names at once", async () => {
  const onTranslate = vi.fn(async () => {});
  const load = vi.fn(async () => ({
    path: "/std/standard.dwg",
    layers: [
      { name: "A-WALL", color: [0, 0, 255] as [number, number, number], off: false, frozen: false, locked: false },
      { name: "S-COLS", color: [0, 0, 255] as [number, number, number], off: false, frozen: false, locked: false },
    ],
  }));
  const own = (name: string) => ({ name, color: [255, 0, 0] as [number, number, number], off: false, frozen: false, locked: false });
  wrap(
    <LayerTranslator
      layers={[own("0"), own("a-wall"), own("COLUMNS"), own("PLAN|X")]}
      loadStandard={load}
      onTranslate={onTranslate}
      onClose={() => {}}
    />,
  );
  // An xref's layers are not this drawing's to translate.
  expect(screen.queryByRole("combobox", { name: "Translate PLAN|X to" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Load standard…" }));
  await screen.findByText("/std/standard.dwg");
  fireEvent.click(screen.getByRole("button", { name: "Map same names" }));
  expect((screen.getByRole("combobox", { name: "Translate a-wall to" }) as HTMLSelectElement).value).toBe("A-WALL");
  fireEvent.change(screen.getByRole("combobox", { name: "Translate COLUMNS to" }), { target: { value: "S-COLS" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Colour for COLUMNS" }), { target: { value: "ACI 4" } });
  fireEvent.click(screen.getByRole("button", { name: "Translate" }));
  await waitFor(() =>
    expect(onTranslate).toHaveBeenCalledWith("/std/standard.dwg", [
      { from: "a-wall", to: "A-WALL", color: null },
      { from: "COLUMNS", to: "S-COLS", color: "ACI 4" },
    ]),
  );
});

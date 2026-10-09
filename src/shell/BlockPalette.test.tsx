// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import type { BlockInfo } from "../api";
import { LanguageProvider } from "../i18n";
import { BlockPalette } from "./BlockPalette";
import { DesignCenter } from "./DesignCenter";
import { ToolPalettes } from "./ToolPalettes";

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

const BLOCKS: BlockInfo[] = [
  { name: "DOOR", attdefs: [] },
  { name: "TAG", attdefs: [{ tag: "MARK", prompt: "Mark", default: "D1", constant: false }] },
];

function palette() {
  const thumbnail = vi.fn(async (name: string) => `blob:${name}`);
  const onInsert = vi.fn();
  const onWrite = vi.fn();
  const onAdd = vi.fn();
  const onClose = vi.fn();
  render(
    <LanguageProvider>
      <BlockPalette
        load={async () => BLOCKS}
        thumbnail={thumbnail}
        onInsert={onInsert}
        onWrite={onWrite}
        onAddToPalette={onAdd}
        onClose={onClose}
      />
    </LanguageProvider>,
  );
  return { thumbnail, onInsert, onWrite, onAdd, onClose };
}

test("the block palette shows each block's thumbnail; a click inserts it", async () => {
  const { thumbnail, onInsert, onAdd, onClose } = palette();
  const door = await screen.findByRole("img", { name: "DOOR" });
  await waitFor(() => expect(door.getAttribute("src")).toBe("blob:DOOR"));
  expect(thumbnail).toHaveBeenCalledWith("TAG");
  // Adding to the tool palette keeps the palette open.
  fireEvent.click(screen.getByRole("button", { name: "Add DOOR to the tool palette" }));
  expect(onAdd).toHaveBeenCalledWith("DOOR");
  fireEvent.click(screen.getByRole("button", { name: "Insert TAG" }));
  expect(onInsert).toHaveBeenCalledWith(BLOCKS[1]);
  expect(onClose).toHaveBeenCalled();
});

test("the block palette writes a block to a file (WBLOCK)", async () => {
  const { onWrite } = palette();
  fireEvent.click(await screen.findByRole("button", { name: "Write DOOR to a file…" }));
  expect(onWrite).toHaveBeenCalledWith("DOOR");
});

test("an empty drawing says it has no blocks", async () => {
  render(
    <LanguageProvider>
      <BlockPalette
        load={async () => []}
        thumbnail={async () => ""}
        onInsert={() => {}}
        onWrite={() => {}}
        onAddToPalette={() => {}}
        onClose={() => {}}
      />
    </LanguageProvider>,
  );
  expect(await screen.findByText("This drawing has no blocks")).toBeTruthy();
});

const BLOCKS_THERE: BlockInfo[] = [
  { name: "DOOR", attdefs: [] },
  { name: "WIN", attdefs: [{ tag: "TYPE", prompt: "Type", default: "W1", constant: false }] },
];

test("the Design Center lists another drawing's blocks to insert or redefine", async () => {
  const onInsert = vi.fn();
  const onRedefine = vi.fn(async () => {});
  render(
    <LanguageProvider>
      <DesignCenter
        choose={async () => "/plans/other.dwg"}
        list={async () => BLOCKS_THERE}
        has={(name) => name === "DOOR"}
        onInsert={onInsert}
        onRedefine={onRedefine}
        onClose={() => {}}
      />
    </LanguageProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Open drawing…" }));
  expect(await screen.findByText("/plans/other.dwg")).toBeTruthy();
  // Only a block this drawing has too can be redefined.
  expect(screen.queryByRole("button", { name: "Redefine WIN" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Redefine DOOR" }));
  await waitFor(() => expect(onRedefine).toHaveBeenCalledWith("DOOR", "/plans/other.dwg"));
  // Its attribute definitions come along, so the insert can prompt for them.
  fireEvent.click(screen.getByRole("button", { name: "Insert WIN" }));
  expect(onInsert).toHaveBeenCalledWith(BLOCKS_THERE[1], "/plans/other.dwg");
});

test("the tool palette runs its buttons and keeps what is added or removed", async () => {
  const onRun = vi.fn();
  const onSave = vi.fn(async () => {});
  const items = [{ kind: "block" as const, label: "Door", value: "DOOR" }];
  render(
    <LanguageProvider>
      <ToolPalettes items={items} onRun={onRun} onSave={onSave} onClose={() => {}} />
    </LanguageProvider>,
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Kind" }), { target: { value: "command" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Label" }), { target: { value: "Line" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Block or command" }), { target: { value: "LINE" } });
  fireEvent.click(screen.getByRole("button", { name: "Add" }));
  const line = { kind: "command", label: "Line", value: "LINE" };
  await waitFor(() => expect(onSave).toHaveBeenLastCalledWith([...items, line]));
  fireEvent.click(screen.getByRole("button", { name: "Remove Door" }));
  await waitFor(() => expect(onSave).toHaveBeenLastCalledWith([line]));
  // A button runs and closes the palette.
  fireEvent.click(await screen.findByRole("button", { name: "Line" }));
  expect(onRun).toHaveBeenCalledWith(line);
});

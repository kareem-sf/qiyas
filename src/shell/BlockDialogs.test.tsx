// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { LanguageProvider } from "../i18n";
import {
  AttDefDialog,
  AttributeManager,
  AttributesDialog,
  BlockEditBar,
  CreateBlockDialog,
  promptsFor,
} from "./BlockDialogs";

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

test("a new block needs a name the drawing has not got, without AutoCAD's refused characters", async () => {
  const onSubmit = vi.fn(async () => {});
  wrap(<CreateBlockDialog names={["DOOR"]} onSubmit={onSubmit} onClose={() => {}} />);
  const name = screen.getByRole("textbox", { name: "Block name" });
  const ok = screen.getByRole("button", { name: "Create" }) as HTMLButtonElement;
  expect(ok.disabled).toBe(true);
  fireEvent.change(name, { target: { value: "door" } });
  expect(screen.getByText("The drawing has a block of that name")).toBeTruthy();
  expect(ok.disabled).toBe(true);
  fireEvent.change(name, { target: { value: "A<B" } });
  expect(ok.disabled).toBe(true);
  fireEvent.change(name, { target: { value: "WINDOW" } });
  const convert = screen.getByRole("checkbox", { name: "Replace them with a block reference" }) as HTMLInputElement;
  expect(convert.checked).toBe(true);
  fireEvent.click(convert);
  fireEvent.click(ok);
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("WINDOW", false));
});

test("the attributes dialog asks each prompt and hands back every value by tag", async () => {
  const onSubmit = vi.fn(async () => {});
  wrap(
    <AttributesDialog
      heading="block.attributesTitle"
      fields={[
        { tag: "MARK", prompt: "Door mark", value: "D1" },
        { tag: "SIZE", prompt: "", value: "" },
      ]}
      onSubmit={onSubmit}
      onClose={() => {}}
    />,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Door mark" }), { target: { value: "D7" } });
  // No prompt: the tag is the label. A value may be left empty.
  expect(screen.getByRole("textbox", { name: "SIZE" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "OK" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith({ MARK: "D7", SIZE: "" }));
});

test("inserting asks for every attribute but a constant one", () => {
  expect(
    promptsFor([
      { tag: "MARK", prompt: "Door mark", default: "D1", constant: false },
      { tag: "MAKER", prompt: "", default: "ACME", constant: true },
    ]),
  ).toEqual([{ tag: "MARK", prompt: "Door mark", value: "D1" }]);
});

test("an attribute definition needs a one-word tag", async () => {
  const onSubmit = vi.fn(async () => {});
  wrap(<AttDefDialog height={0.25} onSubmit={onSubmit} onClose={() => {}} />);
  const ok = screen.getByRole("button", { name: "Place" }) as HTMLButtonElement;
  fireEvent.change(screen.getByRole("textbox", { name: "Tag" }), { target: { value: "door mark" } });
  expect(ok.disabled).toBe(true);
  fireEvent.change(screen.getByRole("textbox", { name: "Tag" }), { target: { value: "mark" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Prompt" }), { target: { value: "Door mark" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Default" }), { target: { value: "D1" } });
  fireEvent.click(ok);
  await waitFor(() =>
    expect(onSubmit).toHaveBeenCalledWith({ tag: "MARK", prompt: "Door mark", default: "D1", height: 0.25 }),
  );
});

test("the attribute manager edits the definitions changed and syncs the references", async () => {
  const onSave = vi.fn(async () => {});
  const onSync = vi.fn(async () => {});
  wrap(
    <AttributeManager
      blocks={[
        { name: "DOOR", attdefs: [] },
        {
          name: "TAG",
          attdefs: [
            { tag: "MARK", prompt: "Mark", default: "D1", constant: false },
            { tag: "SIZE", prompt: "Width", default: "900", constant: false },
          ],
        },
      ]}
      onSave={onSave}
      onSync={onSync}
      onClose={() => {}}
    />,
  );
  // Only blocks with attributes are listed.
  const block = screen.getByRole("combobox", { name: "Block" }) as HTMLSelectElement;
  expect([...block.options].map((o) => o.value)).toEqual(["TAG"]);
  fireEvent.change(screen.getByRole("textbox", { name: "Default of SIZE" }), { target: { value: "1000" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
  expect(onSave).toHaveBeenCalledWith("TAG", "SIZE", "Width", "1000");
  fireEvent.click(screen.getByRole("button", { name: "Sync references" }));
  await waitFor(() => expect(onSync).toHaveBeenCalledWith("TAG"));
});

test("the edit bar names the block and saves or discards", () => {
  const onSave = vi.fn();
  const onDiscard = vi.fn();
  wrap(<BlockEditBar name="DOOR" onSave={onSave} onDiscard={onDiscard} />);
  expect(screen.getByText("Editing block DOOR in place")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Save block" }));
  fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
  expect(onSave).toHaveBeenCalled();
  expect(onDiscard).toHaveBeenCalled();
});

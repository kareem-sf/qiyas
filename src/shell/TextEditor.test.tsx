// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { InPlaceText, TextDialog } from "./TextEditor";

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

const styles = ["NOTES", "Standard"];

test("the in-place editor sits at the click, reads either way and places on Enter", () => {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  render(
    <InPlaceText at={[120, 40]} styles={styles} style="NOTES" height={2.5} onSubmit={onSubmit} onCancel={onCancel} />,
  );
  const form = screen.getByRole("form", { name: "Text" });
  expect(form.style.left).toBe("120px");
  expect(form.style.top).toBe("40px");
  const input = screen.getByRole("textbox", { name: "Text" });
  expect(input.getAttribute("dir")).toBe("auto");
  expect(document.activeElement).toBe(input);
  fireEvent.change(input, { target: { value: "قياس D1" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Style" }), { target: { value: "Standard" } });
  fireEvent.change(screen.getByRole("spinbutton", { name: "Height" }), { target: { value: "5" } });
  fireEvent.submit(form);
  expect(onSubmit).toHaveBeenCalledWith({ value: "قياس D1", style: "Standard", height: 5 });
  // Once only: the blur that follows places nothing more.
  fireEvent.blur(input);
  expect(onSubmit).toHaveBeenCalledTimes(1);
  expect(onCancel).not.toHaveBeenCalled();
});

test("Esc or nothing typed places nothing", () => {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  render(
    <InPlaceText at={[0, 0]} styles={styles} style="NOTES" height={2.5} onSubmit={onSubmit} onCancel={onCancel} />,
  );
  fireEvent.submit(screen.getByRole("form", { name: "Text" }));
  expect(onCancel).toHaveBeenCalledTimes(1);
  cleanup();
  render(
    <InPlaceText at={[0, 0]} styles={styles} style="NOTES" height={2.5} onSubmit={onSubmit} onCancel={onCancel} />,
  );
  const input = screen.getByRole("textbox", { name: "Text" });
  fireEvent.change(input, { target: { value: "D1" } });
  fireEvent.keyDown(input, { key: "Escape" });
  expect(onCancel).toHaveBeenCalledTimes(2);
  expect(onSubmit).not.toHaveBeenCalled();
});

test("a click on the drawing places what was typed", () => {
  const onSubmit = vi.fn();
  render(<InPlaceText at={[0, 0]} styles={styles} style="NOTES" height={2.5} onSubmit={onSubmit} onCancel={vi.fn()} />);
  const input = screen.getByRole("textbox", { name: "Text" });
  fireEvent.change(input, { target: { value: "D1" } });
  // Moving to the style stays in the editor.
  fireEvent.blur(input, { relatedTarget: screen.getByRole("combobox", { name: "Style" }) });
  expect(onSubmit).not.toHaveBeenCalled();
  fireEvent.blur(input, { relatedTarget: null });
  expect(onSubmit).toHaveBeenCalledWith({ value: "D1", style: "NOTES", height: 2.5 });
});

test("the MText dialog gives the lines, the style and the height; Ctrl+Enter places", async () => {
  const onSubmit = vi.fn(async () => {});
  const onClose = vi.fn();
  render(
    <TextDialog
      heading="text.newMtext"
      value=""
      multiline
      styles={styles}
      style="NOTES"
      height={3.5}
      onSubmit={onSubmit}
      onClose={onClose}
    />,
  );
  expect(screen.getByRole("heading").textContent).toBe("New MText");
  const area = screen.getByRole("textbox", { name: "Text" });
  expect(area.tagName).toBe("TEXTAREA");
  expect(area.getAttribute("dir")).toBe("auto");
  fireEvent.change(area, { target: { value: "غرفة\nRoom" } });
  fireEvent.keyDown(area, { key: "Enter", ctrlKey: true });
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(onSubmit).toHaveBeenCalledWith({ value: "غرفة\nRoom", style: "NOTES", height: 3.5, tag: null });
});

test("editing a block's attributes edits the one chosen, and an error stays", async () => {
  const onSubmit = vi.fn(async () => {
    throw new Error("refused");
  });
  render(
    <TextDialog
      heading="text.editTitle"
      value=""
      multiline={false}
      attributes={[
        ["MARK", "D9"],
        ["WIDTH", "0.9"],
      ]}
      onSubmit={onSubmit}
      onClose={() => {}}
    />,
  );
  const input = screen.getByRole("textbox", { name: "Text" });
  expect((input as HTMLInputElement).value).toBe("D9");
  fireEvent.change(screen.getByRole("combobox", { name: "Attribute" }), { target: { value: "WIDTH" } });
  expect((input as HTMLInputElement).value).toBe("0.9");
  expect(screen.queryByRole("combobox", { name: "Style" })).toBeNull();
  fireEvent.change(input, { target: { value: "1.0" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("refused"));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ value: "1.0", tag: "WIDTH" }));
});

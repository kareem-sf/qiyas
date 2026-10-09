import { expect, test, vi } from "vitest";
import { leaveDrawing, readChoice } from "./unsaved";

const labels = { yes: "نعم", no: "لا", cancel: "إلغاء" };

test("the dialog's answer reads as save, discard or cancel", () => {
  expect(readChoice("Yes", labels)).toBe("save");
  expect(readChoice("نعم", labels)).toBe("save");
  expect(readChoice("No", labels)).toBe("discard");
  expect(readChoice("لا", labels)).toBe("discard");
  expect(readChoice("Cancel", labels)).toBe("cancel");
  expect(readChoice("إلغاء", labels)).toBe("cancel");
  expect(readChoice("", labels)).toBe("cancel");
});

test("a drawing is left freely when saved, after saving, without saving, or not at all", async () => {
  const steps = (dirty: boolean, choice: "save" | "discard" | "cancel") => ({
    isDirty: vi.fn(async () => dirty),
    ask: vi.fn(async () => choice),
    save: vi.fn(async () => {}),
  });
  const clean = steps(false, "cancel");
  expect(await leaveDrawing(clean)).toBe(true);
  expect(clean.ask).not.toHaveBeenCalled();

  const save = steps(true, "save");
  expect(await leaveDrawing(save)).toBe(true);
  expect(save.save).toHaveBeenCalled();

  const discard = steps(true, "discard");
  expect(await leaveDrawing(discard)).toBe(true);
  expect(discard.save).not.toHaveBeenCalled();

  const cancel = steps(true, "cancel");
  expect(await leaveDrawing(cancel)).toBe(false);
  expect(cancel.save).not.toHaveBeenCalled();

  const failing = { ...steps(true, "save"), save: vi.fn(async () => Promise.reject("disk full")) };
  await expect(leaveDrawing(failing)).rejects.toBe("disk full");
});

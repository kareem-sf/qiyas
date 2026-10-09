import { expect, test } from "vitest";
import { shortcutFor, type Keys } from "./shortcuts";

const keys = (code: string, more: Partial<Keys> = {}): Keys => ({
  code,
  key: "",
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  altKey: false,
  ...more,
});

test("tool letters, Escape and Enter", () => {
  expect(shortcutFor(keys("KeyD"))).toBe("distance");
  expect(shortcutFor(keys("KeyA"))).toBe("area");
  expect(shortcutFor(keys("KeyC"))).toBe("count");
  expect(shortcutFor(keys("KeyS"))).toBe("select");
  expect(shortcutFor(keys("Escape"))).toBe("escape");
  expect(shortcutFor(keys("Enter"))).toBe("finish");
  expect(shortcutFor(keys("NumpadEnter"))).toBe("finish");
  expect(shortcutFor(keys("KeyD", { shiftKey: true }))).toBeNull();
  expect(shortcutFor(keys("KeyQ"))).toBeNull();
});

test("undo, redo and save with Ctrl", () => {
  expect(shortcutFor(keys("KeyZ", { ctrlKey: true }))).toBe("undo");
  expect(shortcutFor(keys("KeyZ", { ctrlKey: true, shiftKey: true }))).toBe("redo");
  expect(shortcutFor(keys("KeyY", { ctrlKey: true }))).toBe("redo");
  expect(shortcutFor(keys("KeyS", { ctrlKey: true }))).toBe("save");
  expect(shortcutFor(keys("Digit9", { ctrlKey: true }))).toBe("commandLine");
  expect(shortcutFor(keys("Digit9"))).toBeNull();
  expect(shortcutFor(keys("KeyS", { ctrlKey: true, shiftKey: true }))).toBeNull();
  expect(shortcutFor(keys("KeyD", { ctrlKey: true }))).toBeNull();
});

test("an Arabic keyboard layout still finds the shortcut", () => {
  expect(shortcutFor(keys("KeyZ", { key: "ئ", ctrlKey: true }))).toBe("undo");
  expect(shortcutFor(keys("KeyD", { key: "ي" }))).toBe("distance");
});

test("Alt and Meta leave the key alone", () => {
  expect(shortcutFor(keys("KeyD", { altKey: true }))).toBeNull();
  expect(shortcutFor(keys("KeyZ", { ctrlKey: true, metaKey: true }))).toBeNull();
});

test("F3, F7, F8 and F10 switch snaps, grid, ortho and polar", () => {
  expect(shortcutFor(keys("F3"))).toBe("osnap");
  expect(shortcutFor(keys("F7"))).toBe("grid");
  expect(shortcutFor(keys("F8"))).toBe("ortho");
  expect(shortcutFor(keys("F10"))).toBe("polar");
  expect(shortcutFor(keys("F8", { ctrlKey: true }))).toBeNull();
  expect(shortcutFor(keys("F9"))).toBeNull();
});

test("Ctrl+Tab, Ctrl+Shift+Tab and Ctrl+W go through and close the file tabs", () => {
  expect(shortcutFor(keys("Tab", { ctrlKey: true }))).toBe("nextTab");
  expect(shortcutFor(keys("Tab", { ctrlKey: true, shiftKey: true }))).toBe("prevTab");
  expect(shortcutFor(keys("KeyW", { ctrlKey: true }))).toBe("closeTab");
  // By the physical key, so an Arabic layout closes too.
  expect(shortcutFor(keys("KeyW", { key: "ص", ctrlKey: true }))).toBe("closeTab");
  expect(shortcutFor(keys("Tab"))).toBeNull();
  expect(shortcutFor(keys("KeyW"))).toBeNull();
  expect(shortcutFor(keys("KeyW", { ctrlKey: true, shiftKey: true }))).toBeNull();
});

// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { blurAfterClick } from "./blurAfterClick";

let undo: () => void;
beforeEach(() => {
  vi.useFakeTimers();
  undo = blurAfterClick();
});
afterEach(() => {
  undo();
  vi.useRealTimers();
  document.body.innerHTML = "";
});

function pointerUp() {
  document.body.dispatchEvent(new Event("pointerup", { bubbles: true }));
  vi.runAllTimers();
}

test("a clicked button lets go of the focus, so Space can't press it again", () => {
  document.body.innerHTML = "<button id=b>Save</button>";
  const b = document.getElementById("b") as HTMLButtonElement;
  b.focus();
  expect(document.activeElement).toBe(b);
  pointerUp();
  expect(document.activeElement).not.toBe(b);
});

test("a button in a dialog and a text box keep the focus", () => {
  document.body.innerHTML = "<dialog open><button id=d>OK</button></dialog><input id=i>";
  const d = document.getElementById("d") as HTMLButtonElement;
  d.focus();
  pointerUp();
  expect(document.activeElement).toBe(d);
  const i = document.getElementById("i") as HTMLInputElement;
  i.focus();
  pointerUp();
  expect(document.activeElement).toBe(i);
});

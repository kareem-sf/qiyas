// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { nextIndex, useRoving } from "./useRoving";

afterEach(cleanup);

test("arrows go round, Home and End jump, other keys do nothing", () => {
  expect(nextIndex("ArrowRight", 0, 3)).toBe(1);
  expect(nextIndex("ArrowRight", 2, 3)).toBe(0);
  expect(nextIndex("ArrowLeft", 0, 3)).toBe(2);
  expect(nextIndex("Home", 2, 3)).toBe(0);
  expect(nextIndex("End", 0, 3)).toBe(2);
  expect(nextIndex("a", 0, 3)).toBeNull();
  expect(nextIndex("ArrowRight", 0, 0)).toBeNull();
});

test("right to left mirrors the arrows", () => {
  expect(nextIndex("ArrowRight", 1, 3, true)).toBe(0);
  expect(nextIndex("ArrowLeft", 1, 3, true)).toBe(2);
});

function Tabs({ onPick }: { onPick(name: string): void }) {
  const roving = useRoving(3, 1);
  return (
    <div role="tablist" {...roving.props}>
      {["a", "b", "c"].map((name, i) => (
        <button key={name} role="tab" tabIndex={roving.tabIndex(i)} onClick={() => onPick(name)}>
          {name}
        </button>
      ))}
    </div>
  );
}

test("one tab is tabbable; the arrows move focus and Space presses", () => {
  const onPick = vi.fn();
  render(<Tabs onPick={onPick} />);
  const tab = (name: string) => screen.getByRole("tab", { name });
  expect(["a", "b", "c"].map((n) => tab(n).tabIndex)).toEqual([-1, 0, -1]);

  tab("b").focus();
  fireEvent.keyDown(tab("b"), { key: "ArrowRight" });
  expect(document.activeElement).toBe(tab("c"));
  expect(["a", "b", "c"].map((n) => tab(n).tabIndex)).toEqual([-1, -1, 0]);
  fireEvent.keyDown(tab("c"), { key: "Home" });
  expect(document.activeElement).toBe(tab("a"));
  fireEvent.keyDown(tab("a"), { key: "End" });
  expect(document.activeElement).toBe(tab("c"));

  const down = fireEvent.keyDown(tab("c"), { key: " " });
  expect(down).toBe(false); // default prevented: the page must not scroll or pan
  expect(onPick).toHaveBeenCalledWith("c");

  // Focus leaves the list: the selected tab is the tabbable one again.
  act(() => tab("c").blur());
  expect(["a", "b", "c"].map((n) => tab(n).tabIndex)).toEqual([-1, 0, -1]);
});

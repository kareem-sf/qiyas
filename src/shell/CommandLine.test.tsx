// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { mergeAliases } from "../tools/aliases";
import type { ParseState } from "../tools/commandParser";
import { CommandLine } from "./CommandLine";

afterEach(cleanup);

const state = (expect: ParseState["expect"] = "command"): ParseState => ({
  expect,
  last: [1, 1],
  aliases: mergeAliases(),
});

function setup(s = state(), prompt = "Command:") {
  const onAction = vi.fn();
  const view = render(<CommandLine state={s} prompt={prompt} onAction={onAction} />);
  const input = screen.getByRole("combobox", { name: prompt }) as HTMLInputElement;
  const type = (text: string) => fireEvent.change(input, { target: { value: text } });
  const key = (k: string) => fireEvent.keyDown(input, { key: k });
  const enter = (text: string) => {
    type(text);
    key("Enter");
  };
  const log = () => [...view.container.querySelectorAll(".command-log li")].map((l) => l.textContent);
  return { onAction, input, type, key, enter, log, view };
}

test("Enter runs the line, echoes it and an empty Enter repeats it", () => {
  const { onAction, input, enter, key, log } = setup();
  enter("di");
  expect(onAction).toHaveBeenLastCalledWith({ type: "tool", tool: "distance" });
  expect(input.value).toBe("");
  expect(log()).toEqual(["Command: di"]);
  key("Enter");
  expect(onAction).toHaveBeenCalledTimes(2);
  expect(onAction).toHaveBeenLastCalledWith({ type: "tool", tool: "distance" });
  expect(log()).toEqual(["Command: di", "Command: di"]);
});

test("errors show inline, in the language of the window, and run nothing", () => {
  const { onAction, enter, log } = setup();
  enter("XYZ");
  expect(onAction).not.toHaveBeenCalled();
  expect(log()).toEqual(["Command: XYZ", "Unknown command: XYZ"]);
});

test("↑ and ↓ go through the lines typed before", () => {
  const { input, enter, key } = setup();
  enter("LAYOFF A");
  enter("Z E");
  key("ArrowUp");
  expect(input.value).toBe("Z E");
  key("ArrowUp");
  expect(input.value).toBe("LAYOFF A");
  key("ArrowUp");
  expect(input.value).toBe("LAYOFF A");
  key("ArrowDown");
  expect(input.value).toBe("Z E");
  key("ArrowDown");
  expect(input.value).toBe("");
});

test("typing lists the matching commands; ↓ and Enter run one, Tab fills it in", () => {
  const { onAction, input, type, key } = setup();
  type("lay");
  const names = () => screen.getAllByRole("option").map((o) => o.firstChild?.textContent);
  expect(names()).toEqual(["LAYALL", "LAYER", "LAYISO", "LAYOFF", "LAYON", "LAYSTATE", "LAYTRANS"]);
  expect(input.getAttribute("aria-expanded")).toBe("true");
  key("ArrowDown");
  key("ArrowDown");
  key("ArrowDown");
  expect(screen.getByRole("option", { selected: true }).textContent).toBe("LAYISO");
  key("Tab");
  expect(input.value).toBe("LAYISO ");
  expect(screen.queryByRole("listbox")).toBeNull();

  type("LAYA");
  key("ArrowDown");
  key("Enter");
  expect(onAction).toHaveBeenLastCalledWith({ type: "showAll" });

  // Escape closes the list first, then cancels.
  type("Z");
  key("Escape");
  expect(screen.queryByRole("listbox")).toBeNull();
  expect(onAction).toHaveBeenCalledTimes(1);
  key("Escape");
  expect(onAction).toHaveBeenLastCalledWith({ type: "cancel" });
});

test("at a point prompt, a typed point is an answer, not a command to repeat", () => {
  const { onAction, enter, key, log } = setup(state("point"), "Specify next point:");
  enter("@2,0");
  expect(onAction).toHaveBeenLastCalledWith({ type: "point", p: [3, 1] });
  key("Enter");
  expect(onAction).toHaveBeenLastCalledWith({ type: "finish" });
  expect(log()).toEqual(["Specify next point: @2,0", "Specify next point:"]);
});

test("Enter never repeats COUNT <block> or AI <text>; tools and UNDO still repeat", () => {
  const { onAction, enter, key } = setup();
  enter("COUNT DOOR");
  key("Enter");
  enter("AI how long is the wall?");
  key("Enter");
  expect(onAction.mock.calls.map(([a]) => a.type)).toEqual(["count", "ai"]);

  enter("U");
  key("Enter");
  enter("AA");
  key("Enter");
  expect(onAction.mock.calls.slice(2).map(([a]) => a)).toEqual([
    { type: "run", what: "undo" },
    { type: "run", what: "undo" },
    { type: "tool", tool: "area" },
    { type: "tool", tool: "area" },
  ]);
});

test("history keeps the last 50 lines", () => {
  const { input, enter, key } = setup();
  for (let i = 1; i <= 55; i++) enter(`LAYOFF L${i}`);
  key("ArrowUp");
  expect(input.value).toBe("LAYOFF L55");
  for (let i = 0; i < 60; i++) key("ArrowUp");
  expect(input.value).toBe("LAYOFF L6");
});

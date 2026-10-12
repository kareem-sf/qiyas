import { expect, test } from "vitest";
import type { RibbonLayout } from "../api";
import { buttonKeys, DEFAULT_RIBBON, editTree, layoutOf } from "./ribbonLayout";

/** The core's default, cut down: Home (file with Audit, history) and Manage (Run script). */
const CORE: RibbonLayout = {
  tabs: [
    {
      id: "home",
      label: "home",
      groups: [
        {
          id: "file",
          label: "ribbon.file",
          buttons: [
            { kind: "action", command: "open", label: "open" },
            { kind: "action", command: "audit", label: "audit.button" },
          ],
        },
        { id: "history", label: "ribbon.history", buttons: [{ kind: "action", command: "undo", label: "undo" }] },
      ],
    },
    {
      id: "manage",
      label: "manage",
      groups: [{ id: "scripts", label: "ribbon.scripts", buttons: [{ kind: "action", command: "runScript", label: "script.run" }] }],
    },
  ],
};

test("the window's default has the seven tabs, and Manage has Audit, Purge, Options, Customize and Aliases", () => {
  expect(DEFAULT_RIBBON.tabs.map((t) => t.id)).toEqual(["home", "measure", "annotate", "insert", "view", "manage", "output"]);
  const manage = DEFAULT_RIBBON.tabs.find((t) => t.id === "manage")!;
  expect(manage.groups.flatMap((g) => g.buttons.map((b) => b.command))).toEqual([
    "runScript",
    "audit",
    "purge",
    "options",
    "customize",
    "aliases",
  ]);
});

test("a saved layout shows in its order, and what the core's default has that it leaves out is hidden", () => {
  const saved: RibbonLayout = {
    tabs: [
      {
        id: "home",
        label: "home",
        groups: [{ id: "history", label: "ribbon.history", buttons: [{ kind: "action", command: "undo", label: "undo" }] }],
      },
    ],
  };
  const tree = editTree(CORE, saved);
  expect(tree.map((t) => [t.id, t.hidden])).toEqual([
    ["home", false],
    ["manage", true],
  ]);
  expect(tree[0].groups.map((g) => [g.id, g.hidden])).toEqual([
    ["history", false],
    ["file", true],
  ]);
  // Nothing hidden changes: the same layout back.
  expect(layoutOf(tree)).toEqual(saved);
});

test("with no saved layout the tree is the window's default, less the buttons the core lacks", () => {
  const tree = editTree(CORE, null);
  const keys = (id: string) => tree.find((t) => t.id === id)!.groups.flatMap((g) => g.buttons.filter((b) => !b.hidden).map((b) => b.command));
  // Options, Customize and Aliases are the window's own: the core can't keep them in a layout
  // (nor Purge, which this cut-down core lacks).
  expect(keys("manage")).toEqual(["runScript", "audit"]);
  // Audit, beside Open in the core's data, is hidden there (it shows on Manage in the window's).
  const file = tree[0].groups.find((g) => g.id === "file")!;
  expect(file.buttons.find((b) => b.command === "audit")!.hidden).toBe(true);
  // Every button the layout saves is one the core knows.
  const known = buttonKeys(CORE);
  for (const k of buttonKeys(layoutOf(tree)!)) expect(known.has(k)).toBe(true);
});

test("hidden tabs, groups and buttons are left out; empty groups and tabs go; no tab left is null", () => {
  const tree = editTree(CORE, CORE);
  tree[0].groups[0].buttons[1].hidden = true;
  tree[0].groups[1].buttons[0].hidden = true;
  tree[1].hidden = true;
  expect(layoutOf(tree)).toEqual({
    tabs: [
      {
        id: "home",
        label: "home",
        groups: [{ id: "file", label: "ribbon.file", buttons: [{ kind: "action", command: "open", label: "open" }] }],
      },
    ],
  });
  tree[0].hidden = true;
  expect(layoutOf(tree)).toBeNull();
});

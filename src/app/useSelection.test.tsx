// @vitest-environment jsdom
/** `useSelection`'s whole-selection read for the properties palette's sums: every picked entity is
 *  asked for, and one that cannot be read leaves none (never a short sum). */
import { renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { EntityInfo } from "../api";
import { useSelection } from "./useSelection";

const asked = vi.hoisted(() => ({ handles: [] as number[], failing: null as number | null }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    if (cmd !== "entity_info") return null;
    const handle = args!.handle as number;
    asked.handles.push(handle);
    if (handle === asked.failing) throw new Error("gone");
    return { handle, kind: "Line", length: handle } as unknown as EntityInfo;
  }),
}));

const deps = (picks: number[]) =>
  ({
    info: null,
    tool: "select",
    measure: { highlighted: null, picks },
    batch: null,
    redraw: async () => {},
    runLayerCommand: () => {},
  }) as unknown as Parameters<typeof useSelection>[0];

test("two or more picks are all read, in order, beyond one batch of 64", async () => {
  asked.handles = [];
  asked.failing = null;
  const picks = Array.from({ length: 150 }, (_, i) => i + 1);
  const { result } = renderHook(() => useSelection(deps(picks)));
  await waitFor(() => expect(result.current.pickedMany).not.toBeNull());
  expect(result.current.pickedMany!.map((e) => e.handle)).toEqual(picks);
  expect(asked.handles).toHaveLength(150);
});

test("one pick is no multiple selection, and a pick that cannot be read leaves none", async () => {
  asked.handles = [];
  const one = renderHook(() => useSelection(deps([5])));
  await new Promise((r) => setTimeout(r, 20));
  expect(one.result.current.pickedMany).toBeNull();
  expect(asked.handles).toEqual([]);

  asked.failing = 2;
  const bad = renderHook(() => useSelection(deps([1, 2, 3])));
  await new Promise((r) => setTimeout(r, 20));
  expect(bad.result.current.pickedMany).toBeNull();
});

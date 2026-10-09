import { describe, expect, it } from "vitest";
import type { Command } from "../api";
import type { Point } from "../viewport/camera";
import {
  initialModifyState,
  modifyOptions,
  modifyPreview,
  modifyReducer,
  takesModifyNumber,
  type ModifyInput,
  type ModifyKind,
  type ModifyState,
} from "./modifyTools";

const t = (handles: number[]) => handles.map((handle) => ({ handle, chain: [] }));

/** Feeds `inputs` in order; returns the last state and every command made on the way. */
function feed(s: ModifyState, inputs: ModifyInput[]): { s: ModifyState; commands: Command[] } {
  const commands: Command[] = [];
  for (const input of inputs) {
    s = modifyReducer(s, input);
    if (s.command) commands.push(s.command);
  }
  return { s, commands };
}

const pt = (p: Point): ModifyInput => ({ type: "point", p });
const picked = (...handles: number[]): ModifyInput => ({ type: "picked", handles });
const finish: ModifyInput = { type: "finish" };
const num = (value: number): ModifyInput => ({ type: "number", value });
const opt = (option: string): ModifyInput => ({ type: "option", option });

describe("selecting", () => {
  it("collects picks once each until Enter, then asks for points", () => {
    const { s } = feed(initialModifyState("move", []), [picked(1), picked(2, 1)]);
    expect(s.phase).toBe("select");
    expect(s.handles).toEqual([1, 2]);
    expect(s.hintArgs).toEqual({ n: "2" });
    const next = modifyReducer(s, finish);
    expect(next.phase).toBe("points");
    expect(next.hint).toBe("modify.prompt.base");
  });

  it("Enter with nothing picked keeps asking", () => {
    const s = modifyReducer(initialModifyState("move", []), finish);
    expect(s.phase).toBe("select");
    expect(s.command).toBeNull();
  });

  it("starts at the points with a selection made before", () => {
    const s = initialModifyState("rotate", [7]);
    expect(s.phase).toBe("points");
    expect(s.handles).toEqual([7]);
  });

  it("points are ignored while selecting", () => {
    const s = modifyReducer(initialModifyState("move", []), pt([1, 1]));
    expect(s.points).toEqual([]);
  });
});

describe("move and copy", () => {
  it("move: base, second point, then a new selection", () => {
    const { s, commands } = feed(initialModifyState("move", [3]), [pt([1, 1]), pt([4, 5])]);
    expect(commands).toEqual([{ type: "modify", kind: "move", targets: t([3]), delta: [3, 4] }]);
    expect(s.phase).toBe("select");
    expect(s.handles).toEqual([]);
  });

  it("copy keeps placing copies from the same base until Enter; A makes several in a row", () => {
    const { s, commands } = feed(initialModifyState("copy", [3]), [
      pt([0, 0]),
      pt([1, 0]),
      opt("A"),
      num(3),
      pt([0, 2]),
    ]);
    expect(commands).toEqual([
      { type: "modify", kind: "copy", targets: t([3]), delta: [1, 0], count: 1 },
      { type: "modify", kind: "copy", targets: t([3]), delta: [0, 2], count: 3 },
    ]);
    expect(s.phase).toBe("points");
    const done = modifyReducer(s, finish);
    expect(done.phase).toBe("select");
    expect(done.handles).toEqual([]);
  });

  it("a copy count must be a whole number of 1 or more", () => {
    const s = feed(initialModifyState("copy", [3]), [pt([0, 0]), opt("A"), num(2.5)]).s;
    expect(s.count).toBe(1);
    expect(s.asking).toBe("count");
  });
});

describe("rotate and scale", () => {
  it("rotate by a typed angle, or by a click from the base point", () => {
    const typed = feed(initialModifyState("rotate", [3]), [pt([1, 1]), num(90)]).commands;
    expect(typed).toEqual([
      { type: "modify", kind: "rotate", targets: t([3]), base: [1, 1], angle: 90, copy: false },
    ]);
    const clicked = feed(initialModifyState("rotate", [3]), [pt([0, 0]), opt("C"), pt([0, 2])]).commands;
    expect(clicked).toEqual([
      { type: "modify", kind: "rotate", targets: t([3]), base: [0, 0], angle: 90, copy: true },
    ]);
  });

  it("scale by a typed factor, or by the distance clicked from the base point", () => {
    const typed = feed(initialModifyState("scale", [3]), [pt([0, 0]), num(2)]).commands;
    expect(typed[0]).toMatchObject({ kind: "scale", factor: 2, copy: false });
    const clicked = feed(initialModifyState("scale", [3]), [pt([0, 0]), pt([3, 4])]).commands;
    expect(clicked[0]).toMatchObject({ kind: "scale", base: [0, 0], factor: 5 });
    expect(takesModifyNumber(feed(initialModifyState("scale", [3]), [pt([0, 0])]).s)).toBe("distance");
    expect(takesModifyNumber(feed(initialModifyState("rotate", [3]), [pt([0, 0])]).s)).toBe("angle");
  });
});

describe("mirror", () => {
  it("asks whether to erase the originals; Enter keeps them", () => {
    const base = feed(initialModifyState("mirror", [3]), [pt([0, 0]), pt([0, 1])]).s;
    expect(base.hint).toBe("modify.prompt.mirrorErase");
    expect(modifyOptions(base)).toEqual(["Y", "N"]);
    expect(modifyReducer(base, finish).command).toMatchObject({ kind: "mirror", p1: [0, 0], p2: [0, 1], keep: true });
    expect(modifyReducer(base, opt("Y")).command).toMatchObject({ keep: false });
  });
});

describe("stretch", () => {
  it("takes a crossing window, looks up what it touches, then base and second points", () => {
    let s = initialModifyState("stretch", [9]);
    expect(s.phase).toBe("window");
    expect(s.handles).toEqual([]);
    s = feed(s, [pt([5, 5]), pt([1, 2])]).s;
    expect(s.seek).toEqual([
      [1, 2],
      [5, 5],
    ]);
    s = modifyReducer(s, picked(4, 6));
    expect(s.seek).toBeNull();
    expect(s.phase).toBe("points");
    expect(s.handles).toEqual([4, 6]);
    const { commands } = feed(s, [pt([0, 0]), pt([2, 0])]);
    expect(commands).toEqual([
      {
        type: "modify",
        kind: "stretch",
        targets: t([4, 6]),
        window: [
          [1, 2],
          [5, 5],
        ],
        delta: [2, 0],
      },
    ]);
  });

  it("a dragged box is the window too; nothing inside asks again", () => {
    let s = modifyReducer(initialModifyState("stretch", []), {
      type: "box",
      bounds: [
        [0, 0],
        [1, 1],
      ],
    });
    expect(s.seek).not.toBeNull();
    s = modifyReducer(s, picked());
    expect(s.phase).toBe("window");
    expect(s.hint).toBe("modify.prompt.nothing");
  });
});

describe("align", () => {
  it("one pair moves: Enter after the first pair", () => {
    const { commands } = feed(initialModifyState("align", [3]), [pt([0, 0]), pt([5, 5]), finish]);
    expect(commands).toEqual([{ type: "modify", kind: "align", targets: t([3]), pairs: [[[0, 0], [5, 5]]] }]);
  });

  it("two pairs", () => {
    const { commands } = feed(initialModifyState("align", [3]), [pt([0, 0]), pt([5, 5]), pt([1, 0]), pt([5, 6])]);
    expect(commands[0]).toMatchObject({
      pairs: [
        [[0, 0], [5, 5]],
        [[1, 0], [5, 6]],
      ],
    });
  });
});

describe("array", () => {
  it("rectangular: rows, columns, then the gaps", () => {
    const s = initialModifyState("array", [3]);
    expect(modifyOptions(s)).toEqual(["R", "P"]);
    const { commands } = feed(s, [opt("R"), num(2), num(3), num(4), num(-10)]);
    expect(commands).toEqual([
      { type: "modify", kind: "array_rect", targets: t([3]), rows: 2, cols: 3, dy: 4, dx: -10 },
    ]);
  });

  it("Enter picks rectangular", () => {
    const s = modifyReducer(initialModifyState("array", [3]), finish);
    expect(s.asking).toBe("rows");
  });

  it("polar: centre, items, and the angle between them (Enter fills the circle)", () => {
    const typed = feed(initialModifyState("array", [3]), [opt("P"), pt([0, 0]), num(4), num(30)]).commands;
    expect(typed).toEqual([
      { type: "modify", kind: "array_polar", targets: t([3]), centre: [0, 0], count: 4, angle: 30 },
    ]);
    const full = feed(initialModifyState("array", [3]), [opt("P"), pt([0, 0]), num(4), finish]).commands;
    expect(full[0]).toMatchObject({ count: 4, angle: 90 });
  });

  it("a polar array needs 2 items or more", () => {
    const s = feed(initialModifyState("array", [3]), [opt("P"), pt([0, 0]), num(1)]).s;
    expect(s.asking).toBe("items");
  });
});

describe("erase and explode", () => {
  it.each(["erase", "explode"] as ModifyKind[])("%s runs on Enter, or at once on a selection made before", (kind) => {
    const want: Command =
      kind === "erase"
        ? { type: "delete_entities", targets: t([3]) }
        : { type: "modify", kind: "explode", targets: t([3]) };
    expect(feed(initialModifyState(kind, []), [picked(3), finish]).commands).toEqual([want]);
    expect(initialModifyState(kind, [3]).command).toEqual(want);
  });
});

describe("previews", () => {
  it("draws a line from the last point to the cursor, and the stretch window", () => {
    const s = feed(initialModifyState("move", [3]), [pt([0, 0])]).s;
    expect(Array.from(modifyPreview(s, [2, 3]))).toEqual([0, 0, 2, 3]);
    const w = feed(initialModifyState("stretch", []), [pt([0, 0])]).s;
    expect(modifyPreview(w, [2, 1]).length).toBe(16);
    expect(modifyPreview(initialModifyState("move", []), [1, 1]).length).toBe(0);
  });
});

describe("dimensions and tables", () => {
  it("tools that can't turn them drop them from what is picked and say how many", () => {
    const s = initialModifyState("rotate", [1, 2, 3], { 2: "Dimension", 3: "Table" });
    expect(s.handles).toEqual([1]);
    expect(s.skipped).toBe(2);
    const picked = modifyReducer(initialModifyState("mirror", []), {
      type: "picked",
      handles: [4, 5],
      kinds: ["Line", "Dimension"],
    });
    expect(picked.handles).toEqual([4]);
    expect(picked.skipped).toBe(1);
    // The count is said once, by the step that dropped them.
    expect(modifyReducer(picked, { type: "picked", handles: [6], kinds: ["Line"] }).skipped).toBe(0);
  });

  it("move, copy, erase and explode keep them", () => {
    for (const kind of ["move", "copy", "erase", "explode"] as ModifyKind[]) {
      const s = initialModifyState(kind, [1, 2], { 2: "Dimension" });
      expect(s.skipped).toBe(0);
    }
    expect(initialModifyState("move", [1, 2], { 2: "Dimension" }).handles).toEqual([1, 2]);
  });

  it("a stretch window drops them too", () => {
    let s = initialModifyState("stretch", []);
    s = modifyReducer(s, { type: "box", bounds: [[0, 0], [1, 1]] });
    s = modifyReducer(s, { type: "picked", handles: [1, 2], kinds: ["Table", "Line"] });
    expect(s.handles).toEqual([2]);
    expect(s.skipped).toBe(1);
  });
});

import { expect, test } from "vitest";
import type { Point } from "../viewport/camera";
import {
  initialTextState,
  isTextTool,
  takesTextNumber,
  textOptions,
  textPreview,
  textReducer,
  type TextInput,
  type TextKind,
  type TextRequest,
} from "./textTools";

const pt = (p: Point): TextInput => ({ type: "point", p });
const opt = (option: string): TextInput => ({ type: "option", option });
const num = (value: number): TextInput => ({ type: "number", value });

/** Every request the inputs made, in order. */
function requests(kind: TextKind, inputs: TextInput[]): TextRequest[] {
  let s = initialTextState(kind);
  const out: TextRequest[] = [];
  for (const i of inputs) {
    s = textReducer(s, i);
    if (s.request) out.push(s.request);
  }
  return out;
}

test("the text tools are told apart from the others", () => {
  expect(isTextTool("text")).toBe(true);
  expect(isTextTool("mtext")).toBe(true);
  expect(isTextTool("line")).toBe(false);
});

test("each click places a text, with the height and rotation typed before it", () => {
  expect(requests("text", [pt([1, 2]), opt("H"), num(5), opt("R"), num(90), pt([3, 4])])).toEqual([
    { kind: "text", at: [1, 2], rotation: 0, height: null },
    { kind: "text", at: [3, 4], rotation: 90, height: 5 },
  ]);
});

test("TEXT asks for its options' numbers and says so", () => {
  let s = initialTextState("text");
  expect(s.hint).toBe("text.prompt.start");
  expect(textOptions(s)).toEqual(["H", "R"]);
  s = textReducer(s, opt("R"));
  expect(s.hint).toBe("text.prompt.rotation");
  expect(takesTextNumber(s)).toBe("angle");
  expect(textOptions(s)).toEqual([]);
  s = textReducer(s, num(-30));
  expect(s.rotation).toBe(-30);
  expect(s.hintArgs).toEqual({ r: "-30" });
  s = textReducer(s, opt("H"));
  expect(takesTextNumber(s)).toBe("distance");
  // A height must be above 0: the prompt stays.
  s = textReducer(s, num(0));
  expect(s.asking).toBe("H");
});

test("MTEXT takes a box by two corners in any order: its top left corner and width", () => {
  expect(requests("mtext", [pt([10, 0]), pt([2, 6]), opt("H"), num(3.5), pt([0, 0]), pt([4, -2])])).toEqual([
    { kind: "mtext", at: [2, 6], width: 8, height: null },
    { kind: "mtext", at: [0, 0], width: 4, height: 3.5 },
  ]);
  expect(textOptions(initialTextState("mtext"))).toEqual(["H"]);
});

test("MTEXT shows its box to the cursor and Enter drops the first corner", () => {
  let s = textReducer(initialTextState("mtext"), pt([0, 0]));
  expect(s.hint).toBe("text.prompt.mtextOther");
  expect(Array.from(textPreview(s, [2, 1]))).toEqual([0, 0, 2, 0, 2, 0, 2, 1, 2, 1, 0, 1, 0, 1, 0, 0]);
  // A box with no width is no box.
  expect(textReducer(s, pt([0, 5])).request).toBeNull();
  s = textReducer(s, { type: "finish" });
  expect(s.corner).toBeNull();
  expect(s.hint).toBe("text.prompt.mtextFirst");
  expect(textPreview(s, [2, 1]).length).toBe(0);
});

test("TABLE and FIELD place where clicked: a table's corner, a field's text", () => {
  expect(isTextTool("table")).toBe(true);
  expect(isTextTool("field")).toBe(true);
  expect(requests("table", [pt([1, 2]), pt([3, 4])])).toEqual([
    { kind: "table", at: [1, 2] },
    { kind: "table", at: [3, 4] },
  ]);
  expect(initialTextState("table").hint).toBe("table.prompt.place");
  expect(textOptions(initialTextState("table"))).toEqual([]);
  expect(requests("field", [opt("H"), num(2), pt([5, 6])])).toEqual([{ kind: "field", at: [5, 6], height: 2 }]);
  expect(initialTextState("field").hint).toBe("table.prompt.field");
  expect(textOptions(initialTextState("field"))).toEqual(["H"]);
});

test("INSERT places a block where clicked, turned by R and scaled by S; ATTDEF takes H", () => {
  expect(isTextTool("insertBlock")).toBe(true);
  expect(isTextTool("attdef")).toBe(true);
  const s = initialTextState("insertBlock");
  expect(s.hint).toBe("block.prompt.insert");
  expect(textOptions(s)).toEqual(["R", "S"]);
  expect(takesTextNumber(textReducer(s, opt("S")))).toBe("distance");
  expect(
    requests("insertBlock", [pt([1, 2]), opt("R"), num(90), opt("S"), num(2), pt([3, 4]), opt("S"), num(0)]),
  ).toEqual([
    { kind: "insertBlock", at: [1, 2], rotation: 0, scale: 1 },
    { kind: "insertBlock", at: [3, 4], rotation: 90, scale: 2 },
  ]);
  expect(textOptions(initialTextState("attdef"))).toEqual(["H"]);
  expect(requests("attdef", [opt("H"), num(0.5), pt([7, 8])])).toEqual([{ kind: "attdef", at: [7, 8], height: 0.5 }]);
});

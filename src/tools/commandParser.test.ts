import { expect, test } from "vitest";
import { mergeAliases } from "./aliases";
import { normalizeDigits, parse, suggest, tokenize, type ParseState } from "./commandParser";

const aliases = mergeAliases();
const at = (expect: ParseState["expect"], last: [number, number] | null = null): ParseState => ({
  expect,
  last,
  aliases,
});
const cmd = at("command");

test("aliases and full names start the same tool, in any case", () => {
  for (const line of ["DI", "dist", "Distance"]) {
    expect(parse(line, cmd)).toEqual({ type: "tool", tool: "distance" });
  }
  expect(parse("aa", cmd)).toEqual({ type: "tool", tool: "area" });
  expect(parse("geom", cmd)).toEqual({ type: "tool", tool: "geometry" });
  expect(parse("ID", cmd)).toEqual({ type: "tool", tool: "idPoint" });
  expect(parse("u", cmd)).toEqual({ type: "run", what: "undo" });
  expect(parse("redo", cmd)).toEqual({ type: "run", what: "redo" });
});

test("the user's aliases win over the built-in ones and may carry words", () => {
  const mine = mergeAliases({ di: "AREA", lo: "LAYOFF A-WALL" });
  expect(parse("DI", { ...cmd, aliases: mine })).toEqual({ type: "tool", tool: "area" });
  expect(parse("lo A-DOOR", { ...cmd, aliases: mine })).toEqual({
    type: "layers",
    hide: true,
    names: ["A-WALL", "A-DOOR"],
  });
});

test("an unknown command says which", () => {
  expect(parse("XYZ 1 2", cmd)).toEqual({ type: "error", key: "cmd.unknown", args: { name: "XYZ" } });
});

test("absolute points, with Arabic-Indic digits, ، and ٫", () => {
  expect(parse("10,20", at("point"))).toEqual({ type: "point", p: [10, 20] });
  expect(parse(" -1.5 , 2 ", at("point"))).toEqual({ type: "point", p: [-1.5, 2] });
  expect(parse("١٢٫٥،٣", at("point"))).toEqual({ type: "point", p: [12.5, 3] });
  expect(parse("۴,۵", at("point"))).toEqual({ type: "point", p: [4, 5] });
  expect(normalizeDigits("٠١٢٣٤٥٦٧٨٩")).toBe("0123456789");
});

test("@dx,dy is relative to the last point", () => {
  expect(parse("@3,-4", at("point", [10, 10]))).toEqual({ type: "point", p: [13, 6] });
  expect(parse("@3,4", at("point"))).toEqual({ type: "error", key: "cmd.noLastPoint" });
});

test("@distance<angle is polar from the last point, in degrees", () => {
  const r = parse("@5<45", at("point", [1, 1]));
  expect(r.type).toBe("point");
  const [x, y] = (r as { p: [number, number] }).p;
  expect(x).toBeCloseTo(1 + 5 / Math.SQRT2, 9);
  expect(y).toBeCloseTo(1 + 5 / Math.SQRT2, 9);
  const up = parse("@٢<٩٠", at("point", [0, 0])) as { p: [number, number] };
  expect(up.p[0]).toBeCloseTo(0, 9);
  expect(up.p[1]).toBeCloseTo(2, 9);
});

test("a malformed point is an error; a word at a point prompt is a command", () => {
  expect(parse("1,2,3", at("point"))).toEqual({ type: "error", key: "cmd.badPoint" });
  expect(parse("5", at("point"))).toEqual({ type: "error", key: "cmd.badPoint" });
  expect(parse("@5<", at("point", [0, 0]))).toEqual({ type: "error", key: "cmd.badPoint" });
  expect(parse("AA", at("point"))).toEqual({ type: "tool", tool: "area" });
});

test("an empty line repeats at the command prompt and finishes at a point prompt", () => {
  expect(parse("", cmd)).toEqual({ type: "repeat" });
  expect(parse("  ", at("point", [0, 0]))).toEqual({ type: "finish" });
});

test("a number answers the count and step prompts", () => {
  expect(parse("٥", at("count"))).toEqual({ type: "number", value: 5 });
  expect(parse("1", at("count"))).toEqual({ type: "error", key: "cmd.badCount" });
  expect(parse("2.5", at("count"))).toEqual({ type: "error", key: "cmd.badCount" });
  expect(parse("0٫6", at("step"))).toEqual({ type: "number", value: 0.6 });
  expect(parse("0", at("step"))).toEqual({ type: "error", key: "cmd.badStep" });
  // A command still runs there.
  expect(parse("ESC", at("count"))).toEqual({ type: "cancel" });
});

test("DIVIDE and MEASURE take their number up front", () => {
  expect(parse("DIV", cmd)).toEqual({ type: "tool", tool: "divide" });
  expect(parse("divide 4", cmd)).toEqual({ type: "tool", tool: "divide", preset: 4 });
  expect(parse("DIV 1", cmd)).toEqual({ type: "error", key: "cmd.badCount" });
  expect(parse("ME 0.6", cmd)).toEqual({ type: "tool", tool: "measureAlong", preset: 0.6 });
  expect(parse("measure x", cmd)).toEqual({ type: "error", key: "cmd.badStep" });
});

test("COUNT with a block counts it; without one it starts the tool", () => {
  expect(parse("COUNT DOOR-90", cmd)).toEqual({ type: "count", block: "DOOR-90" });
  expect(parse('count "Door 1"', cmd)).toEqual({ type: "count", block: "Door 1" });
  expect(parse("count", cmd)).toEqual({ type: "tool", tool: "count" });
});

test("LAYOFF and LAYON take names, quoted when they hold spaces", () => {
  expect(parse('LAYOFF A-WALL "FURN 2" x', cmd)).toEqual({
    type: "layers",
    hide: true,
    names: ["A-WALL", "FURN 2", "x"],
  });
  expect(parse("layon A-DOOR", cmd)).toEqual({ type: "layers", hide: false, names: ["A-DOOR"] });
  expect(parse("LAYOFF", cmd)).toEqual({ type: "error", key: "cmd.needLayer", args: { name: "LAYOFF" } });
  expect(parse("LAYISO Ground Floor", cmd)).toEqual({ type: "isolate", name: "Ground Floor" });
  expect(parse("LAYALL", cmd)).toEqual({ type: "showAll" });
  expect(tokenize('a "b c" "d')).toEqual(["a", "b c", "d"]);
});

test("ZOOM takes E, W, IN or OUT", () => {
  expect(parse("Z E", cmd)).toEqual({ type: "zoom", how: "extents" });
  expect(parse("zoom extents", cmd)).toEqual({ type: "zoom", how: "extents" });
  expect(parse("z w", cmd)).toEqual({ type: "tool", tool: "zoomWindow" });
  expect(parse("Z IN", cmd)).toEqual({ type: "zoom", how: "in" });
  expect(parse("Z out", cmd)).toEqual({ type: "zoom", how: "out" });
  expect(parse("Z", cmd)).toEqual({ type: "error", key: "cmd.zoomOption" });
});

test("AI sends the rest of the line as typed", () => {
  expect(parse("ai  كم طول   الجدار؟ ", cmd)).toEqual({ type: "ai", text: "كم طول   الجدار؟" });
  expect(parse("AI", cmd)).toEqual({ type: "error", key: "cmd.needText", args: { name: "AI" } });
});

test("file, view and drafting commands", () => {
  const runs = {
    OPEN: "open",
    SAVE: "save",
    SAVEAS: "saveAs",
    EXPORT: "export",
    BOQ: "importBoq",
    COMPARE: "compare",
    PDFATTACH: "attachPdf",
    IMAGEATTACH: "attachImage",
    XREF: "xrefs",
    CALIBRATE: "calibrate",
    EXTRACT: "extract",
  };
  for (const [line, what] of Object.entries(runs)) expect(parse(line, cmd)).toEqual({ type: "run", what });
  expect(parse("ortho", cmd)).toEqual({ type: "toggle", toggle: "ortho" });
  expect(parse("OSNAP", cmd)).toEqual({ type: "toggle", toggle: "osnap" });
  expect(parse("SHEET Layout 1", cmd)).toEqual({ type: "sheet", name: "Layout 1" });
});

test("suggestions list commands, then aliases, the exact match first", () => {
  expect(suggest("la", aliases).map((s) => s.name)).toEqual(["LAYALL", "LAYISO", "LAYOFF", "LAYON"]);
  expect(suggest("di", aliases)).toEqual([
    { name: "DI", target: "DIST" },
    { name: "DIST" },
    { name: "DIVIDE" },
    { name: "DISTANCE", target: "DIST" },
    { name: "DIV", target: "DIVIDE" },
  ]);
  expect(suggest("LAYOFF A", aliases)).toEqual([]);
  expect(suggest("", aliases)).toEqual([]);
});

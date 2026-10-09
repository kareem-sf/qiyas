/** The command line's grammar: one typed line to one action. Pure; the app maps each action to
 *  what the ribbon and the keyboard already do.
 *
 *  Command names and aliases match in any case. Numbers take Arabic-Indic digits and the Arabic
 *  decimal separator (٫); points take `,` or `،` between x and y. Points are in drawing units:
 *  `x,y` absolute, `@dx,dy` from the last point, `@distance<angle` (degrees) from the last point. */
import type { Point } from "../viewport/camera";
import type { Tool } from "../viewport/Viewport";
import { COMMANDS, type CommandName } from "./aliases";
import type { DraftingToggle } from "./shortcuts";

/** What the command line is waiting for: a command, a point (which a command may replace), or the
 *  number of parts or the step (metres) of a divide or measure along. */
export type Expect = "command" | "point" | "count" | "step";

export type ParseState = {
  expect: Expect;
  /** The last point placed or typed, for `@`; null when there is none yet. */
  last: Point | null;
  /** Alias to command, upper-case aliases (`mergeAliases`). */
  aliases: Record<string, string>;
};

/** One of the app's own actions, run from the command line. */
export type Run =
  | "open"
  | "save"
  | "saveAs"
  | "export"
  | "importBoq"
  | "compare"
  | "undo"
  | "redo"
  | "attachPdf"
  | "attachImage"
  | "xrefs"
  | "calibrate"
  | "extract";

export type Action =
  /** An empty line where a command is expected: run the last command again. */
  | { type: "repeat" }
  /** An empty line where a point is expected: finish the measure. */
  | { type: "finish" }
  | { type: "cancel" }
  | { type: "point"; p: Point }
  /** The number of parts, or the step in metres. */
  | { type: "number"; value: number }
  /** Starts a tool; `preset` is the divide's number of parts or the measure along's step (m),
   *  used once the entity is clicked. */
  | { type: "tool"; tool: Tool; preset?: number }
  | { type: "count"; block: string }
  | { type: "layers"; hide: boolean; names: string[] }
  | { type: "isolate"; name: string }
  | { type: "showAll" }
  | { type: "zoom"; how: "extents" | "in" | "out" }
  | { type: "toggle"; toggle: DraftingToggle }
  | { type: "sheet"; name: string }
  | { type: "ai"; text: string }
  | { type: "run"; what: Run };

/** A line that can't be run: an i18n key and its values. */
export type ParseError = { type: "error"; key: string; args?: Record<string, string> };

const RUNS: Partial<Record<CommandName, Run>> = {
  OPEN: "open",
  SAVE: "save",
  SAVEAS: "saveAs",
  EXPORT: "export",
  BOQ: "importBoq",
  COMPARE: "compare",
  UNDO: "undo",
  REDO: "redo",
  PDFATTACH: "attachPdf",
  IMAGEATTACH: "attachImage",
  XREF: "xrefs",
  CALIBRATE: "calibrate",
  EXTRACT: "extract",
};

const TOOLS: Partial<Record<CommandName, Tool>> = {
  DIST: "distance",
  AREA: "area",
  GEOMETRY: "geometry",
  ID: "idPoint",
};

const TOGGLES: Partial<Record<CommandName, DraftingToggle>> = {
  OSNAP: "osnap",
  ORTHO: "ortho",
  POLAR: "polar",
  GRID: "grid",
};

const ZOOMS: Record<string, Action> = {
  E: { type: "zoom", how: "extents" },
  EXTENTS: { type: "zoom", how: "extents" },
  W: { type: "tool", tool: "zoomWindow" },
  WINDOW: { type: "tool", tool: "zoomWindow" },
  IN: { type: "zoom", how: "in" },
  OUT: { type: "zoom", how: "out" },
};

/** Western digits, `.` and `,` for Arabic-Indic and Eastern Arabic-Indic digits, `٫` and `،`. */
export function normalizeDigits(text: string): string {
  return text
    .replace(/[٠-٩]/g, (d) => String(d.charCodeAt(0) - 0x0660))
    .replace(/[۰-۹]/g, (d) => String(d.charCodeAt(0) - 0x06f0))
    .replace(/٫/g, ".")
    .replace(/،/g, ",");
}

const NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)$/;

/** A number typed in either script, or null. */
function readNumber(text: string): number | null {
  const t = normalizeDigits(text.trim());
  return NUMBER.test(t) ? Number(t) : null;
}

/** Looks like a try at a point or a number rather than a command. */
const numeric = (text: string) => /^[-+.@\d]/.test(normalizeDigits(text.trim()));

/** A typed point, an error for a malformed one, or null when `text` is no point at all. */
function readPoint(text: string, last: Point | null): Point | ParseError | null {
  const t = normalizeDigits(text.trim());
  if (!numeric(t)) return null;
  const bad: ParseError = { type: "error", key: "cmd.badPoint" };
  const relative = t.startsWith("@");
  const body = relative ? t.slice(1).trim() : t;
  if (relative && !last) return { type: "error", key: "cmd.noLastPoint" };
  if (relative && body.includes("<")) {
    const [d, a] = body.split("<").map((s) => readNumber(s));
    if (d === null || a === null || body.split("<").length !== 2) return bad;
    const rad = (a * Math.PI) / 180;
    return [last![0] + d * Math.cos(rad), last![1] + d * Math.sin(rad)];
  }
  const parts = body.split(",");
  if (parts.length !== 2) return bad;
  const [x, y] = parts.map((s) => readNumber(s));
  if (x === null || y === null) return bad;
  return relative ? [last![0] + x, last![1] + y] : [x, y];
}

/** Splits on spaces; double quotes keep a name with spaces in one word. */
export function tokenize(text: string): string[] {
  const words: string[] = [];
  const re = /"([^"]*)"?|(\S+)/g;
  for (let m = re.exec(text); m; m = re.exec(text)) words.push(m[1] ?? m[2]);
  return words;
}

/** The rest of a line as one name: unquoted when the whole of it is quoted. */
function oneName(rest: string): string {
  const t = rest.trim();
  return t.length >= 2 && t.startsWith('"') && t.endsWith('"') ? t.slice(1, -1) : t;
}

/** A command line, with its first word's alias expanded. */
function expand(line: string, aliases: Record<string, string>): { name: string; rest: string } {
  const t = line.trim();
  const first = t.split(/\s/, 1)[0];
  let rest = t.slice(first.length).trim();
  let name = first.toUpperCase();
  const alias = aliases[name];
  if (alias !== undefined) {
    const target = alias.trim();
    const head = target.split(/\s/, 1)[0];
    name = head.toUpperCase();
    rest = [target.slice(head.length).trim(), rest].filter((s) => s !== "").join(" ");
  }
  return { name, rest };
}

function command(line: string, aliases: Record<string, string>): Action | ParseError {
  const { name, rest } = expand(line, aliases);
  if (!(COMMANDS as readonly string[]).includes(name)) {
    return { type: "error", key: "cmd.unknown", args: { name: line.trim().split(/\s/, 1)[0] } };
  }
  const cmd = name as CommandName;
  const run = RUNS[cmd];
  if (run) return { type: "run", what: run };
  const tool = TOOLS[cmd];
  if (tool) return { type: "tool", tool };
  const toggle = TOGGLES[cmd];
  if (toggle) return { type: "toggle", toggle };
  const needs = (key: string): ParseError => ({ type: "error", key, args: { name: cmd } });
  switch (cmd) {
    case "ESC":
      return { type: "cancel" };
    case "COUNT":
      return rest ? { type: "count", block: oneName(rest) } : { type: "tool", tool: "count" };
    case "DIVIDE":
    case "MEASURE": {
      const tool = cmd === "DIVIDE" ? "divide" : "measureAlong";
      if (!rest) return { type: "tool", tool };
      const value = readNumber(rest);
      const bad = cmd === "DIVIDE" ? badCount(value) : badStep(value);
      return bad ?? { type: "tool", tool, preset: value! };
    }
    case "LAYOFF":
    case "LAYON": {
      const names = tokenize(rest);
      return names.length > 0
        ? { type: "layers", hide: cmd === "LAYOFF", names }
        : needs("cmd.needLayer");
    }
    case "LAYISO":
      return rest ? { type: "isolate", name: oneName(rest) } : needs("cmd.needLayer");
    case "LAYALL":
      return { type: "showAll" };
    case "ZOOM":
      return ZOOMS[rest.toUpperCase()] ?? { type: "error", key: "cmd.zoomOption" };
    case "SHEET":
      return rest ? { type: "sheet", name: oneName(rest) } : needs("cmd.needSheet");
    case "AI":
      return rest ? { type: "ai", text: rest } : needs("cmd.needText");
    default:
      return { type: "error", key: "cmd.unknown", args: { name: cmd } };
  }
}

/** Divide takes 2 to 1000 parts, as its prompt does. */
function badCount(value: number | null): ParseError | null {
  return value !== null && Number.isInteger(value) && value >= 2 && value <= 1000
    ? null
    : { type: "error", key: "cmd.badCount" };
}

function badStep(value: number | null): ParseError | null {
  return value !== null && value > 0 ? null : { type: "error", key: "cmd.badStep" };
}

/** The action for one typed line, or why it can't run. */
export function parse(line: string, state: ParseState): Action | ParseError {
  const text = line.trim();
  if (text === "") {
    if (state.expect === "point") return { type: "finish" };
    if (state.expect === "command") return { type: "repeat" };
    return { type: "error", key: state.expect === "count" ? "cmd.badCount" : "cmd.badStep" };
  }
  if (state.expect === "point") {
    const p = readPoint(text, state.last);
    if (p !== null) return Array.isArray(p) ? { type: "point", p } : p;
  }
  if ((state.expect === "count" || state.expect === "step") && numeric(text)) {
    const value = readNumber(text);
    const bad = state.expect === "count" ? badCount(value) : badStep(value);
    return bad ?? { type: "number", value: value! };
  }
  return command(text, state.aliases);
}

/** One autocomplete entry: a command, or an alias and the command it runs. */
export type Suggestion = { name: string; target?: string };

/** The commands and aliases starting with `prefix` (any case): exact match first, then commands,
 *  then aliases, each alphabetical. None once the line has a space. */
export function suggest(prefix: string, aliases: Record<string, string>): Suggestion[] {
  const p = prefix.trim().toUpperCase();
  if (p === "" || /\s/.test(prefix.trimStart())) return [];
  const found: Suggestion[] = [
    ...COMMANDS.filter((c) => c.startsWith(p)).map((name) => ({ name })),
    ...Object.keys(aliases)
      .filter((a) => a.startsWith(p) && !(COMMANDS as readonly string[]).includes(a))
      .sort()
      .map((name) => ({ name, target: aliases[name] })),
  ];
  const exact = found.findIndex((s) => s.name === p);
  if (exact > 0) found.unshift(...found.splice(exact, 1));
  return found;
}

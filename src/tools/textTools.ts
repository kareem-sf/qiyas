/** The text tools: TEXT places single-line text where it is clicked (H height, R rotation), MTEXT
 *  takes a box by two corners (H height). Pure state machines with their prompts (i18n keys), as
 *  the draw tools are; each placement becomes a `request` the app opens a text editor for (the
 *  in-place editor for TEXT, the MText dialog for MTEXT), and the tool waits for the next one. */
import type { Point } from "../viewport/camera";

export const TEXT_TOOLS = ["text", "mtext"] as const;

export type TextKind = (typeof TEXT_TOOLS)[number];

export const isTextTool = (tool: string): tool is TextKind =>
  (TEXT_TOOLS as readonly string[]).includes(tool);

/** Where a new text goes; `height` is the one typed with H (null: the app's text height). */
export type TextRequest =
  | { kind: "text"; at: Point; rotation: number; height: number | null }
  /** `at` is the box's top left corner; `width` its width (drawing units). */
  | { kind: "mtext"; at: Point; width: number; height: number | null };

export type TextState = {
  kind: TextKind;
  /** MTEXT: the box's first corner, once clicked. */
  corner: Point | null;
  /** The height typed with H; null until then. */
  height: number | null;
  /** TEXT: degrees counter-clockwise, typed with R. */
  rotation: number;
  /** The option whose number comes next. */
  asking: "H" | "R" | null;
  /** Set by the input that placed a text; the hook hands it to the app. */
  request: TextRequest | null;
  hint: string;
  hintArgs: Record<string, string> | null;
};

export type TextInput =
  | { type: "point"; p: Point }
  | { type: "number"; value: number }
  | { type: "option"; option: string }
  | { type: "finish" };

const fmt = (v: number) => String(Number(v.toPrecision(6)));

/** The tool waiting for its next click, with its prompt. */
function waiting(s: TextState): TextState {
  const base = { ...s, asking: null, request: null };
  if (s.kind === "mtext") {
    return s.corner
      ? { ...base, hint: "text.prompt.mtextOther", hintArgs: null }
      : { ...base, hint: "text.prompt.mtextFirst", hintArgs: null };
  }
  return { ...base, hint: "text.prompt.start", hintArgs: { r: fmt(s.rotation) } };
}

export function initialTextState(kind: TextKind): TextState {
  return waiting({
    kind,
    corner: null,
    height: null,
    rotation: 0,
    asking: null,
    request: null,
    hint: "",
    hintArgs: null,
  });
}

export function textReducer(s: TextState, input: TextInput): TextState {
  const next = { ...s, request: null };
  switch (input.type) {
    case "option": {
      if (input.option === "H") return { ...next, asking: "H", hint: "text.prompt.height", hintArgs: null };
      if (input.option === "R" && s.kind === "text") {
        return { ...next, asking: "R", hint: "text.prompt.rotation", hintArgs: null };
      }
      return next;
    }
    case "number": {
      if (s.asking === "H" && input.value > 0) return waiting({ ...next, height: input.value });
      if (s.asking === "R" && Number.isFinite(input.value)) return waiting({ ...next, rotation: input.value });
      return next;
    }
    case "point": {
      if (s.kind === "text") {
        return { ...waiting(next), request: { kind: "text", at: input.p, rotation: s.rotation, height: s.height } };
      }
      const a = s.corner;
      if (!a) return waiting({ ...next, corner: input.p });
      const b = input.p;
      if (a[0] === b[0]) return next;
      const request: TextRequest = {
        kind: "mtext",
        at: [Math.min(a[0], b[0]), Math.max(a[1], b[1])],
        width: Math.abs(b[0] - a[0]),
        height: s.height,
      };
      return { ...waiting({ ...next, corner: null }), request };
    }
    case "finish":
      // Enter drops a half-typed option or a first corner.
      return waiting({ ...next, corner: null });
  }
}

/** The option words the command line takes now. */
export function textOptions(s: TextState): string[] {
  if (s.asking) return [];
  return s.kind === "text" ? ["H", "R"] : ["H"];
}

/** The tool takes a typed number now: a height ("distance") or a rotation ("angle"). */
export function takesTextNumber(s: TextState): false | "distance" | "angle" {
  if (s.asking === "H") return "distance";
  if (s.asking === "R") return "angle";
  return false;
}

/** MTEXT's box from its first corner to the cursor, as segments `x0 y0 x1 y1`. */
export function textPreview(s: TextState, cursor: Point | null): Float64Array {
  const a = s.corner;
  if (!a || !cursor) return new Float64Array();
  const [x0, y0] = a;
  const [x1, y1] = cursor;
  return Float64Array.of(x0, y0, x1, y0, x1, y0, x1, y1, x1, y1, x0, y1, x0, y1, x0, y0);
}

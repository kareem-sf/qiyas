import type { Tool } from "../shell/Ribbon";

/** A drafting switch the function keys turn on and off, as in AutoCAD. */
export type DraftingToggle = "osnap" | "grid" | "ortho" | "polar";

/** What a key press asks for: a tool, a drafting switch, or one of the actions below. */
export type Shortcut =
  | Tool
  | DraftingToggle
  | "undo"
  | "redo"
  | "save"
  | "escape"
  | "finish"
  | "commandLine";

export type Keys = Pick<KeyboardEvent, "code" | "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey">;

const TOGGLE_CODES: Record<string, DraftingToggle> = {
  F3: "osnap",
  F7: "grid",
  F8: "ortho",
  F10: "polar",
};

const TOOL_CODES: Record<string, Tool> = {
  KeyD: "distance",
  KeyA: "area",
  KeyC: "count",
  KeyS: "select",
};

/** The shortcut for a key press, or null. Matched on `code` (the physical key), not `key`, so the
 *  shortcuts work the same with an Arabic keyboard layout. */
export function shortcutFor(e: Keys): Shortcut | null {
  if (e.altKey || e.metaKey) return null;
  if (e.ctrlKey) {
    if (e.code === "KeyZ") return e.shiftKey ? "redo" : "undo";
    if (e.code === "KeyY") return "redo";
    if (e.code === "KeyS" && !e.shiftKey) return "save";
    // Shows and hides the command line, as in AutoCAD.
    if (e.code === "Digit9" && !e.shiftKey) return "commandLine";
    return null;
  }
  if (e.code === "Escape") return "escape";
  if (!e.shiftKey && e.code in TOGGLE_CODES) return TOGGLE_CODES[e.code];
  if (e.code === "Enter" || e.code === "NumpadEnter") return "finish";
  if (e.shiftKey) return null;
  return TOOL_CODES[e.code] ?? null;
}

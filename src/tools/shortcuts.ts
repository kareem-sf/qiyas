import type { Tool } from "../shell/Ribbon";

/** What a key press asks for: a tool, or one of the actions below. */
export type Shortcut = Tool | "undo" | "redo" | "save" | "escape" | "finish";

export type Keys = Pick<KeyboardEvent, "code" | "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey">;

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
    return null;
  }
  if (e.code === "Escape") return "escape";
  if (e.code === "Enter" || e.code === "NumpadEnter") return "finish";
  if (e.shiftKey) return null;
  return TOOL_CODES[e.code] ?? null;
}

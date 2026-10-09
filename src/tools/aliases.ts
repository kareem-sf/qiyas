/** The command line's commands and their aliases. Command names are English in both languages. */

/** Every command the command line knows, by its full name. */
export const COMMANDS = [
  "AI",
  "AREA",
  "BOQ",
  "CALIBRATE",
  "COMPARE",
  "COUNT",
  "DIST",
  "DIVIDE",
  "ESC",
  "EXPORT",
  "EXTRACT",
  "GEOMETRY",
  "GRID",
  "ID",
  "IMAGEATTACH",
  "LAYALL",
  "LAYISO",
  "LAYOFF",
  "LAYON",
  "MEASURE",
  "OPEN",
  "ORTHO",
  "OSNAP",
  "PDFATTACH",
  "POLAR",
  "REDO",
  "SAVE",
  "SAVEAS",
  "SHEET",
  "UNDO",
  "XREF",
  "ZOOM",
] as const;

export type CommandName = (typeof COMMANDS)[number];

/** The built-in aliases, alias to command. */
export const DEFAULT_ALIASES: Record<string, string> = {
  AA: "AREA",
  DI: "DIST",
  DISTANCE: "DIST",
  DIV: "DIVIDE",
  GEOM: "GEOMETRY",
  ME: "MEASURE",
  U: "UNDO",
  Z: "ZOOM",
};

/** The built-in aliases with the user's (from the settings) over them; aliases upper-cased. */
export function mergeAliases(user: Record<string, string> = {}): Record<string, string> {
  const merged = { ...DEFAULT_ALIASES };
  for (const [alias, command] of Object.entries(user)) merged[alias.toUpperCase()] = command;
  return merged;
}

/** The command line's commands and their aliases. Command names are English in both languages. */

/** Every command the command line knows, by its full name. */
export const COMMANDS = [
  "AI",
  "ARC",
  "AREA",
  "BOQ",
  "CALIBRATE",
  "CIRCLE",
  "COMPARE",
  "COUNT",
  "DIST",
  "DIVIDE",
  "DONUT",
  "ELLIPSE",
  "ESC",
  "EXPORT",
  "EXTRACT",
  "GEOMETRY",
  "GRID",
  "HATCH",
  "ID",
  "IMAGEATTACH",
  "LAYALL",
  "LAYISO",
  "LAYOFF",
  "LAYON",
  "LINE",
  "MEASURE",
  "MLINE",
  "OPEN",
  "ORTHO",
  "OSNAP",
  "PDFATTACH",
  "PLINE",
  "POINT",
  "POLAR",
  "POLYGON",
  "RAY",
  "RECTANG",
  "REDO",
  "REVCLOUD",
  "SAVE",
  "SAVEAS",
  "SHEET",
  "SKETCH",
  "SOLID",
  "SPLINE",
  "TRACE",
  "UNDO",
  "WIPEOUT",
  "XLINE",
  "XREF",
  "ZOOM",
] as const;

export type CommandName = (typeof COMMANDS)[number];

/** The built-in aliases, alias to command. */
export const DEFAULT_ALIASES: Record<string, string> = {
  A: "ARC",
  AA: "AREA",
  C: "CIRCLE",
  DI: "DIST",
  DISTANCE: "DIST",
  DIV: "DIVIDE",
  DO: "DONUT",
  EL: "ELLIPSE",
  GEOM: "GEOMETRY",
  H: "HATCH",
  L: "LINE",
  ME: "MEASURE",
  ML: "MLINE",
  PL: "PLINE",
  PO: "POINT",
  POL: "POLYGON",
  REC: "RECTANG",
  SO: "SOLID",
  SPL: "SPLINE",
  U: "UNDO",
  XL: "XLINE",
  Z: "ZOOM",
};

/** The built-in aliases with the user's (from the settings) over them; aliases upper-cased. */
export function mergeAliases(user: Record<string, string> = {}): Record<string, string> {
  const merged = { ...DEFAULT_ALIASES };
  for (const [alias, command] of Object.entries(user)) merged[alias.toUpperCase()] = command;
  return merged;
}

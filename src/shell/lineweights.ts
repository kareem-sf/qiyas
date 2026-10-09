/** AutoCAD's lineweights, in hundredths of a millimetre (as the core checks them). */
export const LINEWEIGHTS = [
  0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200, 211,
];

/** A lineweight as shown: ByLayer, ByBlock and Default by their names (in the window's language,
 *  through `t`), the rest in millimetres. */
export const weightLabel = (v: number, t: (key: string) => string) =>
  v === -1 ? t("props.byLayer") : v === -2 ? t("props.byBlock") : v < 0 ? t("props.default") : `${(v / 100).toFixed(2)} mm`;

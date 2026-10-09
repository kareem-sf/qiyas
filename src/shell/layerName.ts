/** A layer name as a QS reads it, as the core's `short_layer_name`: a bound xref's layer
 *  `XREF$0$LAYER` (nested ones repeat the `$N$`) gives the part after its last `$N$`, e.g.
 *  `A-WALL`. Other names are kept whole. */
export function shortLayerName(name: string): string {
  let start = 0;
  for (let i = 0; i < name.length; i++) {
    if (name[i] !== "$") continue;
    let digits = 0;
    while (/[0-9]/.test(name[i + 1 + digits] ?? "")) digits++;
    if (digits > 0 && name[i + 1 + digits] === "$") start = i + digits + 2;
  }
  return name.slice(start) || name;
}

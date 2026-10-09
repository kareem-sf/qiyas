import type { Sheet } from "../api";
import { useT } from "../i18n";
import { useRoving } from "./useRoving";

type Props = { sheets: Sheet[]; active: string; onSelect(block: string): void };

/** One tab per sheet: the model first as the drawing lists it, then the layouts. */
export function SheetTabs({ sheets, active, onSelect }: Props) {
  const t = useT();
  const roving = useRoving(
    sheets.length,
    sheets.findIndex((s) => s.block === active),
  );
  return (
    <div className="sheet-tabs" role="tablist" aria-label={t("sheets")} {...roving.props}>
      {sheets.map((s, i) => (
        <button
          key={s.block}
          type="button"
          role="tab"
          aria-selected={s.block === active}
          tabIndex={roving.tabIndex(i)}
          onClick={() => onSelect(s.block)}
        >
          {s.is_model ? t("model") : <bdi>{s.name}</bdi>}
        </button>
      ))}
    </div>
  );
}

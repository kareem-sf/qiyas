import type { Sheet } from "../api";
import { useT } from "../i18n";

type Props = { sheets: Sheet[]; active: string; onSelect(block: string): void };

/** One tab per sheet: the model first as the drawing lists it, then the layouts. */
export function SheetTabs({ sheets, active, onSelect }: Props) {
  const t = useT();
  return (
    <div className="sheet-tabs" role="tablist" aria-label={t("sheets")}>
      {sheets.map((s) => (
        <button
          key={s.block}
          type="button"
          role="tab"
          aria-selected={s.block === active}
          onClick={() => onSelect(s.block)}
        >
          {s.is_model ? t("model") : s.name}
        </button>
      ))}
    </div>
  );
}

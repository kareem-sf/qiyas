import { memo, useRef, useState } from "react";
import type { Measurement, Total } from "../api";
import { useT } from "../i18n";

type Props = {
  items: Measurement[];
  totals: Total[];
  selectedId: string | null;
  onSelect(id: string): void;
  onRemove(id: string): void;
  onRename(id: string, text: string): void;
};

/** Quantities keep Latin digits and three decimals in both languages. */
const qty = (n: number) => n.toFixed(3);

/** The take-off: one row per measurement (click to show it on the drawing, double click or the pen
 *  to rename, the cross to remove) and the totals per kind underneath. Memoised: the app re-renders
 *  on every pointer move. */
export const TakeoffPanel = memo(function TakeoffPanel({
  items,
  totals,
  selectedId,
  onSelect,
  onRemove,
  onRename,
}: Props) {
  const t = useT();
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  /** Set by Escape, so the blur that ends the edit doesn't save it. */
  const cancelled = useRef(false);

  function startEdit(m: Measurement) {
    setDraft(m.description);
    setEditing(m.id);
  }

  function endEdit(m: Measurement) {
    const text = draft.trim();
    if (!cancelled.current && text && text !== m.description) onRename(m.id, text);
    cancelled.current = false;
    setEditing(null);
  }

  return (
    <aside className="takeoff">
      <h2 className="heading panel-head">{t("takeoff")}</h2>
      {items.length === 0 ? (
        <p className="empty">{t("takeoffEmpty")}</p>
      ) : (
        <div className="takeoff-rows">
          <table>
            <thead>
              <tr>
                <th>{t("description")}</th>
                <th className="num">{t("quantity")}</th>
                <th>{t("unit")}</th>
                <th>{t("layer")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {items.map((m) => {
                const renameLabel = `${t("rename")}: ${m.description}`;
                const removeLabel = `${t("remove")}: ${m.description}`;
                return (
                  <tr
                    key={m.id}
                    aria-current={m.id === selectedId || undefined}
                    onClick={() => onSelect(m.id)}
                  >
                    <td>
                      <div className="desc">
                        {editing === m.id ? (
                          <input
                            aria-label={t("description")}
                            value={draft}
                            autoFocus
                            onChange={(e) => setDraft(e.target.value)}
                            onClick={(e) => e.stopPropagation()}
                            onBlur={() => endEdit(m)}
                            onKeyDown={(e) => {
                              if (e.key === "Escape") cancelled.current = true;
                              if (e.key === "Enter" || e.key === "Escape") e.currentTarget.blur();
                            }}
                          />
                        ) : (
                          <button
                            type="button"
                            title={m.description}
                            onDoubleClick={() => startEdit(m)}
                          >
                            {m.description}
                          </button>
                        )}
                        {m.self_intersecting && (
                          <span
                            className="warn-icon"
                            role="img"
                            aria-label={t("selfCrossing")}
                            title={t("selfCrossing")}
                          >
                            <svg viewBox="0 0 16 16" aria-hidden="true">
                              <path d="M8 2 15 14H1Z M8 6.5v3.5 M8 12v.01" />
                            </svg>
                          </span>
                        )}
                      </div>
                    </td>
                    <td className="num">{qty(m.shown)}</td>
                    <td>{t(`qty.${m.unit}`)}</td>
                    <td title={m.layer}>{m.layer}</td>
                    <td>
                      <div className="actions">
                        <button
                          type="button"
                          className="icon"
                          aria-label={renameLabel}
                          title={renameLabel}
                          onClick={(e) => {
                            e.stopPropagation();
                            startEdit(m);
                          }}
                        >
                          <svg viewBox="0 0 16 16" aria-hidden="true">
                            <path d="M3 13l.6-2.6L10.8 3.2l2 2-7.2 7.2Z" />
                          </svg>
                        </button>
                        <button
                          type="button"
                          className="icon"
                          aria-label={removeLabel}
                          title={removeLabel}
                          onClick={(e) => {
                            e.stopPropagation();
                            onRemove(m.id);
                          }}
                        >
                          <svg viewBox="0 0 16 16" aria-hidden="true">
                            <path d="M4 4l8 8M12 4l-8 8" />
                          </svg>
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
      <h2 className="heading panel-head">{t("totals")}</h2>
      <ul className="totals" aria-label={t("totals")}>
        {totals.map((total) => (
          <li key={`${total.kind}-${total.unit}`}>
            <span className="kind">{t(total.kind)}</span>
            <span className="num">{total.count} ×</span>
            <span className="num">
              {qty(total.sum)} {t(`qty.${total.unit}`)}
            </span>
          </li>
        ))}
      </ul>
    </aside>
  );
});

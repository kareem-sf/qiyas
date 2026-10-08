import { memo, useRef, useState, type ReactNode } from "react";
import type { Boq, BoqItem, Measurement } from "../api";
import { useT } from "../i18n";
import {
  DEFAULT_TOLERANCE,
  difference,
  matches,
  measuredFor,
  qty,
  unitLabel,
  type Difference,
} from "./boqMath";

type Props = {
  boq: Boq | null;
  /** The take-off, for the measured quantities. */
  takeoff: Measurement[];
  selectedCode: string | null;
  /** Shows the item's linked measurements on the drawing. */
  onSelect(code: string): void;
  /** Missing while no drawing is open: the button is disabled. */
  onImport?(): void;
  /** A tolerance in percent, or null for the default. */
  onTolerance(code: string, percent: number | null): void;
};

const WARN = (
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <path d="M8 2 15 14H1Z M8 6.5v3.5 M8 12v.01" />
  </svg>
);

/** "+12.5 %", Latin digits; "—" when there is nothing to compare. */
function percentText({ percent }: Difference) {
  if (percent === null) return "—";
  const p = Math.abs(percent) < 0.05 ? 0 : percent;
  return `${p > 0 ? "+" : ""}${p.toFixed(1)} %`;
}

/** The imported BOQ against the take-off: per item its quantity, what the linked measurements add
 *  up to, and the difference coloured by the item's tolerance. Click a row to show its
 *  measurements; double click the tolerance to change it. Memoised: the app re-renders on every
 *  pointer move. */
export const BoqPanel = memo(function BoqPanel({
  boq,
  takeoff,
  selectedCode,
  onSelect,
  onImport,
  onTolerance,
}: Props) {
  const t = useT();
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  /** Set by Escape, so the blur that ends the edit doesn't save it. */
  const cancelled = useRef(false);

  function endEdit(item: BoqItem) {
    const text = draft.trim();
    const percent = text === "" ? null : Number(text);
    const valid = percent === null || (Number.isFinite(percent) && percent >= 0);
    if (!cancelled.current && valid && percent !== item.tolerance) onTolerance(item.code, percent);
    cancelled.current = false;
    setEditing(null);
  }

  const importButton = (
    <button type="button" className="boq-import" disabled={!onImport} onClick={onImport}>
      {t("importBoq")}
    </button>
  );

  if (!boq) {
    return (
      <aside className="boq">
        <div className="panel-head boq-head">
          <h2 className="heading">{t("boq")}</h2>
          {importButton}
        </div>
        <p className="empty">{t("boq.empty")}</p>
      </aside>
    );
  }

  const found = boq.items.map((item, i) => ({ item, i })).filter(({ item }) => matches(item, query));
  const rows: ReactNode[] = [];
  let section = "";
  for (const { item, i } of found) {
    if (item.section !== section && item.section !== "") {
      rows.push(
        <tr key={`s${i}`} className="section">
          <th colSpan={8} scope="colgroup" title={item.section}>
            {item.section}
          </th>
        </tr>,
      );
    }
    section = item.section;
    const measured = measuredFor(item, takeoff);
    const diff = difference(item, measured.sum);
    const others = measured.other.map((o) => `${qty(o.sum, o.unit)} ${unitLabel(t, o.unit)}`);
    const sum = measured.sum === null ? [] : [qty(measured.sum, item.unit)];
    const shown = [...sum, ...others].join(" · ");
    rows.push(
      <tr
        key={i}
        aria-current={(item.code !== "" && item.code === selectedCode) || undefined}
        onClick={() => onSelect(item.code)}
      >
        <td title={item.code}>{item.code}</td>
        <td title={item.description} dir="auto">
          {item.description}
        </td>
        <td>{unitLabel(t, item.unit)}</td>
        <td className="num">{qty(item.quantity, item.unit)}</td>
        <td className={`num${measured.links.length === 0 ? " muted" : ""}`} title={shown || undefined}>
          <span className="measured">
            {measured.other.length > 0 && (
              <span
                className="warn-icon"
                role="img"
                aria-label={t("boq.unitMismatch")}
                title={t("boq.unitMismatch")}
              >
                {WARN}
              </span>
            )}
            <span dir="ltr">{shown || "—"}</span>
          </span>
        </td>
        <td className={`num diff-${diff.status}`}>
          <span dir="ltr">{percentText(diff)}</span>
        </td>
        <td className="num">{measured.links.length}</td>
        <td
          className="num tol"
          onDoubleClick={() => {
            setDraft(item.tolerance === null ? "" : String(item.tolerance));
            setEditing(i);
          }}
        >
          {editing === i ? (
            <input
              type="number"
              min={0}
              step="any"
              aria-label={t("boq.tolerance")}
              placeholder={String(DEFAULT_TOLERANCE)}
              value={draft}
              autoFocus
              onChange={(e) => setDraft(e.target.value)}
              onClick={(e) => e.stopPropagation()}
              onBlur={() => endEdit(item)}
              onKeyDown={(e) => {
                if (e.key === "Escape") cancelled.current = true;
                if (e.key === "Enter" || e.key === "Escape") e.currentTarget.blur();
              }}
            />
          ) : (
            <span dir="ltr" className={item.tolerance === null ? "muted" : undefined}>
              {item.tolerance ?? DEFAULT_TOLERANCE} %
            </span>
          )}
        </td>
      </tr>,
    );
  }

  return (
    <aside className="boq">
      <div className="panel-head boq-head">
        <h2 className="heading">{t("boq")}</h2>
        <span className="source" title={boq.source}>
          {boq.source.split(/[\\/]/).pop()}
        </span>
        {importButton}
      </div>
      {boq.skipped_rows > 0 && (
        <p className="boq-notice" role="status">
          {t("boq.skipped", { n: boq.skipped_rows })}
        </p>
      )}
      <input
        className="boq-search"
        type="search"
        aria-label={t("boq.search")}
        placeholder={t("boq.search")}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <div className="boq-rows">
        <table>
          <thead>
            <tr>
              <th>{t("boq.code")}</th>
              <th>{t("description")}</th>
              <th>{t("unit")}</th>
              <th className="num">{t("boq.quantity")}</th>
              <th className="num">{t("boq.measured")}</th>
              <th className="num">{t("boq.difference")}</th>
              <th className="num">{t("boq.links")}</th>
              <th className="num">{t("boq.tolerance")}</th>
            </tr>
          </thead>
          <tbody>{rows}</tbody>
        </table>
        {found.length === 0 && <p className="empty">{t("boq.noMatch")}</p>}
      </div>
    </aside>
  );
});

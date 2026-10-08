import { memo, useRef, useState, type ReactNode } from "react";
import type { Command, Conflict, ConflictKind } from "../api";
import { useLanguage, useT } from "../i18n";
import { qty, unitLabel } from "./boqMath";
import {
  KINDS,
  STATUSES,
  countByKind,
  filterConflicts,
  groupByKind,
  reasonIn,
  type StatusFilter,
} from "./conflictsMath";

export type ResolveCommand = Extract<Command, { type: "resolve_conflict" }>;

type Props = {
  conflicts: Conflict[];
  /** A comparison ran for this drawing, so an empty list means none were found. */
  compared: boolean;
  hasBoq: boolean;
  /** Missing while no drawing is open: the button is disabled. */
  onCompare?(): void;
  onExport?(): void;
  /** Points the drawing (or the BOQ tab) at what the conflict is about. */
  onShow(conflict: Conflict): void;
  /** Sets a conflict's status and note. */
  onResolve(cmd: ResolveCommand): void;
  /** Opens the BOQ tab, from the "no BOQ" empty state. */
  onOpenBoq(): void;
};

/** The last comparison's conflicts, grouped by kind: filter them, show one on the drawing, set its
 *  status, write a note, export them. Memoised: the app re-renders on every pointer move. */
export const ConflictsPanel = memo(function ConflictsPanel({
  conflicts,
  compared,
  hasBoq,
  onCompare,
  onExport,
  onShow,
  onResolve,
  onOpenBoq,
}: Props) {
  const t = useT();
  const { lang } = useLanguage();
  const [status, setStatus] = useState<StatusFilter>("all");
  const [kind, setKind] = useState<ConflictKind | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  /** Set by Escape, so the blur that ends the edit doesn't save it. */
  const cancelled = useRef(false);

  function endEdit(c: Conflict) {
    const note = draft.trim();
    if (!cancelled.current && note !== c.note) {
      onResolve({ type: "resolve_conflict", id: c.id, status: c.status, note });
    }
    cancelled.current = false;
    setEditing(null);
  }

  const byStatus = filterConflicts(conflicts, status, null);
  const counts = countByKind(byStatus);
  // A chip stays while its kind has conflicts in any status, so a chosen one doesn't vanish.
  const present = countByKind(conflicts);
  const shown = filterConflicts(byStatus, "all", kind);
  const arrow = lang === "ar" ? "←" : "→";

  let empty: ReactNode = null;
  if (conflicts.length === 0) {
    if (!hasBoq) {
      empty = (
        <p className="empty">
          {t("conflicts.noBoq")}{" "}
          <button type="button" className="link" onClick={onOpenBoq}>
            {t("conflicts.toBoq")}
          </button>
        </p>
      );
    } else {
      empty = <p className="empty">{t(compared ? "conflicts.none" : "conflicts.notCompared")}</p>;
    }
  } else if (shown.length === 0) {
    empty = <p className="empty">{t("conflicts.noMatch")}</p>;
  }

  const chip = (value: ConflictKind | null, label: string, n: number) => (
    <button
      key={value ?? "all"}
      type="button"
      aria-pressed={kind === value}
      onClick={() => setKind(kind === value || value === null ? null : value)}
    >
      {label} <span className="n">{n}</span>
    </button>
  );

  const row = (c: Conflict) => {
    const numbers = c.expected !== null || c.found !== null;
    return (
      <li key={c.id} className={c.status === "open" ? "conflict" : "conflict done"}>
        <div className="what">
          <p className="reason" dir="auto">
            {reasonIn(c, lang)}
          </p>
          <p className="meta">
            {c.boq_code && <span className="code">{c.boq_code}</span>}
            {numbers && (
              <span className="numbers" title={t("conflicts.expectedFound")}>
                <bdi>{c.expected === null ? "—" : qty(c.expected, c.unit)}</bdi> {arrow}{" "}
                <bdi>{c.found === null ? "—" : qty(c.found, c.unit)}</bdi>
                {c.unit && <> {unitLabel(t, c.unit)}</>}
              </span>
            )}
            {c.sheets.length > 0 && (
              <span className="sheets" dir="auto">
                {c.sheets.join(", ")}
              </span>
            )}
          </p>
        </div>
        <div className="row-actions">
          <button
            type="button"
            disabled={c.measurements.length === 0 && !c.boq_code && c.sheets.length === 0}
            onClick={() => onShow(c)}
          >
            {t("conflicts.show")}
          </button>
          <select
            aria-label={t("conflicts.status")}
            value={c.status}
            onChange={(e) =>
              onResolve({
                type: "resolve_conflict",
                id: c.id,
                status: e.target.value as Conflict["status"],
                note: c.note,
              })
            }
          >
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {t(`conflictStatus.${s}`)}
              </option>
            ))}
          </select>
        </div>
        <div className="note">
          {editing === c.id ? (
            <input
              type="text"
              dir="auto"
              aria-label={t("conflicts.note")}
              placeholder={t("conflicts.note")}
              value={draft}
              autoFocus
              onChange={(e) => setDraft(e.target.value)}
              onBlur={() => endEdit(c)}
              onKeyDown={(e) => {
                if (e.key === "Escape") cancelled.current = true;
                if (e.key === "Enter" || e.key === "Escape") e.currentTarget.blur();
              }}
            />
          ) : (
            <button
              type="button"
              dir="auto"
              aria-label={c.note ? undefined : t("conflicts.note")}
              className={c.note ? undefined : "muted"}
              onClick={() => {
                setDraft(c.note);
                setEditing(c.id);
              }}
            >
              {c.note || t("conflicts.note")}
            </button>
          )}
        </div>
      </li>
    );
  };

  return (
    <aside className="conflicts">
      <div className="panel-head conflicts-head">
        <h2 className="heading">{t("conflicts")}</h2>
        <button type="button" className="primary" disabled={!onCompare} onClick={onCompare}>
          {t("compare")}
        </button>
        <button
          type="button"
          disabled={!onExport || conflicts.length === 0}
          onClick={onExport}
        >
          {t("exportConflicts")}
        </button>
      </div>
      {conflicts.length > 0 && (
        <div className="conflicts-filters">
          <div className="segmented" role="group" aria-label={t("conflicts.status")}>
            {(["all", ...STATUSES] as StatusFilter[]).map((s) => (
              <button key={s} type="button" aria-pressed={status === s} onClick={() => setStatus(s)}>
                {t(s === "all" ? "conflicts.all" : `conflictStatus.${s}`)}
              </button>
            ))}
          </div>
          <div className="chips" role="group" aria-label={t("conflicts.kind")}>
            {chip(null, t("conflicts.all"), byStatus.length)}
            {KINDS.filter((k) => present[k] > 0).map((k) => chip(k, t(`conflictKind.${k}`), counts[k]))}
          </div>
        </div>
      )}
      {empty ?? (
        <div className="conflicts-rows">
          {groupByKind(shown).map((g) => (
            <section key={g.kind} aria-label={t(`conflictKind.${g.kind}`)}>
              <h3 className="group">
                {t(`conflictKind.${g.kind}`)} <span className="n">{g.conflicts.length}</span>
              </h3>
              <ul>{g.conflicts.map(row)}</ul>
            </section>
          ))}
        </div>
      )}
    </aside>
  );
});

/** AUDIT: what in the drawing points at something it does not have (and, listed only, empty
 *  blocks and unused layers), by kind and name; Fix makes the safe repairs as one undo step and
 *  says how many. The core refuses while a block is edited in place; its message is shown. */
import { useEffect, useRef, useState } from "react";
import type { AuditKind, AuditReport, Command, Outcome } from "../api";
import { useT } from "../i18n";

type Props = {
  /** Runs a command that changes nothing (the audit). */
  look(cmd: Command): Promise<Outcome>;
  /** Runs a command that changes the drawing (the fix), redrawing after it. */
  apply(cmd: Command): Promise<Outcome>;
  onClose(): void;
};

/** What the fix repairs: entities on missing layers, texts in missing styles, gone group members. */
const FIXABLE: AuditKind[] = ["missing_layer", "missing_text_style", "group_member_gone"];
/** Listed only, never changed. */
const LISTED: AuditKind[] = ["empty_block", "unused_layer"];

const reportOf = (o: Outcome) => (o.type === "audited" ? o.report : null);

export function AuditDialog({ look, apply, onClose }: Props) {
  const t = useT();
  const dialog = useRef<HTMLDialogElement>(null);
  const [report, setReport] = useState<AuditReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    look({ type: "audit", fix: false })
      .then((o) => setReport(reportOf(o)))
      .catch((e) => setError(String(e)));
    return () => opener?.focus();
  }, []);

  async function fix() {
    setBusy(true);
    try {
      setReport(reportOf(await apply({ type: "audit", fix: true })));
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const canFix = !!report && !report.fix && report.items.some((i) => FIXABLE.includes(i.kind) && !i.fixed);
  const counts = report ? (Object.entries(report.counts) as [AuditKind, number][]).filter(([, n]) => n > 0) : [];

  return (
    <dialog
      ref={dialog}
      className="settings audit"
      aria-labelledby="audit-title"
      onClose={onClose}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <form
        method="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          fix();
        }}
      >
        <h2 id="audit-title" className="heading">
          {t("audit.title")}
        </h2>
        {report?.fix && <p role="status">{t("audit.fixed", { n: report.repaired })}</p>}
        {report && report.items.length === 0 && <p className="status">{t("audit.clean")}</p>}
        {counts.length > 0 && (
          <ul className="audit-counts">
            {counts.map(([kind, n]) => (
              <li key={kind}>
                {t(`audit.kind.${kind}`)}: {n}
              </li>
            ))}
          </ul>
        )}
        {report && report.items.length > 0 && (
          <table className="manage-table" aria-label={t("audit.found")}>
            <thead>
              <tr>
                <th>{t("audit.problem")}</th>
                <th>{t("audit.name")}</th>
                <th>{t("audit.count")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {report.items.map((item) => (
                <tr key={`${item.kind}:${item.name}`} title={item.line}>
                  <td>{t(`audit.kind.${item.kind}`)}</td>
                  <td>
                    <bdi>{item.name}</bdi>
                  </td>
                  <td className="num">{item.count}</td>
                  <td>{item.fixed ? t("audit.itemFixed") : LISTED.includes(item.kind) ? t("audit.listedOnly") : ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <div className="buttons">
          <button type="button" onClick={() => dialog.current?.close()}>
            {t("purge.close")}
          </button>
          {canFix && (
            <button type="submit" className="primary" disabled={busy}>
              {t("audit.fix")}
            </button>
          )}
        </div>
      </form>
    </dialog>
  );
}

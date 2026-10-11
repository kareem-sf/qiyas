import { useEffect, useRef, useState, type ReactNode } from "react";
import type { ReadReport, Recoverable } from "../api";
import { useT } from "../i18n";
import { localTime } from "./recoveryQueue";

/** The file name and the folder of `path`. */
function split(path: string): [string, string] {
  const at = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return at < 0 ? [path, ""] : [path.slice(at + 1), path.slice(0, at)];
}

/** `text` (a translation with `{name}` holes) with each hole filled from `vars` in its own
 *  `<bdi>`, so a date, a PID or a computer's name keeps its own order inside Arabic. */
function fill(text: string, vars: Record<string, string | number>): ReactNode[] {
  return text
    .split(/\{(\w+)\}/)
    .map((part, i) => (i % 2 === 1 ? <bdi key={i}>{vars[part] ?? `{${part}}`}</bdi> : part));
}

/** Shows the modal dialog once mounted, and gives the focus back when it goes. */
function useModal() {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    return () => opener?.focus();
  }, []);
  return dialog;
}

type Props = {
  /** The autosaves to offer back (`recoverableDrawings`), crashed ones first (`enqueue`). */
  items: Recoverable[];
  /** Opens the autosave of the drawing as a new drawing; an error stays in the dialog. */
  onRecover(drawing: string): Promise<void>;
  /** Deletes the autosave of the drawing. */
  onDiscard(drawing: string): Promise<void>;
  /** Opens the drawing itself, open in another Qiyas: its autosave is that Qiyas's, left alone. */
  onOpen(drawing: string): Promise<void>;
  onClose(): void;
};

/** On launch and on open: each autosave newer than its drawing, or left when Qiyas closed
 *  unexpectedly (then whatever its age: the row says when it is older than the drawing), to
 *  recover (opened as a new drawing beside it, then Save As) or discard (deleted). A drawing open
 *  in another Qiyas is warned about and only opens anyway (or Later): Discard would delete that
 *  Qiyas's autosave. */
export function RecoveryDialog({ items, onRecover, onDiscard, onOpen, onClose }: Props) {
  const t = useT();
  const dialog = useModal();
  const crashed = items.some((i) => i.crashed);
  /** Crashed and newer drawings together: the heading speaks of the crash, rows mark their own. */
  const mixed = crashed && items.some((i) => !i.crashed);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function act(f: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <dialog
      ref={dialog}
      className="settings recovery"
      aria-labelledby="recovery-title"
      onClose={onClose}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <h2 id="recovery-title" className="heading">
        {t(crashed ? "recovery.crashedTitle" : "recovery.title")}
      </h2>
      <p>{t("recovery.intro")}</p>
      <ul className="recovery-list">
        {items.map((item) => {
          const [name, folder] = split(item.drawing);
          return (
            <li key={item.drawing}>
              <span className="recovery-file" dir="auto">
                <strong>{name}</strong> <span className="muted">{folder}</span>
              </span>
              {mixed && item.crashed && <span className="warn">{t("recovery.crashedItem")}</span>}
              <span className="muted">{fill(t("recovery.savedAt"), { time: localTime(item.saved_at) })}</span>
              <span className="muted">
                {item.drawing_at == null
                  ? t("recovery.drawingGone")
                  : fill(t("recovery.drawingAt"), { time: localTime(item.drawing_at) })}
              </span>
              {item.drawing_at != null && item.saved_at < item.drawing_at && (
                <span className="warn">{t("recovery.older")}</span>
              )}
              <span className="actions">
                {item.locked_elsewhere ? (
                  <button type="button" disabled={busy} onClick={() => act(() => onOpen(item.drawing))}>
                    {t("recovery.openAnyway")}
                  </button>
                ) : (
                  <>
                    <button type="button" disabled={busy} onClick={() => act(() => onRecover(item.drawing))}>
                      {t("recovery.recover")}
                    </button>
                    <button type="button" disabled={busy} onClick={() => act(() => onDiscard(item.drawing))}>
                      {t("recovery.discard")}
                    </button>
                  </>
                )}
              </span>
              {item.locked_elsewhere && (
                <p className="recovery-warning" role="note">
                  {fill(t("recovery.lockedElsewhere"), {
                    pid: item.locked_elsewhere.pid,
                    host: item.locked_elsewhere.host,
                  })}
                </p>
              )}
            </li>
          );
        })}
      </ul>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="actions">
        <button type="button" onClick={() => dialog.current?.close()}>
          {t("recovery.later")}
        </button>
      </div>
    </dialog>
  );
}

/** RECOVER: what the reader made of a damaged drawing, and what it dropped by kind. */
export function RecoverReportDialog({
  path,
  report,
  onClose,
}: {
  path: string;
  report: ReadReport;
  onClose(): void;
}) {
  const t = useT();
  const dialog = useModal();
  const [name] = split(path);
  return (
    <dialog
      ref={dialog}
      className="settings recovery"
      aria-labelledby="recover-report-title"
      onClose={onClose}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <h2 id="recover-report-title" className="heading">
        {t("recover.title", { name })}
      </h2>
      <p>
        {t("recover.summary", {
          entities: report.entities,
          decoded: report.decoded,
          records: report.records,
          skipped: report.skipped,
        })}
      </p>
      {report.dropped.length === 0 ? (
        <p>{t("recover.nothingDropped")}</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>{t("recover.kind")}</th>
              <th>{t("recover.count")}</th>
            </tr>
          </thead>
          <tbody>
            {report.dropped.map((d) => (
              <tr key={d.kind}>
                <td>{d.kind}</td>
                <td>{d.count}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="actions">
        <button type="button" onClick={() => dialog.current?.close()}>
          {t("recover.close")}
        </button>
      </div>
    </dialog>
  );
}

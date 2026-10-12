import { useEffect, useRef } from "react";
import type { ScriptResult } from "../api";
import { useT } from "../i18n";

/** How long a run took: milliseconds under a second, else seconds to a tenth. */
function took(ms: number, t: ReturnType<typeof useT>): string {
  return ms < 1000 ? t("script.tookMs", { n: ms }) : t("script.tookS", { n: (ms / 1000).toFixed(1) });
}

/** What Run script did (SCRIPT): each line run with a mark for done or failed and what it did or
 *  why it failed, where it stopped (or that a line didn't read and nothing ran), and how long it
 *  took. The script's words and the messages take their own direction (`dir="auto"`), so English
 *  commands read left to right and Arabic ECHO text right to left in either language. */
export function ScriptResultDialog({
  path,
  result,
  onClose,
}: {
  path: string;
  result: ScriptResult;
  onClose(): void;
}) {
  const t = useT();
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    return () => opener?.focus();
  }, []);
  const name = path.split(/[\\/]/).pop() ?? path;
  const failed = result.lines.find((l) => !l.ok);
  const summary = !result.ran
    ? t("script.notRead", { line: failed?.line_no ?? 0 })
    : failed
      ? t("script.stopped", { line: failed.line_no })
      : t("script.ran", { n: new Set(result.lines.map((l) => l.line_no)).size });
  // The file name stays one run of text inside either language's title.
  const [before, after = ""] = t("script.title").split("{name}");
  return (
    <dialog
      ref={dialog}
      className="settings script-result"
      aria-labelledby="script-result-title"
      onClose={onClose}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <h2 id="script-result-title" className="heading">
        {before}
        <bdi>{name}</bdi>
        {after}
      </h2>
      <p>{`${summary} · ${took(result.took_ms, t)}`}</p>
      <div className="extract-table">
        <table>
          <thead>
            <tr>
              <th>{t("script.line")}</th>
              <th>{t("script.command")}</th>
              <th>{t("script.result")}</th>
            </tr>
          </thead>
          <tbody>
            {result.lines.map((l, i) => (
              <tr key={i} className={l.ok ? undefined : "failed"}>
                <td>{l.line_no}</td>
                <td>
                  <code dir="auto">{l.text}</code>
                </td>
                <td>
                  <span role="img" aria-label={t(l.ok ? "script.ok" : "script.error")}>
                    {l.ok ? "✓" : "✗"}
                  </span>{" "}
                  <span dir="auto">{l.message}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="actions">
        <button type="button" onClick={() => dialog.current?.close()}>
          {t("script.close")}
        </button>
      </div>
    </dialog>
  );
}

import { useEffect, useRef, useState } from "react";
import { checkUpdate, installUpdate, NO_UPDATE, UNSAVED_DRAWINGS, UPDATES_NOT_CONFIGURED } from "../api";
import { useLanguage, useT } from "../i18n";
import type { Theme } from "../viewport/gl";
import { version } from "../../package.json";

/** Where "Check for updates" has got to. Nothing installs until Install and restart is clicked. */
type Step =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string; notes: string | null; installing: boolean };

/** The About box: the Qiyas lockup (English or Arabic, ink or white to suit the theme), who makes
 *  it, the version, and Check for updates. While an update installs, the box can't be closed. */
export function About({ theme, onClose }: { theme: Theme; onClose: () => void }) {
  const t = useT();
  const { lang } = useLanguage();
  const dialog = useRef<HTMLDialogElement>(null);
  const [step, setStep] = useState<Step>({ kind: "idle" });
  const [error, setError] = useState<string | null>(null);
  const installing = step.kind === "available" && step.installing;

  useEffect(() => {
    // A modal dialog moves focus inside; give it back to the button that opened it on close.
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    return () => opener?.focus();
  }, []);

  /** The words for a refusal or failure of the updater. */
  function say(e: unknown, key: string): string {
    const text = String(e);
    if (text === UPDATES_NOT_CONFIGURED) return t("about.update.notConfigured");
    if (text === UNSAVED_DRAWINGS) return t("about.update.unsaved");
    if (text === NO_UPDATE) return t("about.update.checkAgain");
    return t(key, { error: text });
  }

  async function check() {
    setError(null);
    setStep({ kind: "checking" });
    try {
      const found = await checkUpdate();
      setStep(
        found.status === "available"
          ? { kind: "available", version: found.version, notes: found.notes, installing: false }
          : { kind: "upToDate" },
      );
    } catch (e) {
      setStep({ kind: "idle" });
      setError(say(e, "about.update.checkFailed"));
    }
  }

  async function install() {
    if (step.kind !== "available") return;
    setError(null);
    setStep({ ...step, installing: true });
    try {
      // Qiyas restarts when it worked, so this only comes back on a failure.
      await installUpdate();
    } catch (e) {
      setStep(String(e) === NO_UPDATE ? { kind: "idle" } : { ...step, installing: false });
      setError(say(e, "about.update.installFailed"));
    }
  }

  const lockup = `/brand/qiyas-lockup${lang === "ar" ? "-ar" : ""}${theme === "dark" ? "-white" : ""}.svg`;

  return (
    <dialog
      ref={dialog}
      className="about"
      aria-labelledby="about-line"
      onClose={onClose}
      onCancel={(e) => {
        if (installing) e.preventDefault();
      }}
      // Keys typed here are not shortcuts for the drawing behind (Escape still closes the dialog).
      onKeyDown={(e) => e.stopPropagation()}
    >
      <img src={lockup} alt="" width={280} />
      <h2 id="about-line" className="heading">
        {t("about.line")}
      </h2>
      <p dir="ltr" className="about-version">
        v{version}
      </p>
      <div className="about-update">
        {step.kind === "upToDate" && <p role="status">{t("about.upToDate")}</p>}
        {step.kind === "available" && (
          <>
            <p role="status">{t("about.updateAvailable", { version: step.version })}</p>
            {step.notes && (
              <p className="about-notes" dir="auto">
                {step.notes}
              </p>
            )}
          </>
        )}
        {error && <p role="alert">{error}</p>}
        {step.kind === "available" ? (
          <button type="button" disabled={installing} onClick={install}>
            {t(installing ? "about.installing" : "about.install")}
          </button>
        ) : (
          <button type="button" disabled={step.kind === "checking"} onClick={check}>
            {t(step.kind === "checking" ? "about.checking" : "about.checkUpdates")}
          </button>
        )}
      </div>
      <button type="button" autoFocus disabled={installing} onClick={() => dialog.current?.close()}>
        {t("about.close")}
      </button>
    </dialog>
  );
}

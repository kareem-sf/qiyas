import { useEffect, useRef } from "react";
import { useLanguage, useT } from "../i18n";
import type { Theme } from "../viewport/gl";
import { version } from "../../package.json";

/** The About box: the Qiyas lockup (English or Arabic, ink or white to suit the theme), who makes it, and the version. */
export function About({ theme, onClose }: { theme: Theme; onClose: () => void }) {
  const t = useT();
  const { lang } = useLanguage();
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    // A modal dialog moves focus inside; give it back to the button that opened it on close.
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    return () => opener?.focus();
  }, []);

  const lockup = `/brand/qiyas-lockup${lang === "ar" ? "-ar" : ""}${theme === "dark" ? "-white" : ""}.svg`;

  return (
    <dialog
      ref={dialog}
      className="about"
      aria-labelledby="about-line"
      onClose={onClose}
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
      <button type="button" autoFocus onClick={() => dialog.current?.close()}>
        {t("about.close")}
      </button>
    </dialog>
  );
}

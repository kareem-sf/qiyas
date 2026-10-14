/** Notices: what the window says once and lets go ("Saved to …", what a modify left out, an error,
 *  the undo that follows a purge), as toasts at the end of the bottom edge, on Sonner. They are
 *  quiet (a thin card in the panel's colours, no icon), stack three at most, go by themselves after
 *  a few seconds (an error stays until it is closed or the next command works), and sit on the side
 *  the text ends on, so in Arabic they are at the bottom left. The status bar keeps to status: what
 *  is happening, the snap, the switches, the cursor. Sonner's own CSS lays the stack out; the look
 *  is ours (`theme.css`, "Notices") from the brand tokens, light or dark. */
import { useEffect, useState } from "react";
import { toast, Toaster } from "sonner";
import { useLanguage, useT } from "../i18n";
import { DotLoader } from "../shell/DotLoader";

/** How long a notice stays, in milliseconds. */
export const NOTICE_MS = 5000;

type Id = string | number;

/** Where the toasts go: the one Toaster of the window. */
export function Notices({ theme }: { theme: "light" | "dark" }) {
  const { lang } = useLanguage();
  const t = useT();
  const rtl = lang === "ar";
  // Sonner keeps its toasts apart from the component: a window that opens starts with none.
  useEffect(() => {
    toast.dismiss();
  }, []);
  return (
    <Toaster
      position={rtl ? "bottom-left" : "bottom-right"}
      dir={rtl ? "rtl" : "ltr"}
      theme={theme}
      duration={NOTICE_MS}
      visibleToasts={3}
      gap={6}
      // Above the status bar and the command line.
      offset={{ bottom: 76, right: 14, left: 14 }}
      mobileOffset={{ bottom: 76, right: 14, left: 14 }}
      containerAriaLabel={t("notices.label")}
      toastOptions={{
        unstyled: true,
        classNames: { toast: "toast", title: "toast-title", closeButton: "toast-close" },
      }}
    />
  );
}

/** Says `text` once. With `waiting` it is about something still going on: the dots beside it, and it
 *  stays until another notice takes its place. */
export const notify = (text: string, opts: { waiting?: boolean } = {}): Id =>
  opts.waiting
    ? toast(
        <span className="toast-wait">
          <DotLoader pattern="pulse" />
          {text}
        </span>,
        { duration: Infinity },
      )
    : toast(text);

/** Says an error: it stays until it is closed, or until `dismissNotice` takes it away. */
export const notifyError = (text: string): Id =>
  toast.error(text, { duration: Infinity, closeButton: true, className: "toast-error" });

export const dismissNotice = (id?: Id) => void toast.dismiss(id);

type Undo = {
  message: string;
  /** The button's words ("Undo"), followed by the seconds left. */
  label: (seconds: number) => string;
  seconds: number;
  /** Takes the change back: the app's own undo. */
  onUndo(): void;
};

/** A toast with an Undo button that counts down and goes when the time is up; a thin line shrinks
 *  with it. Pressing the button runs `onUndo` and closes it. */
function UndoToast({ id, message, label, seconds, onUndo }: Undo & { id: Id }) {
  const [left, setLeft] = useState(seconds);
  useEffect(() => {
    const tick = setInterval(() => setLeft((s) => Math.max(0, s - 1)), 1000);
    return () => clearInterval(tick);
  }, []);
  return (
    <div className="toast toast-undo" role="group" aria-label={message}>
      <span className="toast-title">{message}</span>
      <button
        type="button"
        className="toast-action"
        onClick={() => {
          toast.dismiss(id);
          onUndo();
        }}
      >
        {label(left)}
      </button>
      <span className="toast-timer" style={{ animationDuration: `${seconds}s` }} aria-hidden="true" />
    </div>
  );
}

/** The undo toast on screen, so another command can take it away: its Undo would no longer be the
 *  last step. */
let undoToast: Id | null = null;

/** A notice of something done that can be taken back for `seconds` seconds. */
export function notifyUndo(undo: Undo): Id {
  if (undoToast !== null) toast.dismiss(undoToast);
  undoToast = toast.custom((id) => <UndoToast id={id} {...undo} />, { duration: undo.seconds * 1000 });
  return undoToast;
}

/** Takes the undo toast away: something else was done after it. */
export function notifyUndoGone() {
  if (undoToast === null) return;
  toast.dismiss(undoToast);
  undoToast = null;
}

/** The label of an undo toast in the window's language: "Undo (5 s)". */
export function useUndoLabel() {
  const t = useT();
  return (seconds: number) => t("toast.undoIn", { n: seconds });
}

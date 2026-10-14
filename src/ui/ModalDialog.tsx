/** The window's modal dialog, on Base UI's Dialog: a centred box over a dimmed backdrop, the focus kept
 *  inside while it is open and given back to the button that opened it, Escape closing it. The
 *  box keeps the `settings` look of the dialogs before it (`className` adds the rest). A click
 *  outside does not close it (it never did: a half-filled form is not thrown away), and the keys
 *  typed in it are not shortcuts for the drawing behind. The caller mounts it while it is open and
 *  gets `onClose` when it should go; `open` false (the Plot dialog stepping aside for two clicks in
 *  the drawing) hides it without closing. */
import { Dialog } from "@base-ui/react/dialog";
import type { CSSProperties, ReactNode } from "react";

type Props = {
  onClose(): void;
  /** Added to `settings`: `purge`, `plot-dialog`... */
  className?: string;
  /** The id of the heading that names the dialog. */
  labelledBy: string;
  style?: CSSProperties;
  /** Hidden while false; the dialog's own state stays. */
  open?: boolean;
  /** False while Escape must not close it (a job is running and its result would be lost). */
  dismissible?: boolean;
  /** Keeps what is inside in the page while it is hidden (a drawn preview). */
  keepMounted?: boolean;
  children: ReactNode;
};

export function ModalDialog({
  onClose,
  className = "",
  labelledBy,
  style,
  open = true,
  dismissible = true,
  keepMounted = false,
  children,
}: Props) {
  return (
    <Dialog.Root
      open={open}
      modal
      disablePointerDismissal
      onOpenChange={(next, details) => {
        if (next) return;
        if (!dismissible) {
          details.cancel();
          return;
        }
        onClose();
      }}
    >
      <Dialog.Portal keepMounted={keepMounted}>
        <Dialog.Backdrop className="dialog-backdrop" />
        <Dialog.Popup
          className={`dialog-popup settings ${className}`.trim()}
          aria-labelledby={labelledBy}
          style={style}
          onKeyDown={(e) => e.stopPropagation()}
        >
          {children}
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

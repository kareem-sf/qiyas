/** Space is the viewport's pan key, but on a focused button it would press the button again. A
 *  mouse click leaves its button focused, so once the click has been handled, let the focus go.
 *  Buttons inside dialogs keep it (the dialog's own keys need it). Returns the undo. */
export function blurAfterClick(): () => void {
  const onPointerUp = () => {
    // After the click handler, which may have moved the focus on by itself.
    setTimeout(() => {
      const el = document.activeElement;
      if (el instanceof HTMLButtonElement && !el.closest("dialog, [role='dialog']")) el.blur();
    }, 0);
  };
  document.addEventListener("pointerup", onPointerUp, true);
  return () => document.removeEventListener("pointerup", onPointerUp, true);
}

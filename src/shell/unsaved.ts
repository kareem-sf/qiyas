/** Leaving a drawing with unsaved changes (PDF imports, underlays, calibrations): ask to save. */

export type UnsavedChoice = "save" | "discard" | "cancel";

export type UnsavedLabels = { yes: string; no: string; cancel: string };

/** The choice behind the answer of the Yes / No / Cancel message dialog: the dialog answers
 *  "Yes", "No" or "Cancel", or with custom labels on some systems the label pressed. Anything else
 *  (the dialog closed) is cancel. */
export function readChoice(answer: string, labels: UnsavedLabels): UnsavedChoice {
  if (answer === "Yes" || answer === labels.yes) return "save";
  if (answer === "No" || answer === labels.no) return "discard";
  return "cancel";
}

/** Whether the drawing may be left: yes when it has no unsaved changes, when the user chose not to
 *  save, or chose to save and the save went through (a failed save throws). No when cancelled, at
 *  the question or at `confirmSave` (the warning a save gives first, e.g. for a recovered drawing). */
export async function leaveDrawing(steps: {
  isDirty(): Promise<boolean>;
  ask(): Promise<UnsavedChoice>;
  confirmSave(): Promise<boolean>;
  save(): Promise<void>;
}): Promise<boolean> {
  if (!(await steps.isDirty())) return true;
  const choice = await steps.ask();
  if (choice === "cancel") return false;
  if (choice === "save") {
    if (!(await steps.confirmSave())) return false;
    await steps.save();
  }
  return true;
}

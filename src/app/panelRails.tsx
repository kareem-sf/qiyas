// The two side panels (layers and properties; take-off, BOQ, conflicts, Mizan) fold away to give
// the drawing the whole width. Each has a thin rail on its inner edge that hides or shows it;
// the choice is remembered on this computer.
import { useState } from "react";
import { useT } from "../i18n";

export type PanelSide = "left" | "right";

const KEY = "qiyas.panels.hidden";

function stored(): Record<PanelSide, boolean> {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { left: raw.left === true, right: raw.right === true };
  } catch {
    // Storage blocked or not JSON: both panels shown.
    return { left: false, right: false };
  }
}

/** Which side panels are hidden, and the toggle that also remembers it. */
export function usePanelRails() {
  const [hidden, setHidden] = useState(stored);
  const toggle = (side: PanelSide) =>
    setHidden((was) => {
      const next = { ...was, [side]: !was[side] };
      try {
        localStorage.setItem(KEY, JSON.stringify(next));
      } catch {
        // Not remembered; the panel still folds.
      }
      return next;
    });
  return { hidden, toggle };
}

/** The rail between a side panel and the drawing: one button, hide or show. */
export function PanelRail({ side, hidden, onToggle }: { side: PanelSide; hidden: boolean; onToggle: () => void }) {
  const t = useT();
  const label = t(`panels.${hidden ? "show" : "hide"}.${side}`);
  return (
    <button
      type="button"
      className={`panel-rail ${side}`}
      aria-expanded={!hidden}
      aria-label={label}
      title={label}
      onClick={onToggle}
    />
  );
}

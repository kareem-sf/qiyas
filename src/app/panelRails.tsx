// The two side panels (layers and properties; take-off, BOQ, conflicts, Mizan) can be dragged
// wider or narrower and folded away to give the drawing the whole width. Between each panel and
// the drawing is a handle: drag it to resize, double-click it (or its round button) to fold or
// unfold, arrows on the keyboard to resize. Widths and folds are remembered on this computer.
import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useT } from "../i18n";

export type PanelSide = "left" | "right";

const KEY = "qiyas.panels.hidden";
const WIDTH_KEY = "qiyas.panels.width";

/** Each panel's width in CSS pixels: the default, the narrowest and the widest it can be. */
export const PANEL_WIDTH: Record<PanelSide, { initial: number; min: number; max: number }> = {
  left: { initial: 220, min: 180, max: 480 },
  right: { initial: 340, min: 260, max: 760 },
};
/** A drag that leaves less than this much of a panel folds it. */
const FOLD_BELOW = 0.6;
/** One arrow key press. */
const STEP = 16;

const clampWidth = (side: PanelSide, w: number) => Math.round(Math.min(PANEL_WIDTH[side].max, Math.max(PANEL_WIDTH[side].min, w)));

function storedHidden(): Record<PanelSide, boolean> {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { left: raw.left === true, right: raw.right === true };
  } catch {
    // Storage blocked or not JSON: both panels shown.
    return { left: false, right: false };
  }
}

function storedWidth(): Record<PanelSide, number> {
  const width = (side: PanelSide, v: unknown) =>
    typeof v === "number" && Number.isFinite(v) ? clampWidth(side, v) : PANEL_WIDTH[side].initial;
  try {
    const raw = JSON.parse(localStorage.getItem(WIDTH_KEY) ?? "{}") ?? {};
    return { left: width("left", raw.left), right: width("right", raw.right) };
  } catch {
    return { left: PANEL_WIDTH.left.initial, right: PANEL_WIDTH.right.initial };
  }
}

function keep(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Not remembered; the panel still changes for this session.
  }
}

/** Which side panels are hidden and how wide each is, with the changes that also remember them. */
export function usePanelRails() {
  const [hidden, setHidden] = useState(storedHidden);
  const [width, setWidth] = useState(storedWidth);
  /** Folds or unfolds a panel; without `folded`, the other way round from now. */
  const setFolded = (side: PanelSide, folded?: boolean) =>
    setHidden((was) => {
      const to = folded ?? !was[side];
      if (was[side] === to) return was;
      const next = { ...was, [side]: to };
      keep(KEY, next);
      return next;
    });
  const toggle = (side: PanelSide) => setFolded(side);
  const resize = (side: PanelSide, w: number) =>
    setWidth((was) => {
      const next = { ...was, [side]: clampWidth(side, w) };
      keep(WIDTH_KEY, next);
      return next;
    });
  return { hidden, width, toggle, setFolded, resize };
}

type HandleProps = {
  side: PanelSide;
  hidden: boolean;
  width: number;
  onToggle(): void;
  onFold(folded: boolean): void;
  onResize(width: number): void;
};

/** The handle between a side panel and the drawing. */
export function PanelHandle({ side, hidden, width, onToggle, onFold, onResize }: HandleProps) {
  const t = useT();
  const drag = useRef<{ x: number; width: number; rtl: boolean } | null>(null);
  const [dragging, setDragging] = useState(false);
  const label = t(`panels.${hidden ? "show" : "hide"}.${side}`);

  // The left panel is on the inline start: dragging towards the drawing widens it. In Arabic the
  // sides swap, so the pointer's direction does too.
  const grow = (dx: number, rtl: boolean) => (side === "left" ? 1 : -1) * (rtl ? -1 : 1) * dx;

  function onPointerDown(e: PointerEvent<HTMLDivElement>) {
    if (e.button !== 0 || (e.target as Element).closest("button")) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    const rtl = getComputedStyle(e.currentTarget).direction === "rtl";
    drag.current = { x: e.clientX, width: hidden ? 0 : width, rtl };
    setDragging(true);
  }
  function onPointerMove(e: PointerEvent<HTMLDivElement>) {
    const d = drag.current;
    if (!d) return;
    const w = d.width + grow(e.clientX - d.x, d.rtl);
    if (w < PANEL_WIDTH[side].min * FOLD_BELOW) {
      onFold(true);
      return;
    }
    onFold(false);
    onResize(w);
  }
  function onPointerUp(e: PointerEvent<HTMLDivElement>) {
    if (!drag.current) return;
    drag.current = null;
    setDragging(false);
    e.currentTarget.releasePointerCapture?.(e.pointerId);
  }
  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (e.target !== e.currentTarget) return;
    const rtl = getComputedStyle(e.currentTarget).direction === "rtl";
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      const dx = e.key === "ArrowRight" ? STEP : -STEP;
      if (hidden) onFold(false);
      onResize((hidden ? PANEL_WIDTH[side].min : width) + grow(dx, rtl));
    } else if (e.key === "Enter") {
      e.preventDefault();
      onToggle();
    }
  }

  return (
    <div
      className={`panel-handle at-${side}${hidden ? " folded" : ""}${dragging ? " dragging" : ""}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={t(`panels.resize.${side}`)}
      aria-valuemin={0}
      aria-valuemax={PANEL_WIDTH[side].max}
      aria-valuenow={hidden ? 0 : width}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onDoubleClick={onToggle}
      onKeyDown={onKeyDown}
    >
      <button
        type="button"
        className="panel-grip"
        aria-expanded={!hidden}
        aria-label={label}
        title={label}
        onClick={onToggle}
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M10 3.5L5.5 8l4.5 4.5" />
        </svg>
      </button>
    </div>
  );
}

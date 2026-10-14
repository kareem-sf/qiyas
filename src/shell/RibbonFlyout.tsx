/** A ribbon flyout: the list behind a group's "⋯" button and behind the tabs' "More". It is placed
 *  on the window (`fixed`) under its opener, its start edge under the opener's start edge (so it
 *  mirrors in Arabic) and kept inside the window, because the ribbon's panel scrolls and would
 *  clip a menu inside it. The keyboard reaches it: it takes the focus when it opens, the arrows,
 *  Home and End move between its items, Escape closes it and gives the focus back to the opener,
 *  so does Tab, and so does a click outside. Items are buttons with a `menuitem` role. */
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { nextIndex } from "./useRoving";

type Props = {
  /** The button that opened it. */
  anchor: HTMLElement;
  label: string;
  /** Items to a column: the items fill a column before the next (arrows Up and Down move within
   *  it, Left and Right between columns). */
  rows: number;
  /** `restoreFocus`: put the focus back on the opener (a key closed it). */
  onClose(restoreFocus: boolean): void;
  children: ReactNode;
};

const MARGIN = 8;

export function RibbonFlyout({ anchor, label, rows, onClose, children }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<CSSProperties>({ position: "fixed", top: 0, left: 0, visibility: "hidden" });
  const close = useRef(onClose);
  close.current = onClose;

  useLayoutEffect(() => {
    const menu = ref.current;
    if (!menu) return;
    const a = anchor.getBoundingClientRect();
    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    const rtl = document.documentElement.dir === "rtl";
    const left = Math.max(MARGIN, Math.min(rtl ? a.right - w : a.left, window.innerWidth - w - MARGIN));
    const below = a.bottom + 4;
    const top = below + h + MARGIN > window.innerHeight && a.top - h - 4 >= MARGIN ? a.top - h - 4 : below;
    setAt({ position: "fixed", top, left });
  }, [anchor]);

  useEffect(() => {
    items()[0]?.focus();
    const outside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!ref.current?.contains(target) && !anchor.contains(target)) close.current(false);
    };
    const escape = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Escape closes the flyout only: it never reaches the window's handler, which would also
      // cancel the tool and the selection.
      e.stopPropagation();
      close.current(true);
    };
    document.addEventListener("mousedown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", outside);
      document.removeEventListener("keydown", escape);
    };
    // Once, when it opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const items = () => Array.from(ref.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]:not(:disabled)') ?? []);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Tab") {
      close.current(true);
      return;
    }
    const all = items();
    const at = all.indexOf(e.target as HTMLElement);
    if (at < 0) return;
    const rtl = document.documentElement.dir === "rtl";
    let to: number | null = null;
    if (e.key === "ArrowDown") to = (at + 1) % all.length;
    else if (e.key === "ArrowUp") to = (at - 1 + all.length) % all.length;
    else if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      // Between columns: the next column is the one on the end side.
      const step = (e.key === "ArrowRight") !== rtl ? rows : -rows;
      if (at + step >= 0 && at + step < all.length) to = at + step;
      else to = at;
    } else to = nextIndex(e.key, at, all.length);
    if (to === null) return;
    e.preventDefault();
    all[to].focus();
  };

  return (
    <div
      ref={ref}
      className="rb-flyout"
      role="menu"
      aria-label={label}
      style={{ ...at, gridTemplateRows: `repeat(${rows}, auto)` }}
      onKeyDown={onKeyDown}
    >
      {children}
    </div>
  );
}

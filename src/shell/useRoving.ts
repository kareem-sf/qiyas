import { useRef, useState, type FocusEvent, type KeyboardEvent } from "react";

/** The tab to focus after `key` from tab `i` of `n` (arrows go round); null for any other key. In
 *  a right-to-left page the arrows are mirrored. */
export function nextIndex(key: string, i: number, n: number, rtl = false): number | null {
  if (n <= 0) return null;
  const step = key === "ArrowRight" ? (rtl ? -1 : 1) : key === "ArrowLeft" ? (rtl ? 1 : -1) : 0;
  if (step !== 0) return (i + step + n) % n;
  if (key === "Home") return 0;
  if (key === "End") return n - 1;
  return null;
}

/** Roving tabindex for a `role="tablist"` of `n` tabs with tab `active` selected: one tab is in
 *  the Tab order (the focused one, else the selected one), the arrows, Home and End move focus,
 *  Enter and Space press the focused tab. Spread `props` on the tablist; give each tab
 *  `role="tab"` and `tabIndex={tabIndex(i)}`. Space is the viewport's pan key, so the tab keeps it. */
export function useRoving<T extends HTMLElement = HTMLDivElement>(n: number, active: number) {
  const ref = useRef<T>(null);
  const [focus, setFocus] = useState<number | null>(null);
  const tabs = () => Array.from(ref.current?.querySelectorAll<HTMLElement>('[role="tab"]') ?? []);
  const current = focus !== null && focus < n ? focus : active >= 0 && active < n ? active : 0;

  const onFocus = (e: FocusEvent) => {
    const at = tabs().indexOf(e.target as HTMLElement);
    if (at >= 0) setFocus(at);
  };
  const onBlur = (e: FocusEvent) => {
    if (!ref.current?.contains(e.relatedTarget as Node | null)) setFocus(null);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const all = tabs();
    const at = all.indexOf(e.target as HTMLElement);
    if (at < 0) return;
    if (e.key === " ") {
      e.preventDefault();
      all[at].click();
      return;
    }
    const to = nextIndex(e.key, at, all.length, document.documentElement.dir === "rtl");
    if (to === null) return;
    e.preventDefault();
    all[to].focus();
  };
  // A pressed Space would click a button again when it is released.
  const onKeyUp = (e: KeyboardEvent) => {
    if (e.key === " " && (e.target as HTMLElement).getAttribute("role") === "tab") e.preventDefault();
  };

  return {
    props: { ref, onFocus, onBlur, onKeyDown, onKeyUp },
    tabIndex: (i: number) => (i === current ? 0 : -1),
  };
}

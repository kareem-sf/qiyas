import { useLayoutEffect, useRef, useState, type FocusEvent, type KeyboardEvent } from "react";

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

/** The same for a `role="toolbar"` of buttons the caller does not number (a split button is two
 *  stops): the enabled buttons outside any menu are the stops, one of them (the last focused,
 *  else the first) is in the Tab order, and the arrows (mirrored in a right-to-left page), Home
 *  and End move focus. Spread `props` on the toolbar; Space stays with the buttons. */
export function useToolbarRoving<T extends HTMLElement = HTMLDivElement>() {
  const ref = useRef<T>(null);
  const last = useRef<HTMLElement | null>(null);
  const stops = () =>
    Array.from(ref.current?.querySelectorAll<HTMLElement>('button:not([role="menuitem"]):not(:disabled)') ?? []);
  const apply = () => {
    const all = stops();
    const keep = last.current && all.includes(last.current) ? last.current : all[0];
    for (const b of all) b.tabIndex = b === keep ? 0 : -1;
  };
  // After every render: a button enabled or taken away meanwhile changes the stops.
  useLayoutEffect(apply);

  const onFocus = (e: FocusEvent) => {
    if (stops().includes(e.target as HTMLElement)) {
      last.current = e.target as HTMLElement;
      apply();
    }
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const all = stops();
    const at = all.indexOf(e.target as HTMLElement);
    if (at < 0) return;
    const to = nextIndex(e.key, at, all.length, document.documentElement.dir === "rtl");
    if (to === null) return;
    e.preventDefault();
    all[to].focus();
  };
  return { props: { ref, onFocus, onKeyDown } };
}

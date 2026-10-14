/** The side panel's tab row, on Base UI's Tabs: one indicator slides under the chosen tab, the arrows
 *  move between tabs, and the labels never wrap. When the panel is too narrow for every tab, the
 *  last ones fold into a "⋯" menu at the row's end (like the ribbon's More): the menu holds them,
 *  and stands for the chosen one while that is among them. Every tab stays in the page (hidden
 *  and disabled while folded), so its width is known and nothing is measured twice. */
import { Menu } from "@base-ui/react/menu";
import { Tabs } from "@base-ui/react/tabs";
import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useT } from "../i18n";

export type SideTabItem<Id extends string> = {
  id: Id;
  label: string;
  /** What follows the label: a count, a loader. */
  extra?: ReactNode;
};

type Props<Id extends string> = {
  tabs: SideTabItem<Id>[];
  value: Id;
  onChange(id: Id): void;
  /** The class of the box holding the tabs and the panels (`side`, or `side wide`). */
  className: string;
  /** What is under the tab row: the panels (`Tabs.Panel`). */
  children: ReactNode;
};

/** The sentinel for the "⋯" tab: it is chosen while the tab chosen is one of the folded ones. */
const FOLDED = "\u0000folded";

export function SideTabs<Id extends string>({ tabs, value, onChange, className, children }: Props<Id>) {
  const t = useT();
  const list = useRef<HTMLDivElement>(null);
  const more = useRef<HTMLButtonElement>(null);
  /** How many of the tabs, from the first, fit in the row beside the "⋯". */
  const [fit, setFit] = useState(tabs.length);
  const [open, setOpen] = useState(false);
  const shown = Math.min(fit, tabs.length);
  const folded = tabs.slice(shown);
  const chosenFolded = folded.some((x) => x.id === value);

  // How many tabs fit: their widths are known whether folded or not (a folded one is only hidden),
  // so one pass over them decides, before the row is painted.
  const measure = () => {
    const row = list.current;
    if (!row) return;
    const gap = parseFloat(getComputedStyle(row).columnGap) || 0;
    const widths = Array.from(row.querySelectorAll<HTMLElement>("[data-side-tab]")).map((el) => el.offsetWidth);
    const room = row.clientWidth;
    const moreWidth = more.current?.offsetWidth || 28;
    const total = widths.reduce((sum, w) => sum + w, 0) + gap * Math.max(0, widths.length - 1);
    // Everything fits, or as many as fit beside the "⋯" (always one, and one folded).
    let k = widths.length;
    if (total > room) {
      let used = moreWidth;
      k = 0;
      while (k < widths.length - 1 && used + widths[k] + gap <= room) used += widths[k++] + gap;
      k = Math.max(1, k);
    }
    setFit(k);
  };
  const labels = tabs.map((x) => `${x.id}:${x.label}`).join("|");
  // Before paint: a new language or tab set. After it: the row, or a tab (a count), changed size.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useLayoutEffect(measure, [labels]);
  useLayoutEffect(() => {
    const row = list.current;
    if (!row || typeof ResizeObserver === "undefined") return;
    const watch = new ResizeObserver(measure);
    watch.observe(row);
    row.querySelectorAll("[data-side-tab]").forEach((el) => watch.observe(el));
    return () => watch.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [labels]);

  return (
    <Tabs.Root
      className={className}
      value={chosenFolded ? FOLDED : value}
      onValueChange={(next) => {
        // The "⋯" tab opens its menu; the tab it stands for is chosen there.
        if (next !== FOLDED) onChange(next as Id);
      }}
    >
      <Tabs.List className="side-tabs" ref={list}>
        {tabs.map((x, i) => (
          <Tabs.Tab
            key={x.id}
            value={x.id}
            id={`tab-${x.id}`}
            className={i < shown ? "side-tab" : "side-tab folded"}
            disabled={i >= shown}
            data-side-tab=""
          >
            {x.label}
            {x.extra}
          </Tabs.Tab>
        ))}
        {folded.length > 0 && (
        <Menu.Root open={open} onOpenChange={setOpen} modal={false}>
          <Menu.Trigger
            ref={more}
            render={<Tabs.Tab value={FOLDED} className="side-tab side-tab-more" />}
            aria-label={t("panels.moreTabs")}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M3.5 8h.01M8 8h.01M12.5 8h.01" />
            </svg>
          </Menu.Trigger>
          <Menu.Portal>
            <Menu.Positioner className="rb-flyout-pos" side="bottom" align="end" sideOffset={4} collisionPadding={8}>
              <Menu.Popup
                className="rb-flyout side-more-menu"
                aria-label={t("panels.moreTabs")}
                onKeyDown={(e) => e.key === "Escape" && e.stopPropagation()}
              >
                <Menu.RadioGroup value={value} onValueChange={(next) => onChange(next as Id)}>
                  {folded.map((x) => (
                    <Menu.RadioItem key={x.id} value={x.id} closeOnClick className="rb-item">
                      {x.label}
                      {x.extra}
                    </Menu.RadioItem>
                  ))}
                </Menu.RadioGroup>
              </Menu.Popup>
            </Menu.Positioner>
          </Menu.Portal>
        </Menu.Root>
        )}
        <Tabs.Indicator className="tab-indicator side-indicator" />
      </Tabs.List>
      {children}
    </Tabs.Root>
  );
}

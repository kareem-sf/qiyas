/** A ribbon flyout, on Base UI's Menu: the list behind a group's "⋯" button and behind the tabs'
 *  "More". The popup is placed on the window under its opener, its start edge under the opener's
 *  start edge (mirrored in Arabic) and kept inside the window, so the ribbon's scrolling row cannot
 *  clip it. The keyboard reaches it: it takes the focus when it opens, the arrows, Home and End move
 *  between its items (Left and Right between columns), Escape closes it and gives the focus back to
 *  the opener, and so does a click outside. Items have the `menuitem` roles. `children` holds the
 *  opener (a `Menu.Trigger`), `items` the rows. */
import { Menu } from "@base-ui/react/menu";
import { useRef, type KeyboardEvent, type ReactNode } from "react";

type Props = {
  open: boolean;
  onOpenChange(open: boolean): void;
  label: string;
  /** Items to a column: the items fill a column before the next. */
  rows: number;
  /** The opener: a `Menu.Trigger`. */
  children: ReactNode;
  /** The rows: `Menu.Item`, `Menu.CheckboxItem`, `Menu.RadioGroup`. */
  items: ReactNode;
};

export function RibbonFlyout({ open, onOpenChange, label, rows, children, items }: Props) {
  const popup = useRef<HTMLDivElement>(null);

  // Base UI moves between the rows with Up and Down; between columns is ours: the next column is
  // the one on the end side, and an item with no neighbour that way stays put.
  const onKeyDown = (e: KeyboardEvent) => {
    // Escape closes the flyout only: it does not also cancel the tool and the selection.
    if (e.key === "Escape") e.stopPropagation();
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    const all = Array.from(popup.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"])') ?? []);
    const at = all.indexOf(e.target as HTMLElement);
    if (at < 0 || rows >= all.length) return;
    e.preventDefault();
    e.stopPropagation();
    const rtl = document.documentElement.dir === "rtl";
    const step = (e.key === "ArrowRight") !== rtl ? rows : -rows;
    all[at + step >= 0 && at + step < all.length ? at + step : at].focus();
  };

  return (
    <Menu.Root open={open} onOpenChange={onOpenChange} modal={false}>
      {children}
      <Menu.Portal>
        <Menu.Positioner className="rb-flyout-pos" side="bottom" align="start" sideOffset={4} collisionPadding={8}>
          <Menu.Popup
            ref={popup}
            className="rb-flyout"
            aria-label={label}
            style={{ gridTemplateRows: `repeat(${rows}, auto)` }}
            onKeyDown={onKeyDown}
          >
            {items}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

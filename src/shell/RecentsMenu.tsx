import { useState, type KeyboardEvent } from "react";
import type { Recent } from "../api";
import { useT } from "../i18n";
import { useMenu } from "./useMenu";

type Props = {
  /** Open…: the file dialog. */
  onOpen(): void;
  /** Reads the recent list, each time the menu opens. */
  load(): Promise<Recent[]>;
  onOpenRecent(path: string): void;
  onClear(): Promise<void>;
  onKey?(e: KeyboardEvent): void;
};

/** The ribbon's Open split button: Open… and, beside it, the recent drawings (file name, its
 *  folder muted; greyed with "(missing)" when the file is gone), then Clear recents. */
export function RecentsMenu({ onOpen, load, onOpenRecent, onClear, onKey }: Props) {
  const t = useT();
  const menu = useMenu<HTMLDivElement>();
  const [list, setList] = useState<Recent[]>([]);

  async function toggle() {
    if (menu.open) {
      menu.setOpen(false);
      return;
    }
    setList(await load().catch(() => []));
    menu.setOpen(true);
  }

  return (
    <div className="split" ref={menu.ref}>
      <button type="button" onClick={onOpen} onKeyDown={onKey} onKeyUp={onKey}>
        {t("open")}
      </button>
      <button
        type="button"
        className="split-arrow"
        aria-haspopup="menu"
        aria-expanded={menu.open}
        aria-label={t("recent.title")}
        title={t("recent.title")}
        onClick={toggle}
        onKeyDown={onKey}
        onKeyUp={onKey}
      >
        ▾
      </button>
      {menu.open && (
        <div className="menu" role="menu" aria-label={t("recent.title")}>
          {list.length === 0 && <div className="menu-note">{t("recent.none")}</div>}
          {list.map((r) => (
            <button
              key={r.path}
              type="button"
              role="menuitem"
              className="recent"
              disabled={r.missing}
              title={r.path}
              onClick={() => {
                menu.setOpen(false);
                onOpenRecent(r.path);
              }}
            >
              <span>
                <bdi>{r.name}</bdi>
                {r.missing && ` ${t("recent.missing")}`}
              </span>
              <span className="muted">
                <bdi>{r.folder}</bdi>
              </span>
            </button>
          ))}
          {list.length > 0 && (
            <>
              <hr />
              <button
                type="button"
                role="menuitem"
                onClick={async () => {
                  await onClear().catch(() => {});
                  setList([]);
                  menu.setOpen(false);
                }}
              >
                {t("recent.clear")}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}

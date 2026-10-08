import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { BoqItem } from "../api";
import { useT } from "../i18n";
import { matches, unitLabel } from "./boqMath";

type Props = {
  /** What is being linked, for the heading. */
  title: string;
  items: BoqItem[];
  /** The code the row is linked to now; Unlink is offered when there is one. */
  current: string | null;
  /** A BOQ code, or null to unlink. */
  onPick(code: string | null): void;
  onClose(): void;
};

/** Picks the BOQ item a take-off row is linked to: a search box over the items ("code ·
 *  description · unit"); arrows move, Enter or a click picks, Escape or a click outside closes. */
export function LinkPicker({ title, items, current, onPick, onClose }: Props) {
  const t = useT();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);
  // An item without a code can't be linked to.
  const found = items.filter((i) => i.code !== "" && matches(i, query));

  useEffect(() => {
    function outside(e: PointerEvent) {
      if (!box.current?.contains(e.target as Node)) onClose();
    }
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [onClose]);

  useEffect(() => {
    list.current?.children[active]?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((a) => Math.min(Math.max(a + step, 0), Math.max(found.length - 1, 0)));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (found[active]) onPick(found[active].code);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  return (
    <div className="link-picker" role="dialog" aria-label={`${t("boq.link")}: ${title}`} ref={box}>
      <div className="link-picker-head">
        <span className="heading" title={title}>
          {t("boq.link")}: {title}
        </span>
        {current !== null && (
          <button type="button" onClick={() => onPick(null)}>
            {t("boq.unlink")}
          </button>
        )}
      </div>
      <input
        type="search"
        aria-label={t("boq.search")}
        placeholder={t("boq.search")}
        value={query}
        autoFocus
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      {found.length === 0 ? (
        <p className="empty">{t("boq.noMatch")}</p>
      ) : (
        <ul role="listbox" aria-label={t("boq")} ref={list}>
          {found.map((item, i) => (
            <li
              key={`${item.row}-${item.code}`}
              role="option"
              aria-selected={i === active}
              className={item.code === current ? "linked" : undefined}
              onPointerEnter={() => setActive(i)}
              onClick={() => onPick(item.code)}
            >
              <span className="code">{item.code}</span>
              {" · "}
              <span className="text">{item.description}</span>
              {" · "}
              <span>{unitLabel(t, item.unit)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

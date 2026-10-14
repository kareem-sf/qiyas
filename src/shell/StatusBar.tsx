import { useEffect, useRef } from "react";
import type { Drafting, Ucs } from "../api";
import { useT } from "../i18n";
import type { DraftingToggle } from "../tools/shortcuts";
import { isWorld, toUcs } from "../tools/ucs";
import { DotLoader, type DotPattern } from "./DotLoader";
import { useMenu } from "./useMenu";

type Props = {
  cursor: [number, number] | null;
  units: string;
  unitsWarning: boolean;
  /** The drawing's reading problems, one line each; a button with their count opens the list. */
  problems: string[];
  message: string;
  /** The motion to show beside the message while something waits (opening, comparing, a picture
   *  drawing); null or absent for none. The message keeps saying what it is. */
  waiting?: DotPattern | null;
  /** Decimals of the cursor position. */
  precision?: number;
  /** Metres per drawing unit: the cursor is shown in metres. */
  metresPerUnit?: number;
  /** The name of the object snap under the cursor, or null. */
  snapLabel?: string | null;
  /** The drawing's drafting settings; null (no drawing) leaves the switches off. */
  drafting?: Drafting | null;
  onToggle?(toggle: DraftingToggle): void;
  /** Opens the drafting settings. */
  onDraftingSettings?(): void;
  /** The user coordinate system: the cursor is shown in it, marked "UCS". */
  ucs?: Ucs | null;
};

const TOGGLES: { toggle: DraftingToggle; key: string; on: (d: Drafting) => boolean }[] = [
  { toggle: "osnap", key: "F3", on: (d) => d.osnap },
  { toggle: "ortho", key: "F8", on: (d) => d.ortho },
  { toggle: "polar", key: "F10", on: (d) => d.polar },
  { toggle: "grid", key: "F7", on: (d) => d.grid },
];

/** Bottom line: what is happening, the snap under the cursor, the drafting switches, cursor
 *  position in metres, units and file warnings. */
export function StatusBar({
  cursor,
  units,
  unitsWarning,
  problems,
  message,
  waiting = null,
  precision = 3,
  metresPerUnit = 1,
  snapLabel = null,
  drafting = null,
  onToggle,
  onDraftingSettings,
  ucs = null,
}: Props) {
  const t = useT();
  const metres = (v: number) => (v * metresPerUnit).toFixed(precision);
  const shown = (p: [number, number]) => {
    const [x, y] = ucs && !isWorld(ucs) ? toUcs(ucs, p) : p;
    const xy = `${metres(x)}, ${metres(y)} ${t("qty.m")}`;
    return ucs && !isWorld(ucs) ? `${t("status.ucs")} ${xy}` : xy;
  };
  const { open, setOpen, ref } = useMenu<HTMLSpanElement>();
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);
  // The list takes the focus when it opens, so the keyboard can scroll it.
  useEffect(() => {
    if (open) list.current?.focus();
  }, [open]);
  // Nothing to list any more (another drawing): close it.
  useEffect(() => {
    if (problems.length === 0) setOpen(false);
  }, [problems.length, setOpen]);
  return (
    <footer className="status">
      <span className="message" role="status" aria-live="polite">
        {waiting && <DotLoader pattern={waiting} />}
        {message}
      </span>
      {snapLabel && <span className="snap-kind">{snapLabel}</span>}
      {problems.length > 0 && (
        <span
          className="problems"
          ref={ref}
          onKeyDown={(e) => {
            if (e.key !== "Escape" || !open) return;
            e.stopPropagation();
            setOpen(false);
            button.current?.focus();
          }}
        >
          <button
            type="button"
            className="warn"
            ref={button}
            aria-haspopup="true"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            {t("recovered", { n: problems.length })}
          </button>
          {open && (
            <ul className="problem-list" ref={list} tabIndex={-1} aria-label={t("problems")}>
              {problems.map((p, i) => (
                <li key={i}>{p}</li>
              ))}
            </ul>
          )}
        </span>
      )}
      {unitsWarning && <span className="warn">{t("unitsUnknown")}</span>}
      {units && <span>{units}</span>}
      <span className="drafting" role="group" aria-label={t("drafting")}>
        {TOGGLES.map(({ toggle, key, on }) => (
          <button
            key={toggle}
            type="button"
            aria-pressed={drafting ? on(drafting) : false}
            disabled={!drafting || !onToggle}
            title={`${t(`drafting.${toggle}Title`)} (${key})`}
            onClick={() => onToggle?.(toggle)}
          >
            {t(`status.${toggle}`)}
          </button>
        ))}
        <button
          type="button"
          className="gear"
          aria-haspopup="dialog"
          aria-label={t("drafting")}
          title={t("drafting")}
          disabled={!drafting || !onDraftingSettings}
          onClick={onDraftingSettings}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="8" cy="8" r="2.2" />
            <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
            <circle cx="8" cy="8" r="4.6" />
          </svg>
        </button>
      </span>
      {/* Coordinates read left to right in both languages. */}
      <span className="cursor" dir="ltr">
        {cursor ? shown(cursor) : ""}
      </span>
    </footer>
  );
}

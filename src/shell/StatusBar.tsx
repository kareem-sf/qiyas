import type { Drafting } from "../api";
import { useT } from "../i18n";
import type { DraftingToggle } from "../tools/shortcuts";

type Props = {
  cursor: [number, number] | null;
  units: string;
  unitsWarning: boolean;
  problems: number;
  /** The problems themselves, one per line; shown on hover over the count. */
  problemsText: string;
  message: string;
  /** Decimals of the cursor position. */
  precision?: number;
  /** The name of the object snap under the cursor, or null. */
  snapLabel?: string | null;
  /** The drawing's drafting settings; null (no drawing) leaves the switches off. */
  drafting?: Drafting | null;
  onToggle?(toggle: DraftingToggle): void;
  /** Opens the drafting settings. */
  onDraftingSettings?(): void;
};

const TOGGLES: { toggle: DraftingToggle; key: string; on: (d: Drafting) => boolean }[] = [
  { toggle: "osnap", key: "F3", on: (d) => d.osnap },
  { toggle: "ortho", key: "F8", on: (d) => d.ortho },
  { toggle: "polar", key: "F10", on: (d) => d.polar },
  { toggle: "grid", key: "F7", on: (d) => d.grid },
];

/** Bottom line: what is happening, the snap under the cursor, the drafting switches, cursor
 *  position in drawing units, units and file warnings. */
export function StatusBar({
  cursor,
  units,
  unitsWarning,
  problems,
  problemsText,
  message,
  precision = 3,
  snapLabel = null,
  drafting = null,
  onToggle,
  onDraftingSettings,
}: Props) {
  const t = useT();
  return (
    <footer className="status">
      <span className="message" role="status" aria-live="polite">
        {message}
      </span>
      {snapLabel && <span className="snap-kind">{snapLabel}</span>}
      {problems > 0 && (
        <span className="warn" title={problemsText}>
          {t("recovered", { n: problems })}
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
        {cursor ? `${cursor[0].toFixed(precision)}, ${cursor[1].toFixed(precision)}` : ""}
      </span>
    </footer>
  );
}

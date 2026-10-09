import { memo } from "react";
import type { LayerInfo } from "../api";
import { useT } from "../i18n";
import type { Theme } from "../viewport/gl";
import { swatchColor } from "./swatch";

type Props = {
  layers: LayerInfo[];
  hidden: Set<string>;
  /** Drives the swatches: white shows as Ink on the light theme, black as Paper on the dark one. */
  theme?: Theme;
  /** A layer change is talking to the core: the eye and isolate buttons wait (and say so). */
  busy?: boolean;
  onToggle(name: string): void;
  onIsolate(name: string): void;
  onShowAll(): void;
  /** The layer new entities are drawn on, and making another one that. Without a handler there
   *  is no button. */
  current?: string;
  onMakeCurrent?(name: string): void;
};

/** The drawing's layers: colour, name, an eye to hide or show, isolate to show only that one, and
 *  make current to draw on it (not an xref's layer, `XREF|LAYER`).
 *  Memoised: the app re-renders on every pointer move (cursor in the status bar). */
export const LayersPanel = memo(function LayersPanel({
  layers,
  hidden,
  theme = "light",
  busy = false,
  onToggle,
  onIsolate,
  onShowAll,
  current,
  onMakeCurrent,
}: Props) {
  const t = useT();
  return (
    <aside className="layers" aria-busy={busy || undefined}>
      <div className="layers-head">
        <h2 className="heading">{t("layers")}</h2>
        <button type="button" className="link" onClick={onShowAll} disabled={hidden.size === 0 || busy}>
          {t("showAll")}
        </button>
      </div>
      <ul>
        {layers.map((l) => {
          // Off and frozen layers stay hidden whatever the user does (the drawing says so).
          const locked = l.off || l.frozen;
          const visible = !locked && !hidden.has(l.name);
          const eyeLabel = `${t("visible")}: ${l.name}`;
          const isolateLabel = `${t("isolate")}: ${l.name}`;
          const currentLabel = `${t("makeCurrent")}: ${l.name}`;
          const isCurrent = current?.toUpperCase() === l.name.toUpperCase();
          return (
            <li key={l.name} className={visible ? undefined : "dim"}>
              <span className="swatch" style={{ background: `rgb(${swatchColor(l.color, theme).join(",")})` }} />
              <span className="name" title={l.name}>
                <bdi>{l.name}</bdi>
              </span>
              <button
                type="button"
                className="icon"
                aria-label={eyeLabel}
                title={eyeLabel}
                aria-pressed={visible}
                disabled={locked}
                aria-disabled={busy || undefined}
                onClick={() => !busy && onToggle(l.name)}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z" />
                  {visible ? <circle cx="8" cy="8" r="2" /> : <path d="M2.5 13.5l11-11" />}
                </svg>
              </button>
              <button
                type="button"
                className="icon"
                aria-label={isolateLabel}
                title={isolateLabel}
                disabled={locked}
                aria-disabled={busy || undefined}
                onClick={() => !busy && onIsolate(l.name)}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <circle cx="8" cy="8" r="5.5" />
                  <circle cx="8" cy="8" r="1.5" />
                </svg>
              </button>
              {onMakeCurrent && (
                <button
                  type="button"
                  className="icon"
                  aria-label={currentLabel}
                  title={currentLabel}
                  aria-pressed={isCurrent}
                  disabled={l.name.includes("|")}
                  aria-disabled={busy || undefined}
                  onClick={() => !busy && !isCurrent && onMakeCurrent(l.name)}
                >
                  <svg viewBox="0 0 16 16" aria-hidden="true">
                    <path d="M3 13l2.5-.5L13 5l-2-2-7.5 7.5z" />
                    {isCurrent && <path d="M2 15h12" />}
                  </svg>
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </aside>
  );
});

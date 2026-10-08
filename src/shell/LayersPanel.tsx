import type { LayerInfo } from "../api";
import { useT } from "../i18n";

type Props = {
  layers: LayerInfo[];
  hidden: Set<string>;
  onToggle(name: string): void;
  onIsolate(name: string): void;
  onShowAll(): void;
};

/** The drawing's layers: colour, name, an eye to hide or show, and isolate to show only that one. */
export function LayersPanel({ layers, hidden, onToggle, onIsolate, onShowAll }: Props) {
  const t = useT();
  return (
    <aside className="layers">
      <div className="layers-head">
        <h2 className="heading">{t("layers")}</h2>
        <button type="button" className="link" onClick={onShowAll} disabled={hidden.size === 0}>
          {t("showAll")}
        </button>
      </div>
      <ul>
        {layers.map((l) => {
          // Off and frozen layers stay hidden whatever the user does (the drawing says so).
          const locked = l.off || l.frozen;
          const visible = !locked && !hidden.has(l.name);
          return (
            <li key={l.name} className={visible ? undefined : "dim"}>
              <span className="swatch" style={{ background: `rgb(${l.color.join(",")})` }} />
              <span className="name" title={l.name}>
                {l.name}
              </span>
              <button
                type="button"
                className="icon"
                aria-label={t("visible")}
                title={t("visible")}
                aria-pressed={visible}
                disabled={locked}
                onClick={() => onToggle(l.name)}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z" />
                  {visible ? <circle cx="8" cy="8" r="2" /> : <path d="M2.5 13.5l11-11" />}
                </svg>
              </button>
              <button
                type="button"
                className="icon"
                aria-label={t("isolate")}
                title={t("isolate")}
                onClick={() => onIsolate(l.name)}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <circle cx="8" cy="8" r="5.5" />
                  <circle cx="8" cy="8" r="1.5" />
                </svg>
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}

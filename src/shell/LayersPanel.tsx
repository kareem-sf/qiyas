import { memo, useState, type KeyboardEvent } from "react";
import type { LayerInfo, LayerProp, ViewportInfo } from "../api";
import { useT } from "../i18n";
import type { Theme } from "../viewport/gl";
import { LINEWEIGHTS, weightLabel } from "./lineweights";
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
  /** The drawing's linetypes, for a layer's linetype. */
  linetypes?: string[];
  /** Edits a layer of the drawing itself (not an xref's): without a handler its properties
   *  don't open. */
  onSetLayer?(name: string, prop: LayerProp, value: boolean | string | number): void;
  onRenameLayer?(from: string, to: string): void;
  onDeleteLayer?(name: string): void;
  onNewLayer?(name: string): void;
  /** Open the layer states and layer translator dialogs. */
  onLayerStates?(): void;
  onTranslate?(): void;
  /** The paper-space viewports of the sheet on screen: with some, a VP freeze column freezes a
   *  layer in the chosen one. */
  viewports?: ViewportInfo[];
  onViewportFrozen?(viewport: number, layer: string, frozen: boolean): void;
};

/** Runs `commit` on Enter. */
const onEnter = (commit: () => void) => (e: KeyboardEvent<HTMLInputElement>) => {
  if (e.key === "Enter") {
    e.preventDefault();
    commit();
  }
};

type LayerPropsProps = {
  layer: LayerInfo;
  linetypes: string[];
  onSet(prop: LayerProp, value: boolean | string | number): void;
  onRename?(to: string): void;
  onDelete?(): void;
};

/** One layer's properties, open under its row: on, frozen, locked, plot, colour, linetype,
 *  lineweight, its name and deleting it. */
function LayerProps({ layer, linetypes, onSet, onRename, onDelete }: LayerPropsProps) {
  const t = useT();
  const [color, setColor] = useState(layer.color_name ?? "");
  const [name, setName] = useState(layer.name);
  const commitColor = () => color.trim() !== "" && color.trim() !== layer.color_name && onSet("color", color.trim());
  const commitName = () => name.trim() !== "" && name.trim() !== layer.name && onRename?.(name.trim());
  const check = (prop: LayerProp, label: string, checked: boolean, value: (on: boolean) => boolean = (on) => on) => (
    <label className="check">
      <input type="checkbox" checked={checked} onChange={(e) => onSet(prop, value(e.currentTarget.checked))} />
      <span>{t(label)}</span>
    </label>
  );
  const types = layer.linetype && !linetypes.includes(layer.linetype) ? [layer.linetype, ...linetypes] : linetypes;
  return (
    <div className="layer-props">
      {check("on", "layer.on", !layer.off)}
      {check("frozen", "layer.frozen", layer.frozen)}
      {check("locked", "layer.locked", layer.locked)}
      {check("plot", "layer.plot", layer.plot ?? true)}
      <label>
        <span>{t("props.color")}</span>
        <input
          dir="ltr"
          value={color}
          onChange={(e) => setColor(e.target.value)}
          onKeyDown={onEnter(commitColor)}
          onBlur={commitColor}
        />
      </label>
      <label>
        <span>{t("props.linetype")}</span>
        <select value={layer.linetype ?? ""} onChange={(e) => onSet("linetype", e.target.value)}>
          {types.map((lt) => (
            <option key={lt} value={lt}>
              {lt}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>{t("props.lineweight")}</span>
        <select value={String(layer.lineweight ?? -3)} onChange={(e) => onSet("lineweight", Number(e.target.value))}>
          {[-3, ...LINEWEIGHTS].map((w) => (
            <option key={w} value={String(w)}>
              {weightLabel(w, t)}
            </option>
          ))}
        </select>
      </label>
      {onRename && (
        <label>
          <span>{t("layer.rename")}</span>
          <input dir="auto" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={onEnter(commitName)} />
        </label>
      )}
      {onDelete && (
        <button type="button" className="link" aria-label={t("layer.delete", { name: layer.name })} onClick={onDelete}>
          {t("layer.deleteShort")}
        </button>
      )}
    </div>
  );
}

/** The drawing's layers: colour, name, an eye to hide or show, isolate to show only that one, and
 *  make current to draw on it (not an xref's layer, `XREF|LAYER`). A layer of the drawing itself
 *  opens its properties under its row (on, frozen, locked, plot, colour, linetype, lineweight,
 *  rename, delete); on a paper sheet with viewports a VP freeze column freezes a layer in the
 *  viewport chosen. The foot adds a layer and opens layer states and the translator.
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
  linetypes = [],
  onSetLayer,
  onRenameLayer,
  onDeleteLayer,
  onNewLayer,
  onLayerStates,
  onTranslate,
  viewports = [],
  onViewportFrozen,
}: Props) {
  const t = useT();
  const [open, setOpen] = useState<string | null>(null);
  const [fresh, setFresh] = useState("");
  const [chosenVp, setChosenVp] = useState<number | null>(null);
  const viewport = viewports.find((v) => v.handle === chosenVp) ?? viewports[0] ?? null;
  const addLayer = () => {
    const name = fresh.trim();
    if (name === "" || !onNewLayer) return;
    onNewLayer(name);
    setFresh("");
  };
  return (
    <aside className="layers" aria-busy={busy || undefined}>
      <div className="layers-head">
        <h2 className="heading">{t("layers")}</h2>
        <button type="button" className="link" onClick={onShowAll} disabled={hidden.size === 0 || busy}>
          {t("showAll")}
        </button>
      </div>
      {viewport && viewports.length > 1 && (
        <label className="layers-vp">
          <span>{t("layer.viewport")}</span>
          <select value={String(viewport.handle)} onChange={(e) => setChosenVp(Number(e.target.value))}>
            {viewports.map((v, i) => (
              <option key={v.handle} value={String(v.handle)}>
                {t("layer.viewportN", { n: i + 1 })}
              </option>
            ))}
          </select>
        </label>
      )}
      <ul>
        {layers.map((l) => {
          // Off and frozen layers stay hidden whatever the user does (the drawing says so).
          const locked = l.off || l.frozen;
          const visible = !locked && !hidden.has(l.name);
          const eyeLabel = `${t("visible")}: ${l.name}`;
          const isolateLabel = `${t("isolate")}: ${l.name}`;
          const currentLabel = `${t("makeCurrent")}: ${l.name}`;
          const propsLabel = `${t("layer.properties")}: ${l.name}`;
          const isCurrent = current?.toUpperCase() === l.name.toUpperCase();
          const own = !l.name.includes("|");
          const expanded = open === l.name;
          const vpFrozen = viewport?.frozen.some((n) => n.toUpperCase() === l.name.toUpperCase()) ?? false;
          return (
            <li key={l.name} className={visible ? undefined : "dim"}>
              <div className="layer-row">
                <span className="swatch" style={{ background: `rgb(${swatchColor(l.color, theme).join(",")})` }} />
                <span className="name" title={l.name}>
                  <bdi>{l.name}</bdi>
                </span>
                {viewport && own && onViewportFrozen && (
                  <input
                    type="checkbox"
                    className="vp-freeze"
                    aria-label={`${t("layer.vpFreeze")}: ${l.name}`}
                    title={`${t("layer.vpFreeze")}: ${l.name}`}
                    checked={vpFrozen}
                    onChange={(e) => onViewportFrozen(viewport.handle, l.name, e.currentTarget.checked)}
                  />
                )}
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
                    disabled={!own}
                    aria-disabled={busy || undefined}
                    onClick={() => !busy && !isCurrent && onMakeCurrent(l.name)}
                  >
                    <svg viewBox="0 0 16 16" aria-hidden="true">
                      <path d="M3 13l2.5-.5L13 5l-2-2-7.5 7.5z" />
                      {isCurrent && <path d="M2 15h12" />}
                    </svg>
                  </button>
                )}
                {onSetLayer && own && (
                  <button
                    type="button"
                    className="icon"
                    aria-label={propsLabel}
                    title={propsLabel}
                    aria-expanded={expanded}
                    onClick={() => setOpen(expanded ? null : l.name)}
                  >
                    <svg viewBox="0 0 16 16" aria-hidden="true">
                      <path d={expanded ? "M4 10l4-4 4 4" : "M4 6l4 4 4-4"} />
                    </svg>
                  </button>
                )}
              </div>
              {expanded && onSetLayer && (
                <LayerProps
                  key={`${l.name}:${l.color_name}`}
                  layer={l}
                  linetypes={linetypes}
                  onSet={(prop, value) => onSetLayer(l.name, prop, value)}
                  onRename={onRenameLayer && ((to) => onRenameLayer(l.name, to))}
                  onDelete={onDeleteLayer && (() => onDeleteLayer(l.name))}
                />
              )}
            </li>
          );
        })}
      </ul>
      {(onNewLayer || onLayerStates || onTranslate) && (
        <div className="layers-foot">
          {onNewLayer && (
            <span className="layer-new">
              <input
                dir="auto"
                aria-label={t("layer.new")}
                placeholder={t("layer.new")}
                value={fresh}
                onChange={(e) => setFresh(e.target.value)}
                onKeyDown={onEnter(addLayer)}
              />
              <button type="button" onClick={addLayer} disabled={fresh.trim() === ""}>
                {t("layer.add")}
              </button>
            </span>
          )}
          {onLayerStates && (
            <button type="button" className="link" onClick={onLayerStates}>
              {t("layer.states")}
            </button>
          )}
          {onTranslate && (
            <button type="button" className="link" onClick={onTranslate}>
              {t("layer.translate")}
            </button>
          )}
        </div>
      )}
    </aside>
  );
});

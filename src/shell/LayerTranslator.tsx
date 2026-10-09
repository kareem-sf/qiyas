import { useState } from "react";
import type { LayerInfo, LayerMapping } from "../api";
import { useT } from "../i18n";
import { Shell, useModal } from "./BlockDialogs";

type Props = {
  /** This drawing's layers (an xref's, `XREF|LAYER`, are left out). */
  layers: LayerInfo[];
  /** Asks for a drawing and reads its layers (read-only); null when none was chosen. */
  loadStandard(): Promise<{ path: string; layers: LayerInfo[] } | null>;
  /** Translates: renames or merges each mapped layer, recolours it, and with `fromPath` gives
   *  it the standard's layer properties. */
  onTranslate(fromPath: string | null, mappings: LayerMapping[]): Promise<void>;
  onClose(): void;
};

/** LAYTRANS: this drawing's layers on the left, each mapped to a layer of the standard (another
 *  drawing, or this drawing's own list until one is loaded) and given a colour if wanted. "Map
 *  same names" maps every layer the standard also has. Mapping onto a layer the drawing has
 *  merges the two. */
export function LayerTranslator({ layers, loadStandard, onTranslate, onClose }: Props) {
  const t = useT();
  const modal = useModal();
  const own = layers.filter((l) => !l.name.includes("|"));
  const [standard, setStandard] = useState<{ path: string; layers: LayerInfo[] } | null>(null);
  const [to, setTo] = useState<Record<string, string>>({});
  const [colors, setColors] = useState<Record<string, string>>({});
  const targets = (standard?.layers ?? own).map((l) => l.name);
  const load = () =>
    modal.act(async () => {
      const found = await loadStandard();
      if (found) {
        setStandard(found);
        setTo({});
      }
    }, false);
  const mapSame = () => {
    const next = { ...to };
    for (const l of own) {
      const same = targets.find((n) => n.toUpperCase() === l.name.toUpperCase());
      if (same) next[l.name] = same;
    }
    setTo(next);
  };
  const mappings: LayerMapping[] = own
    .filter((l) => to[l.name])
    .map((l) => ({ from: l.name, to: to[l.name], color: colors[l.name]?.trim() || null }));
  const submit = () => mappings.length > 0 && modal.act(() => onTranslate(standard?.path ?? null, mappings));
  return (
    <Shell modal={modal} id="layer-translator-title" title={t("laytrans.title")} onClose={onClose} onSubmit={submit}>
      <p className="laytrans-source">
        <span>{t("laytrans.standard")}</span> <bdi dir="ltr">{standard ? standard.path : t("laytrans.thisDrawing")}</bdi>{" "}
        <button type="button" className="link" onClick={load}>
          {t("laytrans.load")}
        </button>
        <button type="button" className="link" onClick={mapSame}>
          {t("laytrans.mapSame")}
        </button>
      </p>
      <table className="laytrans">
        <thead>
          <tr>
            <th>{t("laytrans.from")}</th>
            <th>{t("laytrans.to")}</th>
            <th>{t("props.color")}</th>
          </tr>
        </thead>
        <tbody>
          {own.map((l) => (
            <tr key={l.name}>
              <td>
                <bdi>{l.name}</bdi>
              </td>
              <td>
                <select
                  aria-label={t("laytrans.toNamed", { name: l.name })}
                  value={to[l.name] ?? ""}
                  onChange={(e) => setTo({ ...to, [l.name]: e.target.value })}
                >
                  <option value="">{t("laytrans.leave")}</option>
                  {targets.map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <input
                  dir="ltr"
                  aria-label={t("laytrans.colorNamed", { name: l.name })}
                  placeholder="ACI 1"
                  value={colors[l.name] ?? ""}
                  onChange={(e) => setColors({ ...colors, [l.name]: e.target.value })}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="buttons">
        <button type="button" onClick={modal.close}>
          {t("text.cancel")}
        </button>
        <button type="submit" className="primary" disabled={mappings.length === 0 || modal.busy}>
          {t("laytrans.translate")}
        </button>
      </div>
    </Shell>
  );
}

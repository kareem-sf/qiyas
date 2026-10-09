import { memo, useId, useState, type ReactNode } from "react";
import type { EntityInfo, Units } from "../api";
import { useT } from "../i18n";
import { metresPerUnit } from "./AttachPdfDialog";

type Props = {
  /** The entity picked with the select tool; null when none is. */
  entity: EntityInfo | null;
  units: Units;
  /** Decimals of lengths and areas. */
  precision: number;
  /** The name of the sheet in view. */
  sheet: string;
};

/** The picked entity's properties, read-only for now (editing comes later): handle, type, layer,
 *  colour, linetype, lineweight, size in metres, block and attributes, text, where it sits.
 *  Collapsible; memoised, as the app re-renders on every pointer move. */
export const PropertiesPanel = memo(function PropertiesPanel({
  entity,
  units,
  precision,
  sheet,
}: Props) {
  const t = useT();
  const id = useId();
  const [open, setOpen] = useState(true);
  const m = metresPerUnit(units);
  const metres = (v: number) => `${(v * m).toFixed(precision)} ${t("qty.m")}`;
  const square = (v: number) => `${(v * m * m).toFixed(precision)} ${t("qty.m2")}`;
  /** "ByLayer", "ByBlock", "Default" and "None" in the window's language. */
  const byName = (v: string) =>
    ({ ByLayer: t("props.byLayer"), ByBlock: t("props.byBlock"), None: t("props.noColor") })[v] ?? v;
  const weight = (v: number) =>
    v === -1
      ? t("props.byLayer")
      : v === -2
        ? t("props.byBlock")
        : v < 0
          ? t("props.default")
          : `${(v / 100).toFixed(2)} mm`;

  const rows: [string, ReactNode, boolean?][] = [];
  /** A row; `text` rows read in their own direction, the rest left to right. */
  const row = (key: string, value: ReactNode, text = false) => rows.push([key, value, text]);
  if (entity) {
    row("props.handle", entity.handle.toString(16).toUpperCase());
    row("props.kind", entity.kind);
    row("props.layer", <span title={entity.layer}>{entity.layer_short}</span>, true);
    row(
      "props.color",
      <>
        <span className="swatch" style={{ background: `rgb(${entity.color_rgb.join(",")})` }} />
        {byName(entity.color)}
      </>,
    );
    row("props.linetype", byName(entity.linetype), true);
    row("props.lineweight", weight(entity.lineweight));
    if (entity.length !== null) row("props.length", metres(entity.length));
    if (entity.area !== null) row("props.area", square(entity.area));
    if (entity.radius !== null) row("props.radius", metres(entity.radius));
    if (entity.block_name) row("props.block", entity.block_name, true);
    if (entity.text !== null) row("props.text", entity.text, true);
    if (entity.height !== null) row("props.height", metres(entity.height));
    if (entity.style) row("props.style", entity.style, true);
    if (entity.block_path.length > 0) row("props.path", entity.block_path.join(" › "), true);
    if (entity.xref) row("props.xref", entity.xref, true);
    row("props.sheet", sheet, true);
  }

  return (
    <section className="properties" aria-labelledby={`${id}-title`}>
      <h2 className="heading" id={`${id}-title`}>
        <button
          type="button"
          aria-expanded={open}
          aria-controls={`${id}-body`}
          onClick={() => setOpen((o) => !o)}
        >
          {t("props.title")}
        </button>
      </h2>
      {open && (
        <div id={`${id}-body`}>
          {!entity ? (
            <p className="muted">{t("props.none")}</p>
          ) : (
            <dl>
              {rows.map(([key, value, text]) => (
                <div key={key}>
                  <dt>{t(key)}</dt>
                  <dd dir={text ? "auto" : "ltr"}>{value}</dd>
                </div>
              ))}
              {entity.attributes.length > 0 && (
                <div className="attributes">
                  <dt>{t("props.attributes")}</dt>
                  <dd>
                    <dl>
                      {entity.attributes.map(([tag, value], i) => (
                        <div key={i}>
                          <dt dir="auto">{tag}</dt>
                          <dd dir="auto">{value}</dd>
                        </div>
                      ))}
                    </dl>
                  </dd>
                </div>
              )}
            </dl>
          )}
        </div>
      )}
    </section>
  );
});

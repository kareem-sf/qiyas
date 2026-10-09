import { memo, useId, useState, type KeyboardEvent, type ReactNode } from "react";
import type { Current, EntityInfo, EntityProp, Units } from "../api";
import { useT } from "../i18n";
import { normalizeDigits } from "../tools/commandParser";
import { metresPerUnit } from "./AttachPdfDialog";
import { LINEWEIGHTS, weightLabel } from "./lineweights";

type Props = {
  /** The entity picked with the select tool; null when none is. */
  entity: EntityInfo | null;
  units: Units;
  /** Decimals of lengths and areas. */
  precision: number;
  /** The name of the sheet in view. */
  sheet: string;
  /** The drawing's own layers and linetypes, for the editable fields. */
  layers?: string[];
  linetypes?: string[];
  /** Edits the picked entity: only a drawing's own entity at the top of the sheet (never an
   *  xref's or one inside a block). Without it the palette is read-only. Lengths go in drawing
   *  units, angles in degrees. */
  onSet?(prop: EntityProp, value: boolean | string | number | [number, number]): void;
  /** With nothing picked: what new entities are drawn with, and changing it. */
  current?: Current;
  onSetCurrent?(current: Current): void;
};

/** A number typed in the window's digits (Arabic ones too); null when it is none. */
function typedNumber(text: string): number | null {
  const v = Number(normalizeDigits(text.trim()).replace("٫", "."));
  return text.trim() !== "" && Number.isFinite(v) ? v : null;
}

/** Runs `commit` on Enter. */
const onEnter = (commit: () => void) => (e: KeyboardEvent<HTMLInputElement>) => {
  if (e.key === "Enter") {
    e.preventDefault();
    commit();
  }
};

/** A text field that sends its value on Enter or when it loses focus, if it changed. */
function Field({ label, value, onCommit, dir = "ltr" }: { label: string; value: string; onCommit(v: string): void; dir?: string }) {
  const [text, setText] = useState(value);
  const commit = () => text !== value && onCommit(text);
  return (
    <input
      aria-label={label}
      dir={dir}
      value={text}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={onEnter(commit)}
      onBlur={commit}
    />
  );
}

/** "ByLayer" and "ByBlock" first, then `names` (and `value` when it is none of them). */
function Choice({ label, value, names, onPick }: { label: string; value: string; names: string[]; onPick(v: string): void }) {
  const all = names.includes(value) ? names : [value, ...names];
  return (
    <select aria-label={label} value={value} onChange={(e) => onPick(e.target.value)}>
      {all.map((n) => (
        <option key={n} value={n}>
          {n}
        </option>
      ))}
    </select>
  );
}

/** The picked entity's properties: handle, type, layer, colour, linetype, lineweight, size in
 *  metres, block and attributes, text, where it sits. A drawing's own entity at the top of the
 *  sheet is edited in place (`onSet`): its layer, colour, linetype, lineweight, and as it has them
 *  its text, text height and rotation, radius, closed flag, global width, scale and rotation.
 *  With nothing picked it edits what new entities are drawn with (`onSetCurrent`).
 *  Collapsible; memoised, as the app re-renders on every pointer move. */
export const PropertiesPanel = memo(function PropertiesPanel({
  entity,
  units,
  precision,
  sheet,
  layers = [],
  linetypes = [],
  onSet,
  current,
  onSetCurrent,
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
  const weight = (v: number) => weightLabel(v, t);
  const editable = !!entity && !!onSet && entity.chain.length === 0 && !entity.xref;
  // Fields start again from the core's value when the entity or its values change.
  const key = entity ? JSON.stringify(entity) : current ? JSON.stringify(current) : "";

  const rows: [string, ReactNode, boolean?][] = [];
  /** A row; `text` rows read in their own direction, the rest left to right. */
  const row = (label: string, value: ReactNode, text = false) => rows.push([label, value, text]);
  const types = ["ByLayer", "ByBlock", ...linetypes];
  const weights = [-1, -2, -3, ...LINEWEIGHTS];
  const weightSelect = (value: number, onPick: (w: number) => void) => (
    <select aria-label={t("props.lineweight")} value={String(value)} onChange={(e) => onPick(Number(e.target.value))}>
      {weights.map((w) => (
        <option key={w} value={String(w)}>
          {weight(w)}
        </option>
      ))}
    </select>
  );
  /** A length field in metres, sent in drawing units. */
  const lengthField = (label: string, v: number, prop: EntityProp) => (
    <Field
      label={t(label)}
      value={(v * m).toFixed(precision)}
      onCommit={(s) => {
        const n = typedNumber(s);
        if (n !== null) onSet?.(prop, n / m);
      }}
    />
  );
  const numberField = (label: string, v: number, prop: EntityProp) => (
    <Field
      label={t(label)}
      value={String(Number(v.toFixed(6)))}
      onCommit={(s) => {
        const n = typedNumber(s);
        if (n !== null) onSet?.(prop, n);
      }}
    />
  );

  if (entity && editable && onSet) {
    const textual = entity.kind === "Text" || entity.kind === "MText";
    row("props.handle", entity.handle.toString(16).toUpperCase());
    row("props.kind", entity.kind);
    row("props.layer", <Choice label={t("props.layer")} value={entity.layer} names={layers} onPick={(v) => onSet("layer", v)} />);
    row(
      "props.color",
      <>
        <span className="swatch" style={{ background: `rgb(${entity.color_rgb.join(",")})` }} />
        <Field label={t("props.color")} value={entity.color} onCommit={(v) => v.trim() && onSet("color", v.trim())} />
      </>,
    );
    row("props.linetype", <Choice label={t("props.linetype")} value={entity.linetype} names={types} onPick={(v) => onSet("linetype", v)} />);
    row("props.lineweight", weightSelect(entity.lineweight, (w) => onSet("lineweight", w)));
    if (entity.length !== null) row("props.length", metres(entity.length));
    if (entity.area !== null) row("props.area", square(entity.area));
    if (entity.radius !== null) row("props.radiusM", lengthField("props.radiusM", entity.radius, "radius"));
    if (entity.closed !== null && entity.closed !== undefined) {
      row(
        "props.closed",
        <input
          type="checkbox"
          aria-label={t("props.closed")}
          checked={entity.closed}
          onChange={(e) => onSet("closed", e.currentTarget.checked)}
        />,
      );
    }
    if (entity.global_width !== null && entity.global_width !== undefined) {
      row("props.widthM", lengthField("props.widthM", entity.global_width, "global_width"));
    }
    if (entity.block_name) row("props.block", entity.block_name, true);
    if (entity.kind === "Insert" && entity.scale !== null && entity.scale !== undefined) {
      row("props.scale", numberField("props.scale", entity.scale, "insert_scale"));
    }
    if (entity.viewport_scale !== null && entity.viewport_scale !== undefined) {
      row("props.viewportScale", numberField("props.viewportScale", entity.viewport_scale, "viewport_scale"));
    }
    if (entity.viewport_centre) {
      const [x, y] = entity.viewport_centre;
      row(
        "props.viewportCentre",
        <Field
          label={t("props.viewportCentre")}
          value={`${Number(x.toFixed(6))}, ${Number(y.toFixed(6))}`}
          onCommit={(s) => {
            const parts = normalizeDigits(s).split(",").map(typedNumber);
            if (parts.length === 2 && parts[0] !== null && parts[1] !== null) {
              onSet("viewport_centre", [parts[0], parts[1]]);
            }
          }}
        />,
      );
    }
    if (textual && entity.text !== null) {
      row("props.text", <Field label={t("props.text")} dir="auto" value={entity.plain_text ?? entity.text} onCommit={(v) => onSet("text", v)} />, true);
    } else if (entity.text !== null) {
      row("props.text", entity.text, true);
    }
    if (entity.height !== null) row("props.heightM", lengthField("props.heightM", entity.height, "text_height"));
    if (entity.rotation !== null && entity.rotation !== undefined) {
      row(
        "props.rotationDeg",
        numberField("props.rotationDeg", entity.rotation, entity.kind === "Insert" ? "insert_rotation" : "text_rotation"),
      );
    }
    if (entity.style) row("props.style", entity.style, true);
    row("props.sheet", sheet, true);
  } else if (entity) {
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
  } else if (current && onSetCurrent) {
    const set = (change: Partial<Current>) => onSetCurrent({ ...current, ...change });
    row("props.layer", <Choice label={t("props.layer")} value={current.layer} names={layers} onPick={(layer) => set({ layer })} />);
    row("props.color", <Field label={t("props.color")} value={current.color} onCommit={(v) => v.trim() && set({ color: v.trim() })} />);
    row("props.linetype", <Choice label={t("props.linetype")} value={current.linetype} names={types} onPick={(linetype) => set({ linetype })} />);
    row("props.lineweight", weightSelect(current.lineweight, (lineweight) => set({ lineweight })));
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
          {!entity && rows.length > 0 && <p className="muted">{t("props.current")}</p>}
          {!entity && rows.length === 0 ? (
            <p className="muted">{t("props.none")}</p>
          ) : (
            <dl key={key}>
              {rows.map(([label, value, text]) => (
                <div key={label}>
                  <dt>{t(label)}</dt>
                  <dd dir={text ? "auto" : "ltr"}>{value}</dd>
                </div>
              ))}
              {entity && entity.attributes.length > 0 && (
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

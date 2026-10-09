import { useState, type KeyboardEvent } from "react";
import { useT } from "../i18n";
import type { Tool } from "../viewport/Viewport";

export type { Tool };

/** A missing handler shows its button disabled as "coming soon". */
type Props = {
  active: Tool;
  onTool(t: Tool): void;
  onOpen(): void;
  onSave?(): void;
  onSaveAs?(): void;
  onExport?(): void;
  onImportBoq?(): void;
  onCompare?(): void;
  onExportConflicts?(): void;
  onZoomExtents(): void;
  onUndo?(): void;
  onRedo?(): void;
  /** The drawing holds its Arabic text backwards; the checkbox is disabled without a handler. */
  arabicReversed?: boolean;
  onArabicReversed?(on: boolean): void;
};

type Tab = "home" | "measure" | "view";
const TABS: Tab[] = ["home", "measure", "view"];

// Space is the viewport's pan key: it must never press a focused ribbon button.
const ignoreSpace = (e: KeyboardEvent) => {
  if (e.key === " ") e.preventDefault();
};

export function Ribbon(props: Props) {
  const t = useT();
  const [tab, setTab] = useState<Tab>("home");

  const action = (label: string, onClick?: () => void) => (
    <button
      key={label}
      type="button"
      disabled={!onClick}
      title={onClick ? undefined : t("comingSoon")}
      onClick={onClick}
      onKeyDown={ignoreSpace}
      onKeyUp={ignoreSpace}
    >
      {t(label)}
    </button>
  );

  const tool = (name: Tool) => (
    <button
      key={name}
      type="button"
      aria-pressed={props.active === name}
      onClick={() => props.onTool(props.active === name && name !== "select" ? "select" : name)}
      onKeyDown={ignoreSpace}
      onKeyUp={ignoreSpace}
    >
      {t(name)}
    </button>
  );

  return (
    <div className="ribbon">
      <div className="ribbon-tabs" role="tablist">
        <span className="app-name heading">{t("appName")}</span>
        {TABS.map((name) => (
          <button
            key={name}
            type="button"
            role="tab"
            aria-selected={tab === name}
            onClick={() => setTab(name)}
            onKeyDown={ignoreSpace}
            onKeyUp={ignoreSpace}
          >
            {t(name)}
          </button>
        ))}
      </div>
      <div className="ribbon-panel" role="tabpanel">
        {tab === "home" && [
          action("open", props.onOpen),
          action("save", props.onSave),
          action("saveAs", props.onSaveAs),
          action("exportTakeoff", props.onExport),
          action("importBoq", props.onImportBoq),
          action("compare", props.onCompare),
          action("exportConflicts", props.onExportConflicts),
          <span key="sep" className="sep" />,
          action("undo", props.onUndo),
          action("redo", props.onRedo),
        ]}
        {tab === "measure" && (["select", "distance", "area", "count"] as Tool[]).map(tool)}
        {tab === "view" && [
          action("zoomExtents", props.onZoomExtents),
          tool("zoomWindow"),
          <span key="sep" className="sep" />,
          <label key="arabicReversed" className="ribbon-check">
            <input
              type="checkbox"
              checked={props.arabicReversed ?? false}
              disabled={!props.onArabicReversed}
              onChange={(e) => props.onArabicReversed?.(e.currentTarget.checked)}
              onKeyDown={ignoreSpace}
              onKeyUp={ignoreSpace}
            />
            {t("arabicReversed")}
          </label>,
        ]}
      </div>
    </div>
  );
}

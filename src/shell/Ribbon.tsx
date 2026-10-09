import { useState, type KeyboardEvent } from "react";
import type { NamedView, Recent } from "../api";
import { useT } from "../i18n";
import type { Tool } from "../viewport/Viewport";
import { RecentsMenu } from "./RecentsMenu";
import { useRoving } from "./useRoving";
import { ViewsMenu } from "./ViewsMenu";

export type { Tool };

/** A missing handler shows its button disabled as "coming soon". */
type Props = {
  active: Tool;
  onTool(t: Tool): void;
  onOpen(): void;
  /** The recent drawings beside Open (see `RecentsMenu`); without them Open is a plain button. */
  loadRecents?(): Promise<Recent[]>;
  onOpenRecent?(path: string): void;
  onClearRecents?(): Promise<void>;
  onSave?(): void;
  onSaveAs?(): void;
  onExport?(): void;
  onImportBoq?(): void;
  onCompare?(): void;
  onExportConflicts?(): void;
  onZoomExtents(): void;
  /** Opens the external references (xrefs) dialog. */
  onXrefs?(): void;
  /** Opens the Attach PDF dialog. */
  onAttachPdf?(): void;
  /** Opens the Attach image dialog. */
  onAttachImage?(): void;
  /** Starts calibrating a PDF underlay or image: two points, then their real distance. */
  onCalibrate?(): void;
  /** Opens the Extract data dialog. */
  onExtract?(): void;
  onUndo?(): void;
  onRedo?(): void;
  /** The drawing holds its Arabic text backwards; the checkbox is disabled without a handler. */
  arabicReversed?: boolean;
  onArabicReversed?(on: boolean): void;
  /** The View tab's named views (see `ViewsMenu`); without them Views is disabled. */
  loadViews?(): Promise<NamedView[]>;
  onGoToView?(view: NamedView): void;
  onSaveView?(name: string): Promise<void>;
  onDeleteView?(name: string): Promise<void>;
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
  const roving = useRoving(TABS.length, TABS.indexOf(tab));

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
      <div className="ribbon-tabs" role="tablist" {...roving.props}>
        <span className="app-name heading">{t("appName")}</span>
        {TABS.map((name, i) => (
          <button
            key={name}
            type="button"
            role="tab"
            aria-selected={tab === name}
            tabIndex={roving.tabIndex(i)}
            onClick={() => setTab(name)}
          >
            {t(name)}
          </button>
        ))}
      </div>
      <div className="ribbon-panel" role="tabpanel">
        {tab === "home" && [
          props.loadRecents && props.onOpenRecent && props.onClearRecents ? (
            <RecentsMenu
              key="open"
              onOpen={props.onOpen}
              load={props.loadRecents}
              onOpenRecent={props.onOpenRecent}
              onClear={props.onClearRecents}
              onKey={ignoreSpace}
            />
          ) : (
            action("open", props.onOpen)
          ),
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
        {tab === "measure" && [
          ...(["select", "distance", "area", "count"] as Tool[]).map(tool),
          <span key="sep" className="sep" />,
          ...(["geometry", "divide", "measureAlong", "idPoint"] as Tool[]).map(tool),
          action("extract", props.onExtract),
        ]}
        {tab === "view" && [
          action("zoomExtents", props.onZoomExtents),
          tool("zoomWindow"),
          props.loadViews && props.onGoToView && props.onSaveView && props.onDeleteView ? (
            <ViewsMenu
              key="views"
              load={props.loadViews}
              onGo={props.onGoToView}
              onSave={props.onSaveView}
              onDelete={props.onDeleteView}
              onKey={ignoreSpace}
            />
          ) : (
            action("views")
          ),
          action("xrefs", props.onXrefs),
          action("attachPdf", props.onAttachPdf),
          action("attachImage", props.onAttachImage),
          action("calibrate", props.onCalibrate),
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

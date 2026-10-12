import { useState, type KeyboardEvent } from "react";
import type { NamedView, Recent, RibbonButton, RibbonLayout } from "../api";
import { useT } from "../i18n";
import type { Tool } from "../viewport/Viewport";
import { RecentsMenu } from "./RecentsMenu";
import { DEFAULT_RIBBON, QUICK_ACCESS } from "./ribbonLayout";
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
  /** The Output tab: the Plot dialog (PLOT), quick print of the view (QUICKPRINT), every layout
   *  printed (PUBLISH) and the plot style editor (STYLESMANAGER). */
  onPlot?(): void;
  onQuickPrint?(): void;
  onPrintAll?(): void;
  onPlotStyles?(): void;
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
  /** Opens the Find and replace dialog. */
  onFind?(): void;
  /** Opens the dimension style panel. */
  onDimStyle?(): void;
  /** Works every field out again (UPDATEFIELD). */
  onUpdateFields?(): void;
  /** The Parametric tab: the parameters manager (PARAMETERS) and showing the constraint bar. */
  onParameters?(): void;
  onConstraintBar?(): void;
  /** The Insert tab: the block palette to insert from (INSERT) or write a block (WBLOCK), the
   *  attribute manager (BATTMAN), the Design Center and the tool palette. */
  onInsertBlock?(): void;
  onWriteBlock?(): void;
  onAttributeManager?(): void;
  onDesignCenter?(): void;
  onToolPalettes?(): void;
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
  /** The Home tab's Layers group: layer states (LAYSTATE) and the layer translator (LAYTRANS). */
  onLayerStates?(): void;
  onTranslateLayers?(): void;
  /** Clicking a grouped entity selects its group (PICKSTYLE); the checkbox is disabled without a
   *  handler. */
  groupSelect?: boolean;
  onGroupSelect?(on: boolean): void;
  /** VPORTS: two views side by side (`split`) or one. */
  split?: boolean;
  onSplit?(two: boolean): void;
  /** UCS W: back to the world coordinate system. */
  onUcsWorld?(): void;
  /** Opens the sheet set manager (SHEETSET). */
  onSheetSet?(): void;
  /** RECOVER: opens a damaged drawing and says what was dropped. */
  onRecover?(): void;
  /** The Manage tab: SCRIPT asks for a `.scr` command script and runs it. */
  onRunScript?(): void;
  /** The Manage tab's AUDIT and PURGE dialogs, and OPTIONS, CUI (Customize) and ALIASEDIT. */
  onAudit?(): void;
  onPurge?(): void;
  onOptions?(): void;
  onCustomize?(): void;
  onAliases?(): void;
  /** The tabs, groups and buttons to draw (`ribbonLayout`); the default when null. */
  layout?: RibbonLayout | null;
};

// Space is the viewport's pan key: it must never press a focused ribbon button.
const ignoreSpace = (e: KeyboardEvent) => {
  if (e.key === " ") e.preventDefault();
};

/** The handler each action button runs, by its command; a missing one shows it disabled. */
function actions(props: Props): Record<string, (() => void) | undefined> {
  return {
    open: props.onOpen,
    save: props.onSave,
    saveAs: props.onSaveAs,
    recover: props.onRecover,
    audit: props.onAudit,
    purge: props.onPurge,
    exportTakeoff: props.onExport,
    importBoq: props.onImportBoq,
    compare: props.onCompare,
    exportConflicts: props.onExportConflicts,
    undo: props.onUndo,
    redo: props.onRedo,
    find: props.onFind,
    layerStates: props.onLayerStates,
    translateLayers: props.onTranslateLayers,
    extract: props.onExtract,
    dimStyle: props.onDimStyle,
    updateFields: props.onUpdateFields,
    parameters: props.onParameters,
    constraintBar: props.onConstraintBar,
    blockPalette: props.onInsertBlock,
    writeBlock: props.onWriteBlock,
    attributeManager: props.onAttributeManager,
    designCenter: props.onDesignCenter,
    toolPalettes: props.onToolPalettes,
    zoomExtents: props.onZoomExtents,
    xrefs: props.onXrefs,
    attachPdf: props.onAttachPdf,
    attachImage: props.onAttachImage,
    calibrate: props.onCalibrate,
    split: props.onSplit && (() => props.onSplit!(!props.split)),
    ucsWorld: props.onUcsWorld,
    sheetSet: props.onSheetSet,
    runScript: props.onRunScript,
    plot: props.onPlot,
    quickPrint: props.onQuickPrint,
    printAll: props.onPrintAll,
    plotStyles: props.onPlotStyles,
    options: props.onOptions,
    customize: props.onCustomize,
    aliases: props.onAliases,
  };
}

export function Ribbon(props: Props) {
  const t = useT();
  const layout = props.layout ?? DEFAULT_RIBBON;
  const tabs = layout.tabs;
  const [chosen, setTab] = useState<string>(tabs[0]?.id ?? "");
  // A layout without the tab chosen (saved meanwhile) shows its first.
  const tab = tabs.find((x) => x.id === chosen) ?? tabs[0];
  const roving = useRoving(tabs.length, tab ? tabs.indexOf(tab) : 0);
  const handlers = actions(props);

  const action = (key: string, label: string, onClick?: () => void) => (
    <button
      key={key}
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

  const tool = (key: string, name: Tool, label: string) => (
    <button
      key={key}
      type="button"
      aria-pressed={props.active === name}
      onClick={() => props.onTool(props.active === name && name !== "select" ? "select" : name)}
      onKeyDown={ignoreSpace}
      onKeyUp={ignoreSpace}
    >
      {t(label)}
    </button>
  );

  const check = (key: string, label: string, checked: boolean, onChange?: (on: boolean) => void) => (
    <label key={key} className="ribbon-check">
      <input
        type="checkbox"
        checked={checked}
        disabled={!onChange}
        onChange={(e) => onChange?.(e.currentTarget.checked)}
        onKeyDown={ignoreSpace}
        onKeyUp={ignoreSpace}
      />
      {t(label)}
    </label>
  );

  /** One button of the layout, as its kind and command say. */
  function draw(b: RibbonButton, key: string) {
    if (b.kind === "tool") return tool(key, b.command as Tool, b.label);
    if (b.kind === "toggle") {
      if (b.command === "groupSelect") return check(key, b.label, props.groupSelect ?? true, props.onGroupSelect);
      return check(key, b.label, props.arabicReversed ?? false, props.onArabicReversed);
    }
    if (b.command === "open" && props.loadRecents && props.onOpenRecent && props.onClearRecents) {
      return (
        <RecentsMenu
          key={key}
          onOpen={props.onOpen}
          load={props.loadRecents}
          onOpenRecent={props.onOpenRecent}
          onClear={props.onClearRecents}
          onKey={ignoreSpace}
        />
      );
    }
    if (b.command === "views" && props.loadViews && props.onGoToView && props.onSaveView && props.onDeleteView) {
      return (
        <ViewsMenu
          key={key}
          load={props.loadViews}
          onGo={props.onGoToView}
          onSave={props.onSaveView}
          onDelete={props.onDeleteView}
          onKey={ignoreSpace}
        />
      );
    }
    // Two viewports, or back to one: the default label says which.
    const label = b.command === "split" && b.label === "vport.two" && props.split ? "vport.one" : b.label;
    return action(key, label, handlers[b.command]);
  }

  // AutoCAD's ribbon: each group a box of up to three rows of buttons, filled column by column,
  // with its name under them; the panel scrolls sideways rather than wrap (`theme.css`).
  return (
    <div className="ribbon">
      <div className="ribbon-tabs">
        <span className="app-name heading">{t("appName")}</span>
        <div className="quick-access" role="toolbar" aria-label={t("ribbon.quickAccess")}>
          {QUICK_ACCESS.map((b) => draw(b, b.command))}
        </div>
        <div className="ribbon-tablist" role="tablist" {...roving.props}>
          {tabs.map((x, i) => (
            <button
              key={x.id}
              type="button"
              role="tab"
              aria-selected={x === tab}
              tabIndex={roving.tabIndex(i)}
              onClick={() => setTab(x.id)}
            >
              {t(x.label)}
            </button>
          ))}
        </div>
      </div>
      <div className="ribbon-panel" role="tabpanel">
        {tab?.groups.map((g) => (
          <div key={g.id} className="ribbon-group" role="group" aria-label={t(g.label)}>
            <div className="ribbon-group-buttons">{g.buttons.map((b, i) => draw(b, `${i}`))}</div>
            <div className="ribbon-group-label" aria-hidden="true">
              {t(g.label)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

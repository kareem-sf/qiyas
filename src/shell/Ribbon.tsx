import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import type { NamedView, Recent, RibbonButton, RibbonGroup, RibbonLayout } from "../api";
import { useT } from "../i18n";
import type { Tool } from "../viewport/Viewport";
import { RecentsMenu } from "./RecentsMenu";
import { RibbonFlyout } from "./RibbonFlyout";
import { RibbonIcon } from "./ribbonIcons";
import { aliasesFor, isPrimaryGroup } from "./ribbonInfo";
import { DEFAULT_RIBBON, QUICK_ACCESS } from "./ribbonLayout";
import { Tip, TooltipLayer, type TipInfo } from "./Tooltip";
import { useRoving, useToolbarRoving } from "./useRoving";
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
  /** The command line's aliases (the built-in ones and the user's) for the tooltips; the built-in
   *  ones when left out. */
  aliases?: Record<string, string>;
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


/** Tabs the tabs' row shows; the others are under "More" (all of them when "More" would hold one). */
const SHOWN_TABS = 3;

/** The one flyout open: a group's "⋯" list, or the tabs' "More". */
type Open = { kind: "more" | "group"; key: string; anchor: HTMLElement };

export function Ribbon(props: Props) {
  return (
    <TooltipLayer>
      <RibbonBody {...props} />
    </TooltipLayer>
  );
}

function RibbonBody(props: Props) {
  const t = useT();
  const layout = props.layout ?? DEFAULT_RIBBON;
  const tabs = layout.tabs;
  const [chosen, setTab] = useState<string>(tabs[0]?.id ?? "");
  // A layout without the tab chosen (saved meanwhile) shows its first.
  const tab = tabs.find((x) => x.id === chosen) ?? tabs[0];
  const lead = tabs.length > SHOWN_TABS + 1 ? tabs.slice(0, SHOWN_TABS) : tabs;
  const more = tabs.slice(lead.length);
  const inMore = tab !== undefined && more.includes(tab);
  const roving = useRoving(lead.length + (more.length > 0 ? 1 : 0), !tab ? 0 : inMore ? lead.length : lead.indexOf(tab));
  const quick = useToolbarRoving();
  const handlers = actions(props);
  const [open, setOpen] = useState<Open | null>(null);
  const moreTab = useRef<HTMLButtonElement>(null);

  // How many buttons each group of the tab on screen shows in this width (`main`, or fewer when
  // the panel would need to scroll), by tab and group.
  const [fit, setFit] = useState<Record<string, number>>({});
  const [width, setWidth] = useState(0);
  const panel = useRef<HTMLDivElement>(null);

  /** What a button is and does now; no `run` is a disabled button ("coming soon"). */
  function resolve(b: RibbonButton): { label: string; run?: () => void; pressed?: boolean } {
    if (b.kind === "tool") {
      const name = b.command as Tool;
      return {
        label: b.label,
        pressed: props.active === name,
        run: () => props.onTool(props.active === name && name !== "select" ? "select" : name),
      };
    }
    if (b.kind === "toggle") {
      const groupSelect = b.command === "groupSelect";
      const on = groupSelect ? (props.groupSelect ?? true) : (props.arabicReversed ?? false);
      const set = groupSelect ? props.onGroupSelect : props.onArabicReversed;
      return { label: b.label, pressed: on, run: set && (() => set(!on)) };
    }
    // Two viewports, or back to one: the default label says which.
    const label = b.command === "split" && b.label === "vport.two" && props.split ? "vport.one" : b.label;
    return { label, run: handlers[b.command] };
  }

  /** The tooltip of a command: its name, what it does, its alias; "coming soon" when disabled. */
  function tipFor(command: string, name: string, disabled: boolean): TipInfo {
    const key = `tip.${command}`;
    const text = t(key);
    const aliases = aliasesFor(command, props.aliases);
    return {
      name: name.replace(/…$/, ""),
      text: text === key ? undefined : text,
      alias: aliases.length > 0 ? `${t("ribbon.alias")} · ${aliases.join(" · ")}` : undefined,
      note: disabled ? t("comingSoon") : undefined,
    };
  }

  /** An icon button: its name is its `aria-label`, its tooltip says the rest. */
  function iconButton(b: RibbonButton, key: string, primary: boolean) {
    const r = resolve(b);
    const name = t(r.label);
    return (
      <Tip key={key} info={tipFor(b.command, name, !r.run)}>
        {(describedBy) => (
          <button
            type="button"
            className={primary ? "rb primary" : "rb"}
            aria-label={name}
            aria-pressed={b.kind === "action" ? undefined : r.pressed}
            aria-describedby={describedBy}
            disabled={!r.run}
            onClick={r.run}
            onKeyDown={ignoreSpace}
            onKeyUp={ignoreSpace}
          >
            <RibbonIcon name={b.command} />
          </button>
        )}
      </Tip>
    );
  }

  /** One button of the layout, as its kind and command say. */
  function draw(b: RibbonButton, key: string, primary = false) {
    if (b.command === "open" && props.loadRecents && props.onOpenRecent && props.onClearRecents) {
      return (
        <RecentsMenu
          key={key}
          onOpen={props.onOpen}
          load={props.loadRecents}
          onOpenRecent={props.onOpenRecent}
          onClear={props.onClearRecents}
          onKey={ignoreSpace}
          tip={tipFor("open", t("open"), false)}
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
          tip={tipFor("views", t("views"), false)}
        />
      );
    }
    return iconButton(b, key, primary);
  }

  /** A group's buttons: those on show, and those behind its "⋯" button. Views, a menu of its own,
   *  always stays on show. */
  function parts(tabId: string, g: RibbonGroup) {
    const key = `${tabId}/${g.id}`;
    const n = g.buttons.length;
    const cap = Math.min(g.main ?? n, n);
    const shown = Math.min(cap, fit[key] ?? cap);
    const on = (b: RibbonButton, i: number) => i < shown || b.command === "views";
    return {
      key,
      n,
      shown,
      visible: g.buttons.filter(on),
      rest: g.buttons.filter((b, i) => !on(b, i)),
    };
  }

  // The panel never needs a scrollbar if it can help it: too wide, the group with the most
  // buttons on show puts more of them behind its "⋯" button, until it fits (or nothing is left to
  // put away). A new width, tab or layout starts again from what the groups want to show.
  const seen = useRef({ at: "", layout });
  useLayoutEffect(() => {
    const el = panel.current;
    if (!el || !tab) return;
    const at = `${tab.id}|${width}`;
    const changed = seen.current.at !== at || seen.current.layout !== layout;
    seen.current = { at, layout };
    if (changed && Object.keys(fit).length > 0) {
      setFit({});
      return;
    }
    if (el.scrollWidth <= el.clientWidth) return;
    let pick: { key: string; shown: number; n: number } | null = null;
    for (const g of tab.groups) {
      const p = parts(tab.id, g);
      if (p.shown > 0 && !(p.shown === p.n && p.n < 2) && (!pick || p.shown >= pick.shown)) pick = p;
    }
    // The first button put away is not worth it (the "⋯" takes its place): put two.
    if (pick) {
      const { key, shown, n } = pick;
      setFit((f) => ({ ...f, [key]: shown === n ? Math.max(0, n - 2) : shown - 1 }));
    }
  });
  useEffect(() => {
    const el = panel.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const watch = new ResizeObserver(() => setWidth(el.clientWidth));
    watch.observe(el);
    return () => watch.disconnect();
  }, []);
  // Another tab, or the buttons rearranged under it: its flyout is closed.
  useEffect(() => {
    if (open && !open.anchor.isConnected) setOpen(null);
  });

  const close = (restoreFocus: boolean) => {
    if (restoreFocus) open?.anchor.focus();
    setOpen(null);
  };

  /** One row of a flyout: the icon and the name. */
  function flyoutItem(b: RibbonButton, key: string) {
    const r = resolve(b);
    const checkable = b.kind !== "action";
    return (
      <button
        key={key}
        type="button"
        role={checkable ? "menuitemcheckbox" : "menuitem"}
        aria-checked={checkable ? r.pressed : undefined}
        className={checkable && r.pressed ? "rb-item on" : "rb-item"}
        disabled={!r.run}
        title={r.run ? undefined : t("comingSoon")}
        onClick={() => {
          setOpen(null);
          r.run?.();
        }}
        onKeyDown={ignoreSpace}
        onKeyUp={ignoreSpace}
      >
        <RibbonIcon name={b.command} />
        <span>{t(r.label)}</span>
      </button>
    );
  }

  const flyout = () => {
    if (!open) return null;
    if (open.kind === "more") {
      return (
        <RibbonFlyout anchor={open.anchor} label={t("ribbon.more")} rows={more.length} onClose={close}>
          {more.map((x) => (
            <button
              key={x.id}
              type="button"
              role="menuitemradio"
              aria-checked={x === tab}
              className={x === tab ? "rb-item on" : "rb-item"}
              onClick={() => {
                setTab(x.id);
                setOpen(null);
                moreTab.current?.focus();
              }}
              onKeyDown={ignoreSpace}
              onKeyUp={ignoreSpace}
            >
              <span>{t(x.label)}</span>
            </button>
          ))}
        </RibbonFlyout>
      );
    }
    const g = tab?.groups.find((x) => `${tab.id}/${x.id}` === open.key);
    if (!g) return null;
    const { rest } = parts(tab!.id, g);
    const columns = rest.length > 8 ? 3 : rest.length > 4 ? 2 : 1;
    return (
      <RibbonFlyout anchor={open.anchor} label={t(g.label)} rows={Math.ceil(rest.length / columns)} onClose={close}>
        {rest.map((b, i) => flyoutItem(b, `${i}`))}
      </RibbonFlyout>
    );
  };

  const toggleFlyout = (kind: Open["kind"], key: string, anchor: HTMLElement) =>
    setOpen(open && open.kind === kind && open.key === key ? null : { kind, key, anchor });

  const groupBox = (g: RibbonGroup) => {
    const name = t(g.label);
    const p = parts(tab!.id, g);
    const label = t("ribbon.groupMore", { name });
    const primary = isPrimaryGroup(g);
    return (
      <div key={g.id} className="ribbon-group" role="group" aria-label={name}>
        <div className={primary ? "ribbon-group-buttons primary" : "ribbon-group-buttons"}>
          {p.visible.map((b, i) => draw(b, `${i}`, primary))}
          {p.rest.length > 0 && (
            <Tip info={{ name: label }}>
              {(describedBy) => (
                <button
                  type="button"
                  className={primary ? "rb primary" : "rb"}
                  aria-label={label}
                  aria-haspopup="menu"
                  aria-expanded={open?.kind === "group" && open.key === p.key}
                  aria-describedby={describedBy}
                  onClick={(e) => toggleFlyout("group", p.key, e.currentTarget)}
                  onKeyDown={(e) => {
                    ignoreSpace(e);
                    if (e.key === "ArrowDown") toggleFlyout("group", p.key, e.currentTarget);
                  }}
                  onKeyUp={ignoreSpace}
                >
                  <RibbonIcon name="more" />
                </button>
              )}
            </Tip>
          )}
        </div>
        <div className="ribbon-group-label" aria-hidden="true">
          {name}
        </div>
      </div>
    );
  };

  // Measure first: the tabs' row (the app name, the quick-access bar, three tabs and "More"),
  // then the panel of the tab chosen, each group a box of icon buttons with its name under them.
  // The panel does not wrap: a group puts buttons behind its "⋯" as the window narrows, and only
  // when even that is not enough does it scroll sideways (`theme.css`).
  return (
    <div className="ribbon">
      <div className="ribbon-tabs">
        <span className="app-name heading">{t("appName")}</span>
        <div className="quick-access" role="toolbar" aria-label={t("ribbon.quickAccess")} {...quick.props}>
          {QUICK_ACCESS.map((b) => draw(b, b.command))}
        </div>
        <div className="ribbon-tablist" role="tablist" {...roving.props}>
          {lead.map((x, i) => (
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
          {more.length > 0 && (
            <button
              ref={moreTab}
              type="button"
              role="tab"
              className="ribbon-more"
              aria-selected={inMore}
              aria-haspopup="menu"
              aria-expanded={open?.kind === "more"}
              tabIndex={roving.tabIndex(lead.length)}
              onClick={(e) => toggleFlyout("more", "more", e.currentTarget)}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setOpen({ kind: "more", key: "more", anchor: e.currentTarget });
                }
              }}
            >
              {inMore ? t(tab.label) : t("ribbon.more")}
              <RibbonIcon name="chevron" />
            </button>
          )}
        </div>
      </div>
      <div className="ribbon-panel" role="tabpanel" ref={panel}>
        {tab?.groups.map(groupBox)}
      </div>
      {flyout()}
    </div>
  );
}

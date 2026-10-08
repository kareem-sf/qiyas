import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  aiUndo,
  drawingInfo,
  entityInfo,
  exportTakeoff,
  openDrawing,
  pick,
  redo,
  renderBatch,
  runCommand,
  saveDrawing,
  saveDrawingAs,
  saveProject,
  setLayerHidden,
  setSheet,
  takeoff,
  totals,
  undo,
  type AiUi,
  type Command,
  type DrawingInfo,
  type Measurement,
  type Total,
} from "./api";
import { AssistantPanel } from "./ai/AssistantPanel";
import type { ToolCardItem } from "./ai/messages";
import { useAssistant } from "./ai/useAssistant";
import { useLanguage, useT } from "./i18n";
import { About } from "./shell/About";
import { LayersPanel } from "./shell/LayersPanel";
import { Ribbon, type Tool } from "./shell/Ribbon";
import { SettingsDialog } from "./shell/SettingsDialog";
import { SheetTabs } from "./shell/SheetTabs";
import { StatusBar } from "./shell/StatusBar";
import { TakeoffPanel } from "./shell/TakeoffPanel";
import { handlesBounds, measurementBounds, overlaySegments } from "./tools/overlay";
import { shortcutFor } from "./tools/shortcuts";
import { useTool } from "./tools/useTool";
import { parseBatch, type Batch } from "./viewport/batch";
import type { Theme } from "./viewport/gl";
import { Viewport, type ViewportHandle } from "./viewport/Viewport";

const THEME_KEY = "qiyas.theme";

/** Focus is in a text field: keys are typing, not shortcuts. */
const typing = (e: KeyboardEvent) =>
  e.target instanceof HTMLElement &&
  (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));

/** The stored theme, else the system's. */
function initialTheme(): Theme {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    if (stored === "light" || stored === "dark") return stored;
  } catch {
    // Storage blocked: fall through to the system theme.
  }
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export default function App() {
  const t = useT();
  const { lang, setLang } = useLanguage();
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [info, setInfo] = useState<DrawingInfo | null>(null);
  const [batch, setBatch] = useState<Batch | null>(null);
  const [sheet, setSheetBlock] = useState("");
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [tool, setTool] = useState<Tool>("select");
  const [cursor, setCursor] = useState<[number, number] | null>(null);
  const [opening, setOpening] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** A one-off status bar message, e.g. after an export; the next action clears it. */
  const [notice, setNotice] = useState<string | null>(null);
  const [items, setItems] = useState<Measurement[]>([]);
  const [itemTotals, setItemTotals] = useState<Total[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** The right panel's tab. */
  const [sideTab, setSideTab] = useState<"takeoff" | "assistant">("takeoff");
  /** Entities the assistant pointed at (Show on a tool card, or a zoom it asked for). */
  const [aiHighlight, setAiHighlight] = useState<number[]>([]);
  const viewport = useRef<ViewportHandle>(null);
  const viewportBox = useRef<HTMLDivElement>(null);
  /** Set when the next batch is a new drawing or sheet, so the view fits it once it is drawn. */
  const fitNext = useRef(false);
  /** True while a layer change talks to the core; clicks meanwhile are dropped, so they can't interleave. */
  const layerBusy = useRef(false);
  /** Set when autosave failed for this drawing, so the error shows once, not after every click. A new
   *  open or a successful save turns autosave back on. */
  const autosaveOff = useRef(false);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  // Runs after the viewport has taken the new batch (child effects run first).
  useEffect(() => {
    if (!fitNext.current) return;
    fitNext.current = false;
    viewport.current?.fitExtents();
  }, [batch]);

  function chooseTheme(next: Theme) {
    setTheme(next);
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch {
      // Storage blocked: the choice lasts for this session only.
    }
  }

  /** Runs an IPC step, showing its error in the status bar. */
  async function run(step: () => Promise<void>) {
    setNotice(null);
    try {
      await step();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function refreshTakeoff() {
    const [nextItems, nextTotals] = await Promise.all([takeoff(), totals()]);
    setItems(nextItems);
    setItemTotals(nextTotals);
  }

  /** Saves the take-off next to the drawing. A failure is shown once and turns autosave off, but the
   *  work goes on. */
  async function autosave() {
    if (autosaveOff.current) return;
    try {
      await saveProject();
    } catch (e) {
      autosaveOff.current = true;
      setError(t("project.autosaveOff", { error: String(e) }));
    }
  }

  /** Runs a take-off command; a new measurement becomes the selected row. */
  async function apply(cmd: Command) {
    let changed = false;
    await run(async () => {
      const outcome = await runCommand(cmd);
      if (outcome.type === "nothing") return;
      changed = true;
      await refreshTakeoff();
      if (outcome.type === "measured") setSelectedId(outcome.measurement.id);
    });
    if (changed) await autosave();
  }

  const measure = useTool(tool, {
    pick,
    entityInfo,
    run: apply,
    onError: (e) => setError(String(e)),
  });

  async function history(step: () => Promise<boolean>) {
    let changed = false;
    await run(async () => {
      if (!info || !(await step())) return;
      changed = true;
      await refreshTakeoff();
    });
    if (changed) await autosave();
  }

  async function redraw(fit = false) {
    const next = parseBatch(await renderBatch());
    fitNext.current = fit;
    setBatch(next);
  }

  /** Does what a tool's result asks of the window: another sheet, other layers, a zoom. */
  function applyAiUi(ui: AiUi) {
    run(async () => {
      if (ui.action === "show_sheet" && ui.sheet) {
        setSheetBlock(ui.sheet);
        measure.escape();
        await redraw(true);
      } else if (ui.action === "redraw") {
        if (ui.hidden) {
          // The model may write a layer's name in another case; the panel uses the drawing's.
          const names = new Set(ui.hidden.map((n) => n.toUpperCase()));
          setHidden(new Set(info?.layers.map((l) => l.name).filter((n) => names.has(n.toUpperCase()))));
        }
        await redraw();
      } else if (ui.bounds) {
        setAiHighlight(ui.handles ?? []);
        viewport.current?.fitTo(ui.bounds);
      }
    });
  }

  const assistant = useAssistant({
    lang,
    onMutated: async () => {
      await run(async () => {
        await refreshTakeoff();
        await redraw();
      });
      await autosave();
    },
    onUi: applyAiUi,
    onNeedSettings: () => setSettingsOpen(true),
  });

  const showCard = useCallback(
    (card: ToolCardItem) => {
      setAiHighlight(card.handles);
      const bounds = card.ui?.bounds ?? handlesBounds(card.handles, batch);
      if (bounds) viewport.current?.fitTo(bounds);
    },
    [batch],
  );

  const { markUndone, markUndoStale } = assistant;
  /** Undoes the card's change, unless the take-off changed since (then the card says so). */
  const undoCard = useCallback(
    async (card: ToolCardItem) => {
      let undone = false;
      await run(async () => {
        undone = await aiUndo();
        if (undone) await refreshTakeoff();
      });
      if (undone) {
        markUndone(card.id);
        await autosave();
      } else {
        markUndoStale(card.id);
      }
    },
    [markUndone, markUndoStale],
  );

  const openSettings = useCallback(() => setSettingsOpen(true), []);

  async function openFile() {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return;
    setOpening(true);
    await run(async () => {
      const next = await openDrawing(path);
      autosaveOff.current = false;
      setInfo(next);
      // The core opens a drawing on its first sheet.
      setSheetBlock(next.sheets[0]?.block ?? "");
      setHidden(new Set());
      setSelectedId(null);
      setAiHighlight([]);
      measure.escape();
      await refreshTakeoff();
      await redraw(true);
    });
    setOpening(false);
  }

  /** Writes the drawing with `write`, then shows its new layers and entities. */
  async function saveWith(write: () => Promise<void>) {
    await run(async () => {
      let next: DrawingInfo | null = null;
      try {
        await write().catch((e) => {
          throw t("save.failed", { error: String(e) });
        });
      } finally {
        // Even a failed save may have written the drawing and moved to its new path.
        next = await drawingInfo().catch(() => null);
        if (next) setInfo(next);
      }
      // The save wrote the project file too: autosave can try again.
      autosaveOff.current = false;
      await redraw();
      setNotice(t("save.done", { path: next?.path ?? "" }));
    });
  }

  /** Saves over the drawing's own file. A drawing that opened with problems is rewritten with only
   *  what Qiyas could read, so ask first; the core keeps the original as a `.bak` either way. */
  async function saveFile() {
    if (!info) return;
    if (
      info.problems.length > 0 &&
      !(await ask(t("save.confirmRecovered"), { title: t("appName"), kind: "warning" }))
    ) {
      return;
    }
    await saveWith(saveDrawing);
  }

  /** Asks where to save the drawing, as DWG or DXF, and saves it there. */
  async function saveFileAs() {
    if (!info) return;
    const path = await save({
      defaultPath: info.path,
      filters: [
        { name: "DWG", extensions: ["dwg"] },
        { name: "DXF", extensions: ["dxf"] },
      ],
    });
    if (!path) return;
    await saveWith(() => saveDrawingAs(path));
  }

  /** Asks where to save the Excel take-off, next to the drawing by default, and writes it. */
  async function exportFile() {
    if (!info) return;
    const path = await save({
      defaultPath: `${info.path.replace(/\.[^.\\/]*$/, "")}-takeoff.xlsx`,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (!path) return;
    await run(async () => {
      await exportTakeoff(path).catch((e) => {
        throw t("export.failed", { error: String(e) });
      });
      setNotice(t("export.done", { path }));
    });
  }

  const chooseSheet = (block: string) =>
    run(async () => {
      await setSheet(block);
      setSheetBlock(block);
      measure.escape();
      await redraw(true);
    });

  /** Moves the hidden set to `target`, one core call per changed layer. Whatever happens, the panel
   *  shows the calls that went through and the canvas is redrawn from the core. */
  async function applyHidden(target: Set<string>) {
    if (layerBusy.current) return;
    layerBusy.current = true;
    const done = new Set(hidden);
    let failure: string | null = null;
    try {
      for (const l of info?.layers ?? []) {
        const hide = target.has(l.name);
        if (done.has(l.name) === hide) continue;
        await setLayerHidden(l.name, hide);
        if (hide) done.add(l.name);
        else done.delete(l.name);
      }
    } catch (e) {
      failure = String(e);
    }
    setHidden(done);
    try {
      await redraw();
    } catch (e) {
      failure ??= String(e);
    }
    setError(failure);
    layerBusy.current = false;
  }

  // Stable between pointer moves, so the memoised layers panel doesn't re-render with the cursor.
  const toggleLayer = useCallback(
    (name: string) => {
      const next = new Set(hidden);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      applyHidden(next);
    },
    [hidden, info],
  );

  /** Shows only `name`: hides every other layer and unhides it. */
  const isolateLayer = useCallback(
    (name: string) =>
      applyHidden(new Set(info?.layers.map((l) => l.name).filter((n) => n !== name))),
    [hidden, info],
  );

  const showAll = useCallback(() => applyHidden(new Set()), [hidden, info]);

  // Hand the keyboard back to the drawing, so Space pans instead of pressing a button.
  const focusDrawing = () => viewportBox.current?.focus();

  function chooseTool(next: Tool) {
    setTool(next);
    focusDrawing();
  }

  // Keyboard: tool letters, Escape, Enter, undo, redo and save. Re-bound each render, so it sees fresh state.
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (typing(e)) return;
      const action = shortcutFor(e);
      if (action === "undo" || action === "redo" || action === "save") {
        if (action === "undo") history(undo);
        else if (action === "redo") history(redo);
        else saveFile();
        e.preventDefault();
      } else if (action === "escape") {
        measure.escape();
        setSelectedId(null);
        setAiHighlight([]);
      } else if (action === "finish") {
        // Enter on a focused button presses it; don't also finish the measure.
        if (e.target instanceof HTMLButtonElement) return;
        measure.finish();
      } else if (action && !e.repeat) {
        chooseTool(action);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  /** The selected row, when it was measured on the sheet in view. */
  const selected = items.find(
    (m) => m.id === selectedId && m.sheet.toLowerCase() === sheet.toLowerCase(),
  );

  const selectRow = useCallback(
    (id: string) => {
      setSelectedId(id);
      const m = items.find((i) => i.id === id);
      if (m?.sheet.toLowerCase() !== sheet.toLowerCase()) return;
      const bounds = measurementBounds(m, batch);
      if (bounds) viewport.current?.fitTo(bounds);
    },
    [items, sheet, batch],
  );
  const removeRow = useCallback((id: string) => apply({ type: "remove_measurement", id }), []);
  const renameRow = useCallback(
    (id: string, text: string) => apply({ type: "set_description", id, text }),
    [],
  );

  const highlight = useMemo(() => {
    const handles = new Set([...(selected?.handles ?? []), ...aiHighlight].map(BigInt));
    if (measure.highlighted !== null) handles.add(BigInt(measure.highlighted));
    return handles;
  }, [selected, measure.highlighted, aiHighlight]);

  const selectedOverlay = useMemo(
    () =>
      // A count's points are the counted places, not a line to draw.
      selected && selected.kind !== "count" && selected.points.length > 0
        ? overlaySegments(selected.points, null, selected.kind === "area" ? "area" : "distance")
        : null,
    [selected],
  );
  const rubberBand = useMemo(
    () => (measure.points.length > 0 ? overlaySegments(measure.points, cursor, tool) : null),
    [measure.points, cursor, tool],
  );
  // A measure in progress hides the selected row's outline.
  const overlay = rubberBand ?? selectedOverlay;

  const fileName = info?.path.split(/[\\/]/).pop() ?? "";
  const message = opening
    ? t("opening")
    : measure.hint
      ? t(measure.hint)
      : (notice ?? (info ? fileName : t("noDrawing")));
  // Unknown units are measured as millimetres; the status bar says so.
  const units = info ? t(`unit.${info.units === "unknown" ? "millimetres" : info.units}`) : "";

  return (
    <div className="app">
      <header className="top">
        <Ribbon
          active={tool}
          onTool={chooseTool}
          onOpen={openFile}
          onSave={info ? saveFile : undefined}
          onSaveAs={info ? saveFileAs : undefined}
          onExport={info ? exportFile : undefined}
          onUndo={() => history(undo)}
          onRedo={() => history(redo)}
          onZoomExtents={() => {
            viewport.current?.fitExtents();
            focusDrawing();
          }}
        />
        <div className="prefs">
          <div role="group" aria-label={t("language")}>
            <span>{t("language")}</span>
            <button type="button" lang="en" aria-pressed={lang === "en"} onClick={() => setLang("en")}>
              {t("langEn")}
            </button>
            <button type="button" lang="ar" aria-pressed={lang === "ar"} onClick={() => setLang("ar")}>
              {t("langAr")}
            </button>
          </div>
          <div role="group" aria-label={t("theme")}>
            <span>{t("theme")}</span>
            <button type="button" aria-pressed={theme === "light"} onClick={() => chooseTheme("light")}>
              {t("light")}
            </button>
            <button type="button" aria-pressed={theme === "dark"} onClick={() => chooseTheme("dark")}>
              {t("dark")}
            </button>
          </div>
          <button
            type="button"
            className="gear"
            aria-haspopup="dialog"
            aria-label={t("settings")}
            title={t("settings")}
            onClick={() => setSettingsOpen(true)}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <circle cx="8" cy="8" r="2.2" />
              <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
              <circle cx="8" cy="8" r="4.6" />
            </svg>
          </button>
          <button type="button" aria-haspopup="dialog" onClick={() => setAboutOpen(true)}>
            {t("about")}
          </button>
        </div>
      </header>
      {aboutOpen && <About theme={theme} onClose={() => setAboutOpen(false)} />}
      {settingsOpen && (
        <SettingsDialog
          onClose={() => {
            setSettingsOpen(false);
            assistant.refreshStatus();
          }}
        />
      )}
      <div className="middle">
        <LayersPanel
          layers={info?.layers ?? []}
          hidden={hidden}
          onToggle={toggleLayer}
          onIsolate={isolateLayer}
          onShowAll={showAll}
        />
        <main className="work">
          <div className="viewport" ref={viewportBox} tabIndex={0}>
            <Viewport
              ref={viewport}
              batch={batch}
              highlight={highlight}
              overlay={overlay}
              tool={tool}
              theme={theme}
              onClick={measure.onClick}
              onMove={(p) => setCursor(p)}
            />
          </div>
          {error && (
            <div className="error" role="alert">
              {error}
            </div>
          )}
          {info && <SheetTabs sheets={info.sheets} active={sheet} onSelect={chooseSheet} />}
        </main>
        <div className="side">
          <div className="side-tabs" role="tablist">
            <button
              type="button"
              role="tab"
              id="tab-takeoff"
              aria-controls="panel-takeoff"
              aria-selected={sideTab === "takeoff"}
              onClick={() => setSideTab("takeoff")}
            >
              {t("takeoff")}
            </button>
            <button
              type="button"
              role="tab"
              id="tab-assistant"
              aria-controls="panel-assistant"
              aria-selected={sideTab === "assistant"}
              onClick={() => setSideTab("assistant")}
            >
              {t("assistant")}
              {assistant.busy && <span className="spinner" aria-hidden="true" />}
            </button>
          </div>
          {/* Both stay mounted, so a draft or a running answer survives a tab switch. */}
          <div
            className="side-panel"
            role="tabpanel"
            id="panel-takeoff"
            aria-labelledby="tab-takeoff"
            hidden={sideTab !== "takeoff"}
          >
            <TakeoffPanel
              items={items}
              totals={itemTotals}
              selectedId={selectedId}
              onSelect={selectRow}
              onRemove={removeRow}
              onRename={renameRow}
            />
          </div>
          <div
            className="side-panel"
            role="tabpanel"
            id="panel-assistant"
            aria-labelledby="tab-assistant"
            hidden={sideTab !== "assistant"}
          >
            <AssistantPanel
              assistant={assistant}
              onShow={showCard}
              onUndo={undoCard}
              onSettings={openSettings}
            />
          </div>
        </div>
      </div>
      <StatusBar
        cursor={cursor}
        units={units}
        unitsWarning={info?.units === "unknown"}
        problems={info?.problems.length ?? 0}
        problemsText={info?.problems.join("\n") ?? ""}
        message={message}
      />
    </div>
  );
}

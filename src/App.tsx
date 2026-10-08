import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
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
  type Command,
  type DrawingInfo,
  type Measurement,
  type Total,
} from "./api";
import { useLanguage, useT } from "./i18n";
import { About } from "./shell/About";
import { LayersPanel } from "./shell/LayersPanel";
import { Ribbon, type Tool } from "./shell/Ribbon";
import { SheetTabs } from "./shell/SheetTabs";
import { StatusBar } from "./shell/StatusBar";
import { TakeoffPanel } from "./shell/TakeoffPanel";
import { measurementBounds, overlaySegments } from "./tools/overlay";
import { useTool } from "./tools/useTool";
import { parseBatch, type Batch } from "./viewport/batch";
import type { Theme } from "./viewport/gl";
import { Viewport, type ViewportHandle } from "./viewport/Viewport";

const THEME_KEY = "qiyas.theme";

/** Focus is in a text field: keys are typing, not shortcuts. */
const typing = (e: KeyboardEvent) =>
  e.target instanceof HTMLElement &&
  (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));

const TOOL_KEYS: Record<string, Tool> = { d: "distance", a: "area", c: "count", s: "select" };

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
  const [error, setError] = useState<string | null>(null);
  /** A one-off status bar message, e.g. after an export; the next action clears it. */
  const [notice, setNotice] = useState<string | null>(null);
  const [items, setItems] = useState<Measurement[]>([]);
  const [itemTotals, setItemTotals] = useState<Total[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const viewport = useRef<ViewportHandle>(null);
  const viewportBox = useRef<HTMLDivElement>(null);
  /** Set when the next batch is a new drawing or sheet, so the view fits it once it is drawn. */
  const fitNext = useRef(false);
  /** True while a layer change talks to the core; clicks meanwhile are dropped, so they can't interleave. */
  const layerBusy = useRef(false);

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

  /** Saves the take-off next to the drawing. A failure is shown, but the work goes on. */
  async function autosave() {
    try {
      await saveProject();
    } catch (e) {
      setError(String(e));
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

  async function openFile() {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return;
    setOpening(true);
    await run(async () => {
      const next = await openDrawing(path);
      setInfo(next);
      // The core opens a drawing on its first sheet.
      setSheetBlock(next.sheets[0]?.block ?? "");
      setHidden(new Set());
      setSelectedId(null);
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
      if (typing(e) || e.altKey || e.metaKey) return;
      const key = e.key.toLowerCase();
      if (e.ctrlKey) {
        if (key === "z" && !e.shiftKey) history(undo);
        else if (key === "y" || (key === "z" && e.shiftKey)) history(redo);
        else if (key === "s" && !e.shiftKey) saveFile();
        else return;
        e.preventDefault();
      } else if (e.key === "Escape") {
        measure.escape();
        setSelectedId(null);
      } else if (e.key === "Enter") {
        // Enter on a focused button presses it; don't also finish the measure.
        if (e.target instanceof HTMLButtonElement) return;
        measure.finish();
      } else if (TOOL_KEYS[key] && !e.shiftKey && !e.repeat) {
        chooseTool(TOOL_KEYS[key]);
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
    const handles = new Set((selected?.handles ?? []).map(BigInt));
    if (measure.highlighted !== null) handles.add(BigInt(measure.highlighted));
    return handles;
  }, [selected, measure.highlighted]);

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
          <button type="button" aria-haspopup="dialog" onClick={() => setAboutOpen(true)}>
            {t("about")}
          </button>
        </div>
      </header>
      {aboutOpen && <About theme={theme} onClose={() => setAboutOpen(false)} />}
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
        <TakeoffPanel
          items={items}
          totals={itemTotals}
          selectedId={selectedId}
          onSelect={selectRow}
          onRemove={removeRow}
          onRename={renameRow}
        />
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

import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import { openDrawing, renderBatch, setLayerHidden, setSheet, type DrawingInfo } from "./api";
import { useLanguage, useT } from "./i18n";
import { LayersPanel } from "./shell/LayersPanel";
import { Ribbon, type Tool } from "./shell/Ribbon";
import { SheetTabs } from "./shell/SheetTabs";
import { StatusBar } from "./shell/StatusBar";
import { parseBatch, type Batch } from "./viewport/batch";
import type { Theme } from "./viewport/gl";
import { Viewport, type ViewportHandle } from "./viewport/Viewport";

const NO_HIGHLIGHT = new Set<bigint>();
const THEME_KEY = "qiyas.theme";

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
  const [error, setError] = useState<string | null>(null);
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
    try {
      await step();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
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
      await redraw(true);
    });
    setOpening(false);
  }

  const chooseSheet = (block: string) =>
    run(async () => {
      await setSheet(block);
      setSheetBlock(block);
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

  const fileName = info?.path.split(/[\\/]/).pop() ?? "";
  const message = opening ? t("opening") : info ? fileName : t("noDrawing");
  // Unknown units are measured as millimetres; the status bar says so.
  const units = info ? t(`unit.${info.units === "unknown" ? "millimetres" : info.units}`) : "";

  return (
    <div className="app">
      <header className="top">
        <Ribbon
          active={tool}
          onTool={chooseTool}
          onOpen={openFile}
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
        </div>
      </header>
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
              highlight={NO_HIGHLIGHT}
              tool={tool}
              theme={theme}
              onClick={() => {}}
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
      </div>
      <StatusBar
        cursor={cursor}
        units={units}
        unitsWarning={info?.units === "unknown"}
        problems={info?.problems.length ?? 0}
        message={message}
      />
    </div>
  );
}

/** Plotting: the Plot dialog (the sheet on screen, its page setup and the drawing's plot style,
 *  kept per drawing in the window's storage: the project file has no place for them yet), the
 *  plot window's two clicks in the drawing, plot to PDF, print, quick print, print all layouts,
 *  and the plot style editor. */
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState, type RefObject } from "react";
import {
  defaultPrinter,
  listPrinters,
  loadPlotStyle,
  paperCatalog,
  plotAndPrint,
  plotPdf,
  type DrawingInfo,
  type PlotJob,
  type PlotStyle,
  type Sheet,
} from "../api";
import { defaultSetup, jobFor, normalized, type PageSetup } from "../shell/PlotDialog";
import type { Point } from "../viewport/camera";
import type { ViewportHandle } from "../viewport/Viewport";
import type { Measure, T } from "./shared";
import type { Shell } from "./useShell";

type Deps = Pick<Shell, "run" | "setError" | "setNotice" | "focusDrawing"> & {
  t: T;
  info: DrawingInfo | null;
  /** The sheet on screen, by its block. */
  sheet: string;
  /** The view on screen: quick print plots what it shows. */
  viewport: RefObject<Pick<ViewportHandle, "visible"> | null>;
  /** The tool in use, cancelled while the plot window is clicked. */
  measure: Pick<Measure, "cancel">;
};

/** What a drawing keeps for plotting: its plot style file, and each sheet's page setup by block. */
type Kept = { style: string | null; sheets: Record<string, PageSetup> };

const KEPT_KEY = "qiyas.plot:";

function readKept(drawing: string): Kept {
  try {
    const kept = JSON.parse(localStorage.getItem(KEPT_KEY + drawing) ?? "null") as Kept | null;
    if (kept && typeof kept === "object" && kept.sheets) return kept;
  } catch {
    // Storage blocked or spoilt: start from the defaults.
  }
  return { style: null, sheets: {} };
}

function writeKept(drawing: string, kept: Kept) {
  try {
    localStorage.setItem(KEPT_KEY + drawing, JSON.stringify(kept));
  } catch {
    // Storage blocked: the setup lasts while the dialog is open only.
  }
}

/** `path` ending in `.ext` (any case), added when the Save dialog left it out. */
const withExtension = (path: string, ext: string) =>
  path.toLowerCase().endsWith(`.${ext}`) ? path : `${path}.${ext}`;

/** The core's print messages the window says in its own words; others as the core said them. */
const SAID: Record<string, string> = {
  "Sent to the default printer": "plot.sentDefault",
  "Opened in the PDF viewer: print from there": "plot.openedViewer",
};

function printedNotice(t: T, said: string, printer: string | null): string {
  if (printer && said === `Sent to ${printer}`) return t("plot.sentTo", { printer });
  return SAID[said] ? t(SAID[said]) : said;
}

export function usePlot({ t, info, sheet, viewport, run, setError, setNotice, focusDrawing, measure }: Deps) {
  /** The Plot dialog's sheet and where it starts; null while it is closed. */
  const [plotAsk, setPlotAsk] = useState<{ sheet: Sheet; setup: PageSetup; stylePath: string | null } | null>(null);
  /** The plot style editor and the file it opens first; null while it is closed. */
  const [stylesAsk, setStylesAsk] = useState<{ initialPath: string | null } | null>(null);
  /** The plot window being clicked, with its first corner once given; null when none is. */
  const [windowPick, setWindowPick] = useState<{ first: Point | null } | null>(null);
  /** The last plot window clicked, for the dialog. */
  const [pickedWindow, setPickedWindow] = useState<[Point, Point] | null>(null);

  // Another drawing: the dialog and a window being clicked were for the one before.
  useEffect(() => {
    setPlotAsk(null);
    setWindowPick(null);
    setPickedWindow(null);
  }, [info?.path]);

  /** The sheet on screen (model space before one is shown). */
  const sheetShown = (drawing: DrawingInfo) =>
    drawing.sheets.find((s) => s.block.toLowerCase() === sheet.toLowerCase()) ??
    drawing.sheets.find((s) => s.is_model) ??
    drawing.sheets[0];

  function openPlot() {
    if (!info) {
      setError(t("cmd.noDrawing"));
      return;
    }
    const shown = sheetShown(info);
    const kept = readKept(info.path);
    setPickedWindow(null);
    setPlotAsk({ sheet: shown, setup: kept.sheets[shown.block] ?? defaultSetup(shown.is_model), stylePath: kept.style });
  }

  function closePlot() {
    setPlotAsk(null);
    setWindowPick(null);
    focusDrawing();
  }

  /** Keeps the dialog's setup for its sheet, and the plot style for the drawing. */
  function keep(setup: PageSetup, stylePath: string | null) {
    if (!info || !plotAsk) return;
    const kept = readKept(info.path);
    kept.sheets[plotAsk.sheet.block] = setup;
    kept.style = stylePath;
    writeKept(info.path, kept);
  }

  function startWindowPick() {
    measure.cancel();
    setWindowPick({ first: null });
    focusDrawing();
  }

  /** Gives `p` to the plot window being clicked; false when none is. */
  function pickPoint(p: Point): boolean {
    if (!windowPick) return false;
    if (!windowPick.first) {
      setWindowPick({ first: p });
    } else {
      setPickedWindow([windowPick.first, p]);
      setWindowPick(null);
    }
    return true;
  }

  /** Escape while the window is clicked: back to the dialog without one. */
  const cancelPick = () => setWindowPick(null);

  async function plotToPdf(job: PlotJob) {
    const out = await plotPdf(job);
    setError(null);
    setNotice(
      out.skipped.length > 0
        ? t("plot.doneSkipped", { path: out.path, n: out.skipped.length })
        : t("plot.done", { path: out.path }),
    );
  }

  async function print(job: PlotJob, printer: string | null) {
    const said = await plotAndPrint(job, printer);
    setError(null);
    setNotice(printedNotice(t, said, printer));
  }

  /** The printers and the default one; none when they can't be listed. */
  async function printers() {
    const [list, chosen] = await Promise.all([listPrinters().catch(() => []), defaultPrinter().catch(() => null)]);
    return { printers: list, default: chosen };
  }

  /** The papers, the default printer and the drawing's plot style, for a print without the dialog. */
  async function printing(kept: Kept): Promise<[Awaited<ReturnType<typeof paperCatalog>>, string | null, PlotStyle | null]> {
    return Promise.all([
      paperCatalog(),
      defaultPrinter().catch(() => null),
      kept.style ? loadPlotStyle(kept.style) : null,
    ]);
  }

  /** QUICKPRINT: the view on screen, fitted to the sheet's paper (turned to suit the view), on
   *  the default printer, with the sheet's kept setup otherwise. */
  async function quickPrint() {
    if (!info) {
      setError(t("cmd.noDrawing"));
      return;
    }
    const drawing = info;
    const shown = sheetShown(drawing);
    const view = viewport.current?.visible() ?? null;
    await run(async () => {
      const kept = readKept(drawing.path);
      const base = kept.sheets[shown.block] ?? defaultSetup(shown.is_model);
      const window = view ? normalized(view) : null;
      const tall = window !== null && window[1][1] - window[0][1] > window[1][0] - window[0][0];
      const setup: PageSetup = {
        ...base,
        area: window ? "window" : "extents",
        window,
        scale: "fit",
        orientation: tall ? "portrait" : "landscape",
      };
      setNotice(t("plot.plotting"));
      const [papers, printer, style] = await printing(kept);
      const said = await plotAndPrint(jobFor(shown.block, setup, papers, style, null), printer);
      setNotice(printedNotice(t, said, printer));
    });
  }

  /** PUBLISH: every layout (not model space), one print job each on the default printer, each
   *  with its kept setup; a layout that fails doesn't stop the others. */
  async function printAll() {
    if (!info) {
      setError(t("cmd.noDrawing"));
      return;
    }
    const drawing = info;
    const layouts = drawing.sheets.filter((s) => !s.is_model);
    if (layouts.length === 0) {
      setError(t("plot.noLayouts"));
      return;
    }
    await run(async () => {
      const kept = readKept(drawing.path);
      setNotice(t("plot.plotting"));
      const [papers, printer, style] = await printing(kept);
      const failed: string[] = [];
      let said = "";
      for (const s of layouts) {
        const setup = kept.sheets[s.block] ?? defaultSetup(false);
        try {
          said = await plotAndPrint(jobFor(s.block, setup, papers, style, null), printer);
        } catch (e) {
          failed.push(`${s.name}: ${String(e)}`);
        }
      }
      if (failed.length > 0) {
        throw t("plot.printAllFailed", { n: failed.length, total: layouts.length, errors: failed.join("; ") });
      }
      setNotice(t("plot.printedAll", { n: layouts.length, said: printedNotice(t, said, printer) }));
    });
  }

  /** STYLESMANAGER: the editor, on the drawing's plot style when it keeps one. */
  function openStyles() {
    setStylesAsk({ initialPath: info ? readKept(info.path).style : null });
  }

  const closeStyles = () => setStylesAsk(null);

  async function chooseStyle() {
    const path = await open({ filters: [{ name: "CTB, STB", extensions: ["ctb", "stb"] }] });
    return typeof path === "string" ? path : null;
  }

  async function chooseStyleSave(kind: PlotStyle["kind"], defaultPath: string) {
    const path = await save({ defaultPath, filters: [{ name: kind.toUpperCase(), extensions: [kind] }] });
    return path ? withExtension(path, kind) : null;
  }

  async function choosePdf(defaultPath: string) {
    const path = await save({ defaultPath, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    return path ? withExtension(path, "pdf") : null;
  }

  return {
    plotAsk,
    openPlot,
    closePlot,
    keep,
    windowPick,
    pickedWindow,
    startWindowPick,
    pickPoint,
    cancelPick,
    plotToPdf,
    print,
    printers,
    quickPrint,
    printAll,
    stylesAsk,
    openStyles,
    closeStyles,
    chooseStyle,
    chooseStyleSave,
    choosePdf,
  };
}

export type Plot = ReturnType<typeof usePlot>;

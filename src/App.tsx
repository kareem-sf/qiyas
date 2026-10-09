import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, message as messageDialog, open, save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  aiUndo,
  attachImage,
  attachPdf,
  calibrate,
  IMAGE_EXTENSIONS,
  imageDisplay,
  imageSize,
  isDirty,
  PDF_PROGRESS,
  pdfPageCount,
  pdfPageSize,
  pdfThumbnails,
  underlayImage,
  boq as readBoq,
  compare,
  conflicts as readConflicts,
  drawingInfo,
  entityInfo,
  getDrafting,
  setDrafting,
  snap,
  exportConflicts,
  exportTable,
  exportTakeoff,
  importBoq,
  openDrawing,
  pick,
  redo,
  reloadXrefs,
  atlasPages,
  batchVersion,
  renderBatch,
  runCommand,
  saveDrawing,
  saveDrawingAs,
  saveProject,
  SESSION_CHANGED,
  setLayerHidden,
  setSheet,
  setXrefPath,
  TAKEOFF_CHANGED,
  takeoff,
  totals,
  undo,
  type AiUi,
  type Boq,
  type Command,
  type Conflict,
  type Drafting,
  type DrawingInfo,
  type EntityFilter,
  type Field,
  type ImageDisplay,
  type Measurement,
  type Total,
} from "./api";
import { AssistantPanel } from "./ai/AssistantPanel";
import type { ToolCardItem } from "./ai/messages";
import { useAssistant } from "./ai/useAssistant";
import { BoqPanel } from "./boq/BoqPanel";
import { ConflictsPanel, type ResolveCommand } from "./boq/ConflictsPanel";
import { openCount } from "./boq/conflictsMath";
import { useLanguage, useT } from "./i18n";
import { About } from "./shell/About";
import {
  AttachImageDialog,
  defaultImageWidth,
  ImageDisplayPanel,
  type AttachImageRequest,
} from "./shell/AttachImageDialog";
import { AttachPdfDialog, CalibrateDialog, type AttachRequest } from "./shell/AttachPdfDialog";
import { DraftingSettings } from "./shell/DraftingSettings";
import { ExtractDialog } from "./shell/ExtractDialog";
import { AlongPrompt, InfoCard, type Info } from "./shell/InfoCard";
import { LayersPanel } from "./shell/LayersPanel";
import { Ribbon, type Tool } from "./shell/Ribbon";
import { SettingsDialog } from "./shell/SettingsDialog";
import { SheetTabs } from "./shell/SheetTabs";
import { StatusBar } from "./shell/StatusBar";
import { TakeoffPanel } from "./shell/TakeoffPanel";
import { XrefManager } from "./shell/XrefManager";
import { leaveDrawing, readChoice } from "./shell/unsaved";
import {
  handlesBounds,
  markSize,
  measurementBounds,
  measurementTargets,
  overlaySegments,
  pointMarks,
} from "./tools/overlay";
import { shortcutFor, type DraftingToggle } from "./tools/shortcuts";
import { useTool } from "./tools/useTool";
import {
  parseAtlas,
  parseBatch,
  parsePicture,
  parseThumbnails,
  picturesToFetch,
  picturesToRelease,
  type AtlasPage,
  type Batch,
  type HeldPicture,
  type Target,
} from "./viewport/batch";
import type { Bounds, Point } from "./viewport/camera";
import type { Theme } from "./viewport/gl";
import { Viewport, type ViewportHandle } from "./viewport/Viewport";

const THEME_KEY = "qiyas.theme";

/** The drafting setting each switch turns on and off. */
const TOGGLED: Record<DraftingToggle, "osnap" | "grid" | "ortho" | "polar"> = {
  osnap: "osnap",
  grid: "grid",
  ortho: "ortho",
  polar: "polar",
};

/** A click in the drawing the window is waiting for instead of the tool's: the insert point of a
 *  PDF page or an image, or the two points of a calibration. */
type Pending = { kind: "insert"; of: "pdf" | "image" } | { kind: "calibrate"; first: Point | null };

/** Focus is in a text field: keys are typing, not shortcuts. */
const typing = (e: KeyboardEvent) =>
  e.target instanceof HTMLElement &&
  (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));

/** The box around the measurements' bounds on the drawing, or null when none has bounds. */
function unionBounds(ms: Measurement[], batch: Batch | null): Bounds | null {
  let box: Bounds | null = null;
  for (const m of ms) {
    const b = measurementBounds(m, batch);
    if (!b) continue;
    box = box
      ? [
          [Math.min(box[0][0], b[0][0]), Math.min(box[0][1], b[0][1])],
          [Math.max(box[1][0], b[1][0]), Math.max(box[1][1], b[1][1])],
        ]
      : b;
  }
  return box;
}

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
  /** The batch in hand and the core's version of it (`batchVersion`), so a redraw that changed
   *  nothing on the sheet doesn't fetch the whole batch again. */
  const batchHeld = useRef<Batch | null>(null);
  const batchVersionRef = useRef<number | null>(null);
  /** Bumped to run the fit effect when a redraw keeps the batch it has. */
  const [fitTick, setFitTick] = useState(0);
  /** Every glyph atlas page received for this drawing, and the atlas version they make up. */
  const [atlas, setAtlas] = useState<AtlasPage[]>([]);
  const atlasHeld = useRef(new Map<number, AtlasPage>());
  const atlasVersionRef = useRef<number | null>(null);
  /** True while glyphs are fetched for a new zoom, so zooming on doesn't stack requests. */
  const glyphsBusy = useRef(false);
  /** The underlay pictures held, by image id, and whether some are being fetched. */
  const [pictures, setPictures] = useState(new Map<number, HeldPicture>());
  const picturesHeld = useRef(new Map<number, HeldPicture>());
  const picturesBusy = useRef(false);
  /** Percent drawn of the PDF page pdfium is rendering; null when none is. */
  const [renderProgress, setRenderProgress] = useState<number | null>(null);
  const [attachOpen, setAttachOpen] = useState(false);
  const [attachImageOpen, setAttachImageOpen] = useState(false);
  /** The image the select tool has chosen, and its brightness, contrast and fade. */
  const [imageChosen, setImageChosen] = useState<{ handle: number; display: ImageDisplay } | null>(
    null,
  );
  /** A click the window waits for (see `Pending`), the insert point it gave, and the two
   *  calibration points once both are picked. */
  const [pending, setPending] = useState<Pending | null>(null);
  const [pickedInsert, setPickedInsert] = useState<Point | null>(null);
  const [calibration, setCalibration] = useState<[Point, Point] | null>(null);
  /** The drawing holds its Arabic text backwards (View tab). */
  const [arabicReversed, setArabicReversed] = useState(false);
  const [sheet, setSheetBlock] = useState("");
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [tool, setTool] = useState<Tool>("select");
  /** The drawing's snaps, ortho, polar, grid and display precision; null with no drawing. */
  const [drafting, setDraftingState] = useState<Drafting | null>(null);
  const [draftingOpen, setDraftingOpen] = useState(false);
  const [opening, setOpening] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [xrefsOpen, setXrefsOpen] = useState(false);
  const [extractOpen, setExtractOpen] = useState(false);
  /** What the last inquiry (geometry, ID point) found, shown until closed. */
  const [infoCard, setInfoCard] = useState<Info | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** A one-off status bar message, e.g. after an export; the next action clears it. */
  const [notice, setNotice] = useState<string | null>(null);
  const [items, setItems] = useState<Measurement[]>([]);
  const [itemTotals, setItemTotals] = useState<Total[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** The drawing's imported BOQ. */
  const [bill, setBill] = useState<Boq | null>(null);
  /** The BOQ item whose linked measurements are shown. */
  const [boqCode, setBoqCode] = useState<string | null>(null);
  /** The last comparison's conflicts. */
  const [conflicts, setConflicts] = useState<Conflict[]>([]);
  /** A comparison ran for this drawing, so no conflicts means none were found. */
  const [compared, setCompared] = useState(false);
  /** The measurements a conflict's Show pointed at. */
  const [conflictIds, setConflictIds] = useState<string[]>([]);
  /** The right panel's tab. */
  const [sideTab, setSideTab] = useState<"takeoff" | "boq" | "conflicts" | "assistant">("takeoff");
  /** Entities the assistant pointed at (Show on a tool card, or a zoom it asked for). */
  const [aiHighlight, setAiHighlight] = useState<number[]>([]);
  const viewport = useRef<ViewportHandle>(null);
  const viewportBox = useRef<HTMLDivElement>(null);
  /** Set when the next batch is a new drawing or sheet, so the view fits it once it is drawn. */
  const fitNext = useRef(false);
  /** Measurements to fit instead of the whole sheet, when a conflict's Show changed the sheet. */
  const fitIds = useRef<string[] | null>(null);
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
    const ids = fitIds.current;
    fitIds.current = null;
    const box = ids && unionBounds(items.filter((m) => ids.includes(m.id)), batch);
    if (box) viewport.current?.fitTo(box);
    else viewport.current?.fitExtents();
  }, [batch, fitTick]);

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

  /** Re-reads the take-off, the BOQ and the conflicts, after anything that may have changed them. */
  async function refreshTakeoffAndBoq() {
    const [nextItems, nextTotals, nextBoq, nextConflicts] = await Promise.all([
      takeoff(),
      totals(),
      readBoq(),
      readConflicts(),
    ]);
    setItems(nextItems);
    setItemTotals(nextTotals);
    setBill(nextBoq);
    setConflicts(nextConflicts);
    // Conflicts kept in the project file come from an earlier comparison.
    if (nextConflicts.length > 0) setCompared(true);
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

  /** Runs a take-off command; a new measurement becomes the selected row. An inquiry's answer
   *  shows in the info card. */
  async function apply(cmd: Command) {
    let changed = false;
    await run(async () => {
      const outcome = await runCommand(cmd);
      if (outcome.type === "nothing") return;
      if (outcome.type === "info") {
        setInfoCard({ title: outcome.title, rows: outcome.rows });
        return;
      }
      changed = true;
      await refreshTakeoffAndBoq();
      // Divide and measure along add points to the drawing itself.
      if (cmd.type === "divide_along" || cmd.type === "measure_along") await redraw();
      if (outcome.type === "measured") setSelectedId(outcome.measurement.id);
    });
    if (changed) await autosave();
  }

  const measure = useTool(
    tool,
    {
      pick,
      snap,
      entityInfo,
      run: apply,
      onError: (e) => setError(String(e)),
    },
    drafting,
  );

  /** Makes `next` the drawing's drafting settings and saves them with the project. Throws when the
   *  core refuses them. */
  async function saveDrafting(next: Drafting) {
    await setDrafting(next);
    setDraftingState(next);
    await autosave();
  }

  /** Turns one drafting switch on or off (status bar or F3, F7, F8, F10). Ortho and polar exclude
   *  each other, as in AutoCAD. */
  function toggleDrafting(toggle: DraftingToggle) {
    if (!drafting) return;
    const key = TOGGLED[toggle];
    const next: Drafting = { ...drafting, [key]: !drafting[key] };
    if (toggle === "ortho" && next.ortho) next.polar = false;
    if (toggle === "polar" && next.polar) next.ortho = false;
    run(() => saveDrafting(next));
  }

  async function history(step: () => Promise<boolean>) {
    let changed = false;
    await run(async () => {
      if (!info || !(await step())) return;
      changed = true;
      await refreshTakeoffAndBoq();
      // The step may have been a PDF import, an underlay or a calibration.
      await redraw();
    });
    if (changed) await autosave();
  }

  /** Fetches the atlas pages `next` needs that the window doesn't hold yet. */
  async function syncAtlas(next: Batch) {
    if (next.atlasVersion === 0 || next.atlasVersion === atlasVersionRef.current) return;
    const update = parseAtlas(await atlasPages(atlasVersionRef.current ?? undefined));
    for (const page of update.pages) atlasHeld.current.set(page.index, page);
    atlasVersionRef.current = update.version;
    setAtlas([...atlasHeld.current.values()]);
  }

  /** Draws the core's current batch. When the core says the drawing, sheet and layers are as they
   *  were at the last draw, the batch in hand stays and only the fit (if asked) happens. A fit draws
   *  the glyphs for the core's own guess at the zoom; otherwise for the zoom on screen. */
  async function redraw(fit = false) {
    const version = await batchVersion();
    if (version === batchVersionRef.current && batchHeld.current) {
      // Nothing to send; the effect below still fits the view once the window has rendered.
      if (fit) {
        fitNext.current = true;
        setFitTick((n) => n + 1);
      }
      return;
    }
    const bytes = await renderBatch(undefined, fit ? undefined : viewport.current?.pxPerUnit());
    if (!bytes) return;
    const next = parseBatch(bytes);
    await syncAtlas(next);
    batchVersionRef.current = version;
    batchHeld.current = next;
    fitNext.current = fit;
    setBatch(next);
  }

  /** Fetches the underlay pictures in `visible` that `next` needs for a zoom of `pxPerUnit` and
   *  the window doesn't hold at that version and about that zoom, and lets go of those out of view
   *  (their textures go too). One fetch runs at a time; the viewport asks again when the view
   *  settles. */
  async function syncPictures(next: Batch | null, pxPerUnit: number, visible: Bounds) {
    if (picturesBusy.current) return;
    picturesBusy.current = true;
    try {
      const held = picturesHeld.current;
      for (const id of picturesToRelease(next, held, visible)) held.delete(id);
      for (const id of picturesToFetch(next, held, pxPerUnit, visible)) {
        const entry = next!.imageTable.find((e) => e.id === id)!;
        const picture = parsePicture(await underlayImage(id, pxPerUnit));
        held.set(id, { ...picture, version: entry.version, pxPerUnit });
      }
      setPictures(new Map(held));
    } finally {
      picturesBusy.current = false;
      setRenderProgress(null);
    }
  }

  /** Draws the text again for the zoom on screen (`pxPerUnit` screen pixels per drawing unit). The
   *  lines of the batch in hand stay when the core's batch is the same one. */
  async function redrawGlyphs(pxPerUnit: number) {
    if (!batchHeld.current || glyphsBusy.current) return;
    glyphsBusy.current = true;
    await run(async () => {
      const version = await batchVersion();
      const bytes = await renderBatch(undefined, pxPerUnit);
      if (!bytes) return;
      let next = parseBatch(bytes);
      await syncAtlas(next);
      const held = batchHeld.current;
      if (held && version === batchVersionRef.current) {
        // The same lines: keep the arrays the viewport has uploaded already.
        const { segments, colors, ranges, chains } = held;
        next = { ...next, segments, colors, ranges, chains };
      }
      batchVersionRef.current = version;
      batchHeld.current = next;
      setBatch(next);
    });
    glyphsBusy.current = false;
  }

  /** Says whether the drawing holds its Arabic text backwards, and redraws its text. */
  async function chooseArabicReversed(on: boolean) {
    let changed = false;
    await run(async () => {
      await runCommand({ type: "set_arabic_reversed", on });
      setArabicReversed(on);
      changed = true;
      await redraw();
    });
    if (changed) await autosave();
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
    // The take-off is drawn from `items`; the batch is fetched again only when the drawing
    // itself changed (divide_along, measure_along).
    onMutated: async (tool) => {
      if (tool === "compare_boq") setCompared(true);
      await run(async () => {
        await refreshTakeoffAndBoq();
        await redraw();
      });
      await autosave();
    },
    onUi: applyAiUi,
    onNeedSettings: () => setSettingsOpen(true),
  });

  // An outside AI app changed the take-off through the MCP server: show it and save it, as after
  // the assistant's changes. The listener is set up once and calls the latest code through this.
  const onMcpChange = useRef<() => Promise<void>>(async () => {});
  onMcpChange.current = async () => {
    await run(async () => {
      await refreshTakeoffAndBoq();
      await redraw();
    });
    await autosave();
  };
  // An outside AI app showed another sheet, hid or showed layers, or asked for a zoom: follow it,
  // so the sheet tabs, the layers panel and the canvas show what the session now shows.
  const onMcpUi = useRef<(ui: AiUi) => void>(() => {});
  onMcpUi.current = (ui) => {
    run(async () => {
      setInfo(await drawingInfo());
      applyAiUi(ui);
    });
  };
  useEffect(() => {
    let gone = false;
    const offs: (() => void)[] = [];
    const keep = (unlisten: () => void) => {
      if (gone) unlisten();
      else offs.push(unlisten);
    };
    listen(TAKEOFF_CHANGED, () => onMcpChange.current()).then(keep);
    listen<number>(PDF_PROGRESS, (e) => setRenderProgress(e.payload)).then(keep);
    listen<AiUi>(SESSION_CHANGED, (e) => onMcpUi.current(e.payload)).then(keep);
    return () => {
      gone = true;
      offs.forEach((off) => off());
    };
  }, []);

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
        if (undone) {
          await refreshTakeoffAndBoq();
          await redraw();
        }
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

  /** Before the open drawing is left: asks "Save changes to the drawing?" when the drawing itself
   *  has unsaved changes, and saves when told to. False when the user cancels. */
  async function leaveOpenDrawing(): Promise<boolean> {
    if (!info) return true;
    const labels = { yes: t("unsaved.yes"), no: t("unsaved.no"), cancel: t("unsaved.cancel") };
    return leaveDrawing({
      isDirty,
      ask: async () =>
        readChoice(
          await messageDialog(t("unsaved.question"), {
            title: t("appName"),
            kind: "warning",
            buttons: labels,
          }),
          labels,
        ),
      save: saveDrawing,
    });
  }
  const leaveRef = useRef(leaveOpenDrawing);
  leaveRef.current = leaveOpenDrawing;

  // Closing the window with unsaved drawing changes asks first; a failed save keeps it open.
  useEffect(() => {
    let gone = false;
    let off: (() => void) | null = null;
    getCurrentWindow()
      .onCloseRequested(async (event) => {
        try {
          if (!(await leaveRef.current())) event.preventDefault();
        } catch (e) {
          event.preventDefault();
          setError(t("save.failed", { error: String(e) }));
        }
      })
      .then((unlisten) => (gone ? unlisten() : (off = unlisten)))
      .catch(() => {});
    return () => {
      gone = true;
      off?.();
    };
  }, []);

  async function openFile() {
    try {
      if (!(await leaveOpenDrawing())) return;
    } catch (e) {
      setError(t("save.failed", { error: String(e) }));
      return;
    }
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return;
    setOpening(true);
    await run(async () => {
      const next = await openDrawing(path);
      batchVersionRef.current = null;
      batchHeld.current = null;
      // A new drawing has a new atlas.
      atlasVersionRef.current = null;
      atlasHeld.current = new Map();
      setAtlas([]);
      // And no underlay pictures yet.
      picturesHeld.current = new Map();
      setPictures(new Map());
      setArabicReversed(next.arabic_reversed);
      setDraftingState(await getDrafting());
      autosaveOff.current = false;
      setInfo(next);
      // The core opens a drawing on its first sheet.
      setSheetBlock(next.sheets[0]?.block ?? "");
      setHidden(new Set());
      setSelectedId(null);
      setBoqCode(null);
      setAiHighlight([]);
      setConflictIds([]);
      setCompared(false);
      setInfoCard(null);
      measure.escape();
      await refreshTakeoffAndBoq();
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
      // An xref's problems are not the drawing's own: saving never touches the xref.
      info.problems.length > info.xrefs.reduce((n, x) => n + x.problems.length, 0) &&
      !(await ask(t("save.confirmRecovered"), { title: t("appName"), kind: "warning" }))
    ) {
      return;
    }
    await saveWith(saveDrawing);
  }

  /** Shows the xrefs as the core now has them (their layers, problems and drawing) and saves the
   *  project, which remembers a file chosen for one. Errors go to the xrefs dialog. */
  async function xrefsChanged(next: DrawingInfo) {
    setInfo(next);
    await redraw();
    await autosave();
  }

  /** Asks for the file of the xref `name` and loads it from there. */
  async function locateXref(name: string) {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return;
    await xrefsChanged(await setXrefPath(name, path));
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

  /** Extract data: runs each filter on the sheet shown and gathers the rows. */
  async function extractRows(filters: EntityFilter[], fields: Field[]): Promise<string[][]> {
    const rows: string[][] = [];
    for (const filter of filters) {
      const outcome = await runCommand({ type: "extract_data", filter, fields });
      if (outcome.type === "table") rows.push(...outcome.rows);
    }
    return rows;
  }

  /** Asks where to save the extracted table, next to the drawing by default, and writes it. */
  async function exportExtract(columns: string[], rows: string[][]) {
    if (!info) return;
    const path = await save({
      defaultPath: `${info.path.replace(/\.[^.\\/]*$/, "")}-data.xlsx`,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (!path) return;
    await exportTable(path, columns, rows).catch((e) => {
      throw t("export.failed", { error: String(e) });
    });
    setNotice(t("extract.done", { path }));
  }

  /** Asks for an Excel BOQ and makes it the drawing's BOQ (an undo step), then shows it. */
  async function importBoqFile() {
    if (!info) return;
    const path = await open({ filters: [{ name: "Excel", extensions: ["xlsx", "xlsm"] }] });
    if (typeof path !== "string") return;
    let changed = false;
    await run(async () => {
      await importBoq(path);
      changed = true;
      setBoqCode(null);
      setSideTab("boq");
      await refreshTakeoffAndBoq();
    });
    if (changed) await autosave();
  }

  // Stable for the memoised BOQ panel, which would otherwise re-render on every pointer move.
  const importBoqRef = useRef(importBoqFile);
  importBoqRef.current = importBoqFile;
  const onImportBoq = useCallback(() => importBoqRef.current(), []);

  /** Compares the BOQ and the take-off with the drawings (an undo step) and shows the conflicts. */
  async function compareNow() {
    if (!info) return;
    let changed = false;
    await run(async () => {
      const found = await compare();
      changed = true;
      setConflicts(found);
      setCompared(true);
      setSideTab("conflicts");
      setNotice(
        found.length === 0 ? t("conflicts.none") : t("conflicts.found", { n: found.length }),
      );
    });
    if (changed) await autosave();
  }

  /** Asks where to save the conflicts workbook, next to the drawing by default, and writes it. */
  async function exportConflictsFile() {
    if (!info) return;
    const path = await save({
      defaultPath: `${info.path.replace(/\.[^.\\/]*$/, "")}-conflicts.xlsx`,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (!path) return;
    await run(async () => {
      await exportConflicts(path).catch((e) => {
        throw t("export.failed", { error: String(e) });
      });
      setNotice(t("export.done", { path }));
    });
  }

  // Stable for the memoised conflicts panel.
  const compareRef = useRef(compareNow);
  compareRef.current = compareNow;
  const onCompare = useCallback(() => compareRef.current(), []);
  const exportConflictsRef = useRef(exportConflictsFile);
  exportConflictsRef.current = exportConflictsFile;
  const onExportConflicts = useCallback(() => exportConflictsRef.current(), []);

  /** Shows the sheet `block`, fitted to the measurements `fit` when given, else to the whole sheet. */
  const chooseSheet = (block: string, fit?: string[]) =>
    run(async () => {
      await setSheet(block);
      setSheetBlock(block);
      measure.escape();
      fitIds.current = fit ?? null;
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

  /** Attaches a PDF page from the dialog, then draws it: an underlay's picture is fetched before
   *  the dialog closes, so its progress shows there. Errors go to the dialog. */
  async function attachPdfPage(r: AttachRequest) {
    const added = await attachPdf(r.path, r.page, r.at, r.scale, r.mode);
    setInfo(await drawingInfo());
    await redraw();
    const view = viewport.current;
    if (r.mode === "underlay" && view) {
      await syncPictures(batchHeld.current, view.pxPerUnit(), view.visible());
    } else {
      setNotice(t("attachPdf.imported", { n: added }));
    }
  }

  /** Attaches an image from the dialog, then draws it with its picture. Errors go to the dialog. */
  async function attachImageFile(r: AttachImageRequest) {
    await attachImage(r.path, r.at, r.width);
    setInfo(await drawingInfo());
    await redraw();
    const view = viewport.current;
    if (view) await syncPictures(batchHeld.current, view.pxPerUnit(), view.visible());
  }

  /** Sets the chosen image's brightness, contrast and fade (one undo step), then draws it. */
  async function setChosenDisplay(handle: number, display: ImageDisplay) {
    await runCommand({ type: "set_image_display", handle, ...display });
    await redraw();
  }

  /** A click in the drawing: the one a PDF or image insert point or a calibration waits for, else
   *  the tool's. */
  function clickDrawing(world: Point, screen: Point, worldPerPixel: number) {
    if (pending?.kind === "insert") {
      setPickedInsert(world);
      setPending(null);
    } else if (pending?.kind === "calibrate" && !pending.first) {
      setPending({ kind: "calibrate", first: world });
    } else if (pending?.kind === "calibrate" && pending.first) {
      setCalibration([pending.first, world]);
      setPending(null);
    } else {
      measure.onClick(world, screen, worldPerPixel);
    }
  }

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
      if (action === "osnap" || action === "grid" || action === "ortho" || action === "polar") {
        // F7 and F10 also mean something to the web view; not here.
        e.preventDefault();
        if (!e.repeat) toggleDrafting(action);
      } else if (action === "undo" || action === "redo" || action === "save") {
        if (action === "undo") history(undo);
        else if (action === "redo") history(redo);
        else saveFile();
        e.preventDefault();
      } else if (action === "escape") {
        setPending(null);
        setInfoCard(null);
        measure.escape();
        setSelectedId(null);
        setBoqCode(null);
        setAiHighlight([]);
        setConflictIds([]);
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

  const onSheet = (m: Measurement) => m.sheet.toLowerCase() === sheet.toLowerCase();

  /** The measurements on the sheet in view linked to the selected BOQ item. */
  const boqLinked = useMemo(
    () =>
      boqCode !== null && bill?.items.some((i) => i.code === boqCode)
        ? items.filter((m) => m.boq_code === boqCode && onSheet(m))
        : [],
    [boqCode, bill, items, sheet],
  );

  /** The measurements on the sheet in view that a conflict's Show pointed at. */
  const conflictLinked = useMemo(
    () => items.filter((m) => conflictIds.includes(m.id) && onSheet(m)),
    [conflictIds, items, sheet],
  );

  const selectRow = useCallback(
    (id: string) => {
      setSelectedId(id);
      setBoqCode(null);
      setConflictIds([]);
      const m = items.find((i) => i.id === id);
      if (m?.sheet.toLowerCase() !== sheet.toLowerCase()) return;
      const bounds = measurementBounds(m, batch);
      if (bounds) viewport.current?.fitTo(bounds);
    },
    [items, sheet, batch],
  );
  /** Selects a BOQ item: its linked measurements on this sheet light up and fill the view. */
  const selectBoq = useCallback(
    (code: string) => {
      setBoqCode(code);
      setSelectedId(null);
      setAiHighlight([]);
      setConflictIds([]);
      const linked = items.filter((m) => code !== "" && m.boq_code === code && onSheet(m));
      const box = unionBounds(linked, batch);
      if (box) viewport.current?.fitTo(box);
    },
    [items, sheet, batch],
  );
  /** Shows what a conflict is about: its measurements lit up and filling the view (moving to their
   *  sheet when none is on this one); else its BOQ item in the BOQ tab; else its first sheet. */
  const showConflict = useCallback(
    (c: Conflict) => {
      const involved = items.filter((m) => c.measurements.includes(m.id));
      if (involved.length > 0) {
        setSelectedId(null);
        setBoqCode(null);
        setAiHighlight([]);
        setConflictIds(involved.map((m) => m.id));
        const here = involved.filter(onSheet);
        if (here.length > 0) {
          const box = unionBounds(here, batch);
          if (box) viewport.current?.fitTo(box);
        } else {
          const ids = involved.filter((m) => m.sheet === involved[0].sheet).map((m) => m.id);
          chooseSheet(involved[0].sheet, ids);
        }
      } else if (c.boq_code) {
        setSideTab("boq");
        selectBoq(c.boq_code);
      } else {
        const target = info?.sheets.find((s) => c.sheets.includes(s.name));
        if (target && target.block.toLowerCase() !== sheet.toLowerCase()) chooseSheet(target.block);
      }
    },
    [items, sheet, batch, info, selectBoq],
  );
  const resolveConflict = useCallback((cmd: ResolveCommand) => apply(cmd), []);
  const openBoqTab = useCallback(() => setSideTab("boq"), []);
  const linkRow = useCallback(
    (id: string, code: string | null) => apply({ type: "link_boq", id, code }),
    [],
  );
  const setTolerance = useCallback(
    (code: string, percent: number | null) => apply({ type: "set_boq_tolerance", code, percent }),
    [],
  );
  const removeRow = useCallback((id: string) => apply({ type: "remove_measurement", id }), []);
  const renameRow = useCallback(
    (id: string, text: string) => apply({ type: "set_description", id, text }),
    [],
  );

  // An image chosen with the select tool shows its brightness, contrast and fade; read again when
  // the batch changes (a set, an undo or a redo).
  const chosen = measure.highlighted;
  const chosenHandle = chosen && chosen.chain.length === 0 ? chosen.handle : null;
  useEffect(() => {
    if (chosenHandle === null) {
      setImageChosen(null);
      return;
    }
    let gone = false;
    imageDisplay(chosenHandle)
      .then((display) => !gone && setImageChosen(display ? { handle: chosenHandle, display } : null))
      .catch(() => !gone && setImageChosen(null));
    return () => {
      gone = true;
    };
  }, [chosenHandle, batch]);

  const highlight = useMemo(() => {
    const rows = [...(selected ? [selected] : []), ...boqLinked, ...conflictLinked];
    const targets: Target[] = [
      ...rows.flatMap(measurementTargets),
      ...aiHighlight.map((handle) => ({ handle, chain: [] })),
    ];
    if (measure.highlighted !== null) targets.push(measure.highlighted);
    return targets;
  }, [selected, boqLinked, conflictLinked, measure.highlighted, aiHighlight]);

  const selectedOverlay = useMemo(() => {
    const rows = selected ? [selected] : [...boqLinked, ...conflictLinked];
    // A count's points are the counted places: small crosses, not a line.
    const size = markSize(batch?.bounds ?? null);
    const outlines = rows
      .filter((m) => m.points.length > 0)
      .map((m) =>
        m.kind === "count"
          ? pointMarks(m.points, size)
          : overlaySegments(m.points, null, m.kind === "area" ? "area" : "distance"),
      );
    if (outlines.length === 0) return null;
    const all = new Float64Array(outlines.reduce((n, o) => n + o.length, 0));
    outlines.reduce((at, o) => (all.set(o, at), at + o.length), 0);
    return all;
  }, [selected, boqLinked, conflictLinked, batch]);
  const rubberBand = useMemo(
    () =>
      measure.points.length > 0 ? overlaySegments(measure.points, measure.cursor, tool) : null,
    [measure.points, measure.cursor, tool],
  );
  // A measure in progress hides the selected row's outline; the snap marker goes over either.
  const overlay = useMemo(() => {
    const lines = rubberBand ?? selectedOverlay;
    const marker = measure.marker;
    if (!marker) return lines;
    if (!lines) return marker;
    const all = new Float64Array(lines.length + marker.length);
    all.set(lines);
    all.set(marker, lines.length);
    return all;
  }, [rubberBand, selectedOverlay, measure.marker]);

  const openConflicts = openCount(conflicts);
  const fileName = info?.path.split(/[\\/]/).pop() ?? "";
  const message = opening
    ? t("opening")
    : pending
      ? t(
          pending.kind === "insert"
            ? pending.of === "image"
              ? "attachImage.pickHint"
              : "attachPdf.pickHint"
            : pending.first
              ? "calibrate.second"
              : "calibrate.first",
        )
      : renderProgress !== null && !attachOpen
        ? t("pdf.drawing", { n: renderProgress })
        : measure.hint
      ? t(measure.hint, measure.hintArgs ?? undefined)
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
          onImportBoq={info ? importBoqFile : undefined}
          onCompare={info ? compareNow : undefined}
          onExportConflicts={info ? exportConflictsFile : undefined}
          onUndo={() => history(undo)}
          onRedo={() => history(redo)}
          onZoomExtents={() => {
            viewport.current?.fitExtents();
            focusDrawing();
          }}
          onXrefs={info ? () => setXrefsOpen(true) : undefined}
          onAttachPdf={
            info
              ? () => {
                  setPickedInsert(null);
                  setAttachOpen(true);
                }
              : undefined
          }
          onAttachImage={
            info
              ? () => {
                  setPickedInsert(null);
                  setAttachImageOpen(true);
                }
              : undefined
          }
          onCalibrate={
            batch && batch.imageTable.length > 0
              ? () => {
                  measure.escape();
                  setPending({ kind: "calibrate", first: null });
                  focusDrawing();
                }
              : undefined
          }
          onExtract={info ? () => setExtractOpen(true) : undefined}
          arabicReversed={arabicReversed}
          onArabicReversed={info ? chooseArabicReversed : undefined}
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
      {xrefsOpen && info && (
        <XrefManager
          xrefs={info.xrefs}
          onReload={async () => xrefsChanged(await reloadXrefs())}
          onLocate={locateXref}
          onClear={async (name) => xrefsChanged(await setXrefPath(name, null))}
          onClose={() => setXrefsOpen(false)}
        />
      )}
      {attachOpen && info && (
        <AttachPdfDialog
          units={info.units}
          picked={pickedInsert}
          picking={pending?.kind === "insert" && pending.of === "pdf"}
          progress={renderProgress}
          chooseFile={async () => {
            const path = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
            return typeof path === "string" ? path : null;
          }}
          pageCount={pdfPageCount}
          pageSize={pdfPageSize}
          thumbnails={async (path, first, count) =>
            parseThumbnails(await pdfThumbnails(path, first, count, 96))
          }
          onPickPoint={() => setPending({ kind: "insert", of: "pdf" })}
          onAttach={attachPdfPage}
          onClose={() => {
            setAttachOpen(false);
            setPending(null);
          }}
        />
      )}
      {attachImageOpen && info && (
        <AttachImageDialog
          units={info.units}
          defaultWidth={defaultImageWidth(batch?.bounds ?? null)}
          picked={pickedInsert}
          picking={pending?.kind === "insert" && pending.of === "image"}
          chooseFile={async () => {
            const path = await open({
              filters: [{ name: t("attachImage.file"), extensions: IMAGE_EXTENSIONS }],
            });
            return typeof path === "string" ? path : null;
          }}
          imageSize={imageSize}
          onPickPoint={() => setPending({ kind: "insert", of: "image" })}
          onAttach={attachImageFile}
          onClose={() => {
            setAttachImageOpen(false);
            setPending(null);
          }}
        />
      )}
      {calibration && info && (
        <CalibrateDialog
          points={calibration}
          units={info.units}
          onCalibrate={async (distance) => {
            await calibrate(calibration[0], calibration[1], distance);
            await redraw();
          }}
          onClose={() => setCalibration(null)}
        />
      )}
      {extractOpen && info && (
        <ExtractDialog
          layers={info.layers.map((l) => l.name)}
          onRun={extractRows}
          onExport={exportExtract}
          onClose={() => setExtractOpen(false)}
        />
      )}
      {draftingOpen && drafting && (
        <DraftingSettings
          drafting={drafting}
          onSave={saveDrafting}
          onClose={() => setDraftingOpen(false)}
        />
      )}
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
              atlas={atlas}
              onZoomHint={redrawGlyphs}
              pictures={pictures}
              onPictureHint={(px, visible) => run(() => syncPictures(batchHeld.current, px, visible))}
              highlight={highlight}
              overlay={overlay}
              tool={tool}
              theme={theme}
              onClick={clickDrawing}
              grid={drafting?.grid ? drafting.grid_spacing : null}
              onMove={measure.onMove}
            />
            {measure.asking && info && (
              <AlongPrompt
                key={measure.asking}
                kind={measure.asking}
                units={info.units}
                onAnswer={(value) => {
                  measure.answer(value);
                  focusDrawing();
                }}
                onCancel={() => {
                  measure.escape();
                  focusDrawing();
                }}
              />
            )}
            {imageChosen && (
              <ImageDisplayPanel
                value={imageChosen.display}
                onCommit={(display) => setChosenDisplay(imageChosen.handle, display)}
                onClose={() => setImageChosen(null)}
              />
            )}
          </div>
          {error && (
            <div className="error" role="alert">
              {error}
            </div>
          )}
          {info && <SheetTabs sheets={info.sheets} active={sheet} onSelect={chooseSheet} />}
        </main>
        {/* The BOQ table and the conflicts need more room than the take-off. */}
        <div className={sideTab === "boq" || sideTab === "conflicts" ? "side wide" : "side"}>
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
              id="tab-boq"
              aria-controls="panel-boq"
              aria-selected={sideTab === "boq"}
              onClick={() => setSideTab("boq")}
            >
              {t("boq")}
            </button>
            <button
              type="button"
              role="tab"
              id="tab-conflicts"
              aria-controls="panel-conflicts"
              aria-selected={sideTab === "conflicts"}
              onClick={() => setSideTab("conflicts")}
            >
              {t("conflicts")}
              {openConflicts > 0 && (
                <span className="tab-count" title={t("conflicts.openCount", { n: openConflicts })}>
                  {openConflicts}
                </span>
              )}
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
          {infoCard && <InfoCard info={infoCard} onClose={() => setInfoCard(null)} />}
          {/* All stay mounted, so a draft, a search or a running answer survives a tab switch. */}
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
              boq={bill}
              onLink={linkRow}
              precision={drafting?.precision ?? 3}
            />
          </div>
          <div
            className="side-panel"
            role="tabpanel"
            id="panel-boq"
            aria-labelledby="tab-boq"
            hidden={sideTab !== "boq"}
          >
            <BoqPanel
              boq={bill}
              takeoff={items}
              selectedCode={boqCode}
              onSelect={selectBoq}
              onImport={info ? onImportBoq : undefined}
              onTolerance={setTolerance}
            />
          </div>
          <div
            className="side-panel"
            role="tabpanel"
            id="panel-conflicts"
            aria-labelledby="tab-conflicts"
            hidden={sideTab !== "conflicts"}
          >
            <ConflictsPanel
              conflicts={conflicts}
              compared={compared}
              hasBoq={bill !== null}
              onCompare={info ? onCompare : undefined}
              onExport={info ? onExportConflicts : undefined}
              onShow={showConflict}
              onResolve={resolveConflict}
              onOpenBoq={openBoqTab}
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
        cursor={measure.cursor}
        precision={drafting?.precision ?? 3}
        snapLabel={measure.snap ? t(`snap.${measure.snap.kind}`) : null}
        drafting={info ? drafting : null}
        onToggle={toggleDrafting}
        onDraftingSettings={() => setDraftingOpen(true)}
        units={units}
        unitsWarning={info?.units === "unknown"}
        problems={info?.problems.length ?? 0}
        problemsText={info?.problems.join("\n") ?? ""}
        message={message}
      />
    </div>
  );
}

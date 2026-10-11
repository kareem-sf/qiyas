import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, message as messageDialog, open, save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  activateTab,
  blocks as readBlocks,
  clearRecents,
  drawingBlocks,
  getSettings,
  setToolPalette,
  closeTab,
  EMPTY_MODEL,
  recents as readRecents,
  tabs as readTabs,
  views as readViews,
  calibrate,
  IMAGE_EXTENSIONS,
  imageDisplay,
  imageSize,
  isDirty,
  pdfPageCount,
  pdfPageSize,
  pdfThumbnails,
  compare,
  drawingInfo,
  entityInfo,
  getDrafting,
  snap,
  exportConflicts,
  exportTable,
  exportTakeoff,
  importBoq,
  openDrawing,
  pick,
  pickWindow,
  redo,
  reloadXrefs,
  boundaryAt,
  runCommand,
  saveDrawing,
  saveDrawingAs,
  setXrefPath,
  undo,
  autosaveDrawings,
  discardAutosave,
  recoverableDrawings,
  recoverAutosave,
  recoverDrawing,
  sheetSetEntry,
  sheetSetFromTabs,
  sheetSetOpen,
  sheetSetSave,
  type ReadReport,
  type Conflict,
  type DrawingInfo,
  type EntityFilter,
  type EntityInfo,
  type EntityProp,
  type Field,
  type ImageDisplay,
  type Measurement,
  type NamedView,
} from "../api";
import { AssistantPanel } from "../ai/AssistantPanel";
import { BoqPanel } from "../boq/BoqPanel";
import { ConflictsPanel, type ResolveCommand } from "../boq/ConflictsPanel";
import { openCount } from "../boq/conflictsMath";
import { useLanguage, useT } from "../i18n";
import { About } from "../shell/About";
import {
  AttachImageDialog,
  defaultImageWidth,
  ImageDisplayPanel,
} from "../shell/AttachImageDialog";
import {
  AttachPdfDialog,
  CalibrateDialog,
  metresPerUnit,
} from "../shell/AttachPdfDialog";
import {
  AttDefDialog,
  AttributeManager,
  AttributesDialog,
  BlockEditBar,
  CreateBlockDialog,
} from "../shell/BlockDialogs";
import { BlockPalette } from "../shell/BlockPalette";
import { CommandLine } from "../shell/CommandLine";
import { DraftingSettings } from "../shell/DraftingSettings";
import { ExtractDialog } from "../shell/ExtractDialog";
import { FindReplaceDialog } from "../shell/FindReplaceDialog";
import { cycleTab, FileTabs } from "../shell/FileTabs";
import { isStale, serial } from "../shell/switcher";
import { AlongPrompt, InfoCard } from "../shell/InfoCard";
import { GroupNameDialog, LayerStatesDialog } from "../shell/LayerDialogs";
import { LayersPanel } from "../shell/LayersPanel";
import { LayerTranslator } from "../shell/LayerTranslator";
import { PropertiesPanel } from "../shell/PropertiesPanel";
import { RecoverReportDialog, RecoveryDialog } from "../shell/RecoveryDialog";
import { drop, enqueue, type RecoveryAsk } from "../shell/recoveryQueue";
import { SheetSetManager, type SheetSetApi } from "../shell/SheetSetManager";
import { Ribbon } from "../shell/Ribbon";
import { SettingsDialog } from "../shell/SettingsDialog";
import { DimStylePanel } from "../shell/DimStylePanel";
import { DesignCenter } from "../shell/DesignCenter";
import { ToolPalettes } from "../shell/ToolPalettes";
import { InPlaceText, TextDialog } from "../shell/TextEditor";
import { ToleranceDialog } from "../shell/ToleranceDialog";
import { NewTableDialog, TableEditor } from "../shell/TableEditor";
import { FieldDialog } from "../shell/FieldDialog";
import { SheetTabs } from "../shell/SheetTabs";
import { StatusBar } from "../shell/StatusBar";
import { TakeoffPanel } from "../shell/TakeoffPanel";
import { XrefManager } from "../shell/XrefManager";
import { leaveDrawing, readChoice } from "../shell/unsaved";
import {
  markSize,
  measurementBounds,
  measurementTargets,
  overlaySegments,
  pointMarks,
} from "../tools/overlay";
import type { Action, Expect, ParseState } from "../tools/commandParser";
import { shortcutFor } from "../tools/shortcuts";
import { useTool } from "../tools/useTool";
import {
  parseThumbnails,
  type Target,
} from "../viewport/batch";
import type { Point } from "../viewport/camera";
import { SplitPane } from "../viewport/SplitPane";
import { Viewport } from "../viewport/Viewport";
import { NO_VIEWPORTS, readProblems, typing, unionBounds } from "./shared";
import { useDrafting } from "./useDrafting";
import { useAnnotate } from "./useAnnotate";
import { useAssistantWiring } from "./useAssistantWiring";
import { useAttach } from "./useAttach";
import { useBlocks } from "./useBlocks";
import { type Commands, useCommands, useToolChoice } from "./useCommands";
import { useAutosave, useFileTabs } from "./useDrawings";
import { useLayers } from "./useLayers";
import { useSheets } from "./useSheets";
import { useShell } from "./useShell";
import { useTakeoffState } from "./useTakeoff";

export default function App() {
  const t = useT();
  const { lang, setLang } = useLanguage();
  const [info, setInfo] = useState<DrawingInfo | null>(null);
  const shell = useShell();
  const {
    theme,
    chooseTheme,
    aboutOpen,
    setAboutOpen,
    settingsOpen,
    setSettingsOpen,
    commandAliases,
    loadAliases,
    error,
    setError,
    notice,
    setNotice,
    sideTab,
    setSideTab,
    viewport,
    viewportBox,
    run,
    focusDrawing,
  } = shell;
  const { fileTabs, setFileTabs, activeTab, setActiveTab, refreshTabs } = useFileTabs();
  const { autosaveOff, autosave } = useAutosave(t, setError);
  /** Bumped whenever another drawing comes on screen (open, tab switch, close): work started
   *  before is stale (`isStale`) and drops its results. */
  const switchGen = useRef(0);
  const { drafting, setDraftingState, draftingOpen, setDraftingOpen, saveDrafting, toggleDrafting } = useDrafting({
    run,
    autosave,
  });
  const takeoffState = useTakeoffState(switchGen);
  const {
    items,
    itemTotals,
    selectedId,
    setSelectedId,
    bill,
    boqCode,
    setBoqCode,
    conflicts,
    setConflicts,
    compared,
    setCompared,
    conflictIds,
    setConflictIds,
    aiHighlight,
    setAiHighlight,
    infoCard,
    setInfoCard,
    refreshTakeoffAndBoq,
  } = takeoffState;
  /** The tool hook comes after the sheets (it takes their state); the sheets cancel it through this. */
  const measureRef = useRef<{ cancel(): void } | null>(null);
  const sheets = useSheets({ switchGen, viewport, run, autosave, setInfo, refreshTabs, items, measure: measureRef });
  const {
    batch,
    batchHeld,
    inset,
    split,
    setSplit,
    atlas,
    pictures,
    renderProgress,
    setRenderProgress,
    arabicReversed,
    setArabicReversed,
    sheet,
    setSheetBlock,
    hidden,
    setHidden,
    fitMatch,
    syncAtlas,
    redraw,
    syncPictures,
    redrawGlyphs,
    chooseArabicReversed,
    chooseSheet,
    saveView,
    deleteView,
  } = sheets;
  const layers = useLayers({
    switchGen,
    info,
    setInfo,
    setError,
    run,
    autosave,
    redraw,
    refreshTakeoffAndBoq,
    hidden,
    setHidden,
  });
  const {
    layerStatesOpen,
    setLayerStatesOpen,
    translatorOpen,
    setTranslatorOpen,
    groupAsk,
    setGroupAsk,
    groupSelect,
    setGroupSelect,
    layersBusy,
    runDrawingCommand,
    runLayerCommand,
    groupOf,
    loadLayerStandard,
    syncInfo,
    applyHidden,
    toggleLayer,
    isolateLayer,
    showAll,
    setLayerProp,
    renameLayer,
    deleteLayer,
    newLayer,
    openLayerStates,
    openTranslator,
    setViewportFrozen,
    setCurrentState,
    ownLayers,
    makeCurrent,
  } = layers;
  const toolChoice = useToolChoice(focusDrawing);
  const { tool, setTool, lastPoint, preset, chooseTool } = toolChoice;
  const attach = useAttach({ t, setInfo, setNotice, viewport, batchHeld, redraw, syncPictures });
  const {
    attachOpen,
    setAttachOpen,
    attachImageOpen,
    setAttachImageOpen,
    pending,
    setPending,
    pickedInsert,
    setPickedInsert,
    calibration,
    setCalibration,
    attachPdfPage,
    attachImageFile,
    pendingPoint,
  } = attach;
  /** `apply` and `onCommand` come after the hooks that call them back (a block placed, a
   *  palette's command line); those reach them through these, filled once they exist. */
  const applyRef = useRef<Commands["apply"] | null>(null);
  const onCommandRef = useRef<((a: Action) => void) | null>(null);
  const blocks = useBlocks({
    t,
    switchGen,
    info,
    setInfo,
    setError,
    setNotice,
    run,
    autosave,
    redraw,
    refreshTakeoffAndBoq,
    focusDrawing,
    chooseTool,
    commandAliases,
    apply: applyRef,
    onCommand: onCommandRef,
  });
  const {
    createBlockAsk,
    setCreateBlockAsk,
    attributesAsk,
    setAttributesAsk,
    attDefAt,
    setAttDefAt,
    blockPaletteOpen,
    setBlockPaletteOpen,
    designCenter,
    setDesignCenter,
    palette,
    setPalette,
    managerBlocks,
    setManagerBlocks,
    runBlock,
    blockPicture,
    closeBlockPalette,
    startInsert,
    placeBlock,
    writeBlockFile,
    closeBlockEdit,
    openAttributeManager,
    openDesignCenter,
    openToolPalettes,
    addToPalette,
    runPaletteItem,
  } = blocks;
  const annotate = useAnnotate({
    t,
    switchGen,
    info,
    setInfo,
    setNotice,
    run,
    autosave,
    focusDrawing,
    viewport,
    redraw,
    sheet,
    fitMatch,
    chooseSheet,
    batchHeld,
    setAiHighlight,
    placeBlock,
    setAttDefAt,
    setAttributesAsk,
    apply: applyRef,
  });
  const {
    findOpen,
    setFindOpen,
    dimStyleOpen,
    setDimStyleOpen,
    dimText,
    setDimText,
    newTableAt,
    setNewTableAt,
    tableEditing,
    setTableEditing,
    fieldAsk,
    setFieldAsk,
    fieldTarget,
    setFieldTarget,
    setTextHeight,
    setTextStyle,
    textEditor,
    setTextEditor,
    textDialog,
    setTextDialog,
    currentTextHeight,
    currentTextStyle,
    placeText,
    openTextEdit,
    runDimStyle,
    openDimStyle,
    runText,
    runTable,
    insertField,
    updateFields,
    placeInPlace,
    findTexts,
    replaceTexts,
    showMatch,
  } = annotate;
  /** Autosaves to offer back (on launch, or before opening `then`), one question at a time. */
  const [recoveries, setRecoveries] = useState<RecoveryAsk[]>([]);
  const recovery = recoveries[0] ?? null;
  /** What RECOVER read and dropped from a damaged drawing. */
  const [recoverReport, setRecoverReport] = useState<{ path: string; report: ReadReport } | null>(null);
  const [sheetSetShown, setSheetSetShown] = useState(false);
  /** The image the select tool has chosen, and its brightness, contrast and fade. */
  const [imageChosen, setImageChosen] = useState<{ handle: number; display: ImageDisplay } | null>(
    null,
  );
  const [opening, setOpening] = useState(false);
  const [xrefsOpen, setXrefsOpen] = useState(false);
  const [extractOpen, setExtractOpen] = useState(false);
  /** The command line shows (Ctrl+9), its input, and whether to focus it once shown. */
  const [commandLineOn, setCommandLineOn] = useState(true);
  const commandInput = useRef<HTMLInputElement>(null);
  const focusCommandLine = useRef(false);
  /** What the select tool picked, for the properties palette. */
  const [pickedInfo, setPickedInfo] = useState<EntityInfo | null>(null);
  /** Drawing changes waiting or running; while any is, clicks and tool input on the canvas are
   *  ignored (they would land on a drawing about to go). */
  const switching = useRef(0);
  /** Opens, switches and closes reach the core one at a time, in the order asked. */
  const inTurn = useRef(serial()).current;
  /** The drawing on screen now, for checks inside queued work. */
  const infoRef = useRef(info);
  infoRef.current = info;

  const { apply, history } = useCommands({
    t,
    setInfo,
    info,
    setNotice,
    run,
    autosave,
    redraw,
    refreshTakeoffAndBoq,
    setInfoCard,
    setSelectedId,
    chooseTool,
    syncInfo,
    runDrawingCommand,
    setGroupAsk,
    setCreateBlockAsk,
    currentTextHeight,
  });
  applyRef.current = apply;

  const measure = useTool(
    tool,
    {
      pick,
      pickWindow,
      snap,
      entityInfo,
      run: apply,
      zoomTo: (bounds) => viewport.current?.fitTo(bounds),
      boundary: (p) => {
        // About 50 screen pixels across first; the core looks further until the area fits.
        const pxPerUnit = viewport.current?.pxPerUnit() ?? 1;
        return boundaryAt(p[0], p[1], 25 / pxPerUnit);
      },
      hatchScale: () => 0.025 / metresPerUnit(info?.units ?? "unknown"),
      undo: () => history(undo),
      onError: (e) => setError(String(e)),
      placeText,
      editText: (handle, at) => run(() => openTextEdit(handle, at)),
      dimText: setDimText,
      groupOf,
    },
    drafting,
    info?.ucs,
  );
  measureRef.current = measure;
  onCommandRef.current = onCommand;
  const { assistant, showCard, undoCard, openSettings } = useAssistantWiring({
    lang,
    switchGen,
    info,
    setInfo,
    run,
    autosave,
    setSettingsOpen,
    viewport,
    batch,
    setSheetBlock,
    setHidden,
    setRenderProgress,
    redraw,
    refreshTakeoffAndBoq,
    setCompared,
    setAiHighlight,
    syncInfo,
    measure,
  });






  /** Before the open drawing is left: asks "Save changes to the drawing?" when the drawing itself
   *  has unsaved changes, and saves when told to. False when the user cancels. */
  async function leaveOpenDrawing(): Promise<boolean> {
    if (!info) return true;
    const labels = { yes: t("unsaved.yes"), no: t("unsaved.no"), cancel: t("unsaved.cancel") };
    return leaveDrawing({
      isDirty,
      ask: async () => {
        const choice = readChoice(
          await messageDialog(t("unsaved.question"), {
            title: t("appName"),
            kind: "warning",
            buttons: labels,
          }),
          labels,
        );
        // Don't Save: its autosave goes too (the window closing, or the tab). The drawing on
        // screen now: closing the window shows each drawing before asking.
        if (choice === "discard") await discardAutosave((await drawingInfo()).path).catch(() => {});
        return choice;
      },
      // The drawing on screen now: closing the window shows each drawing before asking.
      confirmSave: async () => confirmRecoveredSave(await drawingInfo()),
      save: saveDrawing,
    });
  }
  /** Before the window closes: asks about each open drawing with unsaved changes in turn, showing
   *  it first. False when the user cancels one. */
  async function leaveAllDrawings(): Promise<boolean> {
    const { tabs: open } = await readTabs();
    for (const tab of open.filter((f) => f.dirty)) {
      if (!(await switchTab(tab.id))) return false;
      if (!(await leaveOpenDrawing())) return false;
    }
    return true;
  }
  const leaveRef = useRef(leaveAllDrawings);
  leaveRef.current = leaveAllDrawings;

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

  // On launch: the recent drawings' autosaves newer than them are offered back.
  useEffect(() => {
    (async () => {
      const paths = (await readRecents()).map((r) => r.path);
      const found = await recoverableDrawings(paths);
      if (found.length > 0) setRecoveries((q) => enqueue(q, found, null, true));
    })().catch(() => {});
  }, []);

  // Autosave: every few minutes (settings, 5 by default, 0 off) the drawings with unsaved changes
  // are written to <name>.sv$ beside them, with their project.
  useEffect(() => {
    let timer = 0;
    let gone = false;
    getSettings()
      .then((s) => {
        const minutes = s.autosave_minutes ?? 5;
        if (gone || !(minutes > 0)) return;
        timer = window.setInterval(() => {
          autosaveDrawings()
            .then((done) => {
              if (done.failed.length > 0) setError(t("autosave.failed", { error: done.failed.join("; ") }));
            })
            .catch((e) => setError(t("autosave.failed", { error: String(e) })));
        }, minutes * 60_000);
      })
      .catch(() => {});
    return () => {
      gone = true;
      clearInterval(timer);
    };
  }, [settingsOpen]);

  /** Forgets everything the window holds about the drawing on screen: the batch, the glyph atlas
   *  and the pictures (all per drawing), the selection, the tool's points, prompts and dialogs. */
  function resetDrawingView() {
    sheets.reset();
    autosaveOff.current = false;
    takeoffState.reset();
    attach.reset();
    setXrefsOpen(false);
    setExtractOpen(false);
    preset.current = null;
    measure.cancel();
    lastPoint.current = null;
  }

  /** Another drawing comes on screen: earlier work for the one there goes stale. */
  function beginShow(): number {
    switchGen.current += 1;
    return switchGen.current;
  }

  /** Shows the drawing the core has on screen now (just opened, or its tab chosen) as its session
   *  keeps it: its sheet, hidden layers, take-off, BOQ and conflicts. `gen` is `beginShow`'s; a
   *  later drawing change stops it at the next step. */
  async function showDrawing(next: DrawingInfo, gen: number) {
    resetDrawingView();
    setArabicReversed(next.arabic_reversed);
    setInfo(next);
    setSheetBlock(next.sheet);
    setHidden(new Set(next.hidden_layers));
    const drafting = await getDrafting();
    if (isStale(gen, switchGen.current)) return;
    setDraftingState(drafting);
    await refreshTakeoffAndBoq();
    if (isStale(gen, switchGen.current)) return;
    await redraw(true);
  }

  /** After the last tab closed: an empty window. */
  function showNoDrawing() {
    beginShow();
    resetDrawingView();
    sheets.clear();
    setInfo(null);
    setDraftingState(null);
    takeoffState.clear();
  }

  /** Runs a drawing change (open, tab switch, close) in turn after those asked before it; canvas
   *  input is ignored until all are done. */
  async function drawingChange<T>(job: () => Promise<T>): Promise<T> {
    switching.current += 1;
    try {
      return await inTurn(job);
    } finally {
      switching.current -= 1;
    }
  }

  /** Opens the drawing at `path` in a new tab, or shows its tab when it is open already; nothing
   *  happens when it is the drawing on screen. An autosave newer than the drawing is offered first
   *  (`recover` false skips that: the user chose to open the drawing itself). */
  async function openPath(path: string, recover = true) {
    if (recover) {
      const found = await recoverableDrawings([path]).catch(() => []);
      const open = fileTabs.some((f) => f.path === path);
      if (found.length > 0 && !open) {
        setRecoveries((q) => enqueue(q, found, path));
        return;
      }
    }
    setOpening(true);
    await drawingChange(() =>
      run(async () => {
        const next = await openDrawing(path);
        // The drawing on screen asked for again: the core changed nothing.
        if (infoRef.current && next.path === infoRef.current.path) return;
        await showDrawing(next, beginShow());
      }),
    );
    setOpening(false);
  }

  async function openFile() {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return;
    await openPath(path);
  }

  /** Recover: the autosave of `drawing` opens in a new tab as a new drawing beside it, and Save As
   *  asks where it goes. */
  async function recoverFrom(drawing: string) {
    let shown = false;
    await drawingChange(async () => {
      const next = await recoverAutosave(drawing);
      await showDrawing(next, beginShow());
      await refreshTabs();
      shown = true;
    });
    dropRecovery(drawing);
    if (shown) await saveFileAs();
  }

  /** Discard: the autosave of `drawing` is deleted; the drawing about to open opens. */
  async function discardFrom(drawing: string) {
    await discardAutosave(drawing);
    const then = recovery?.then;
    dropRecovery(drawing);
    if (then === drawing) await openPath(drawing, false);
  }

  /** `drawing` is dealt with: the question goes when nothing is left in it. */
  function dropRecovery(drawing: string) {
    setRecoveries((q) => drop(q, drawing));
  }

  /** RECOVER: a damaged drawing opens in a new tab through the failsafe reader, and the dialog
   *  says what was dropped. */
  async function recoverFile() {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return;
    await drawingChange(() =>
      run(async () => {
        const { info: next, report } = await recoverDrawing(path);
        await showDrawing(next, beginShow());
        await refreshTabs();
        setRecoverReport({ path, report });
      }),
    );
  }

  /** The sheet set manager's file dialogs and core commands. */
  const sheetSetApi: SheetSetApi = {
    open: sheetSetOpen,
    save: sheetSetSave,
    fromTabs: sheetSetFromTabs,
    entry: sheetSetEntry,
    pickNew: async () =>
      (await save({ filters: [{ name: t("sheetSet.title"), extensions: ["qss"] }] })) ?? null,
    pickOpen: async () => {
      const path = await open({ filters: [{ name: t("sheetSet.title"), extensions: ["qss"] }] });
      return typeof path === "string" ? path : null;
    },
    pickDrawing: async () => {
      const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
      return typeof path === "string" ? path : null;
    },
  };

  /** Opens every drawing of a sheet set, a tab each, one after another. */
  async function openAll(paths: string[]) {
    for (const path of paths) await openPath(path);
  }

  /** Shows the drawing of tab `id` (call inside `drawingChange`); false when that failed (the
   *  error shows). */
  async function activate(id: number): Promise<boolean> {
    let ok = false;
    await run(async () => {
      const gen = beginShow();
      const next = await activateTab(id);
      if (isStale(gen, switchGen.current)) return;
      // Work started while the core was switching (a zoom's glyphs) may have read the old drawing
      // under the new generation: bump it again now that the new one is in place.
      await showDrawing(next, beginShow());
      ok = true;
    });
    return ok;
  }

  /** Shows the drawing of tab `id`, after any drawing change already asked for. */
  const switchTab = (id: number) => drawingChange(() => activate(id));

  /** Shows the tab after (`step` 1) or before (-1) the one on screen, as the core has them. */
  const cycleTabs = (step: 1 | -1) =>
    drawingChange(async () => {
      const now = await readTabs().catch(() => null);
      const next = now && cycleTab(now.tabs, now.active, step);
      if (now && next !== null && next !== now.active) await activate(next);
    });

  /** Closes tab `id` (the one on screen when null). A drawing with unsaved changes is shown first
   *  and asks "Save changes to the drawing?"; Cancel keeps it open. */
  const closeFileTab = (id: number | null) =>
    drawingChange(async () => {
      const now = await readTabs().catch(() => null);
      const tab = now?.tabs.find((f) => f.id === (id ?? now.active));
      if (!now || !tab) return;
      try {
        if (tab.dirty && (!(await activate(tab.id)) || !(await leaveOpenDrawing()))) return;
      } catch (e) {
        setError(t("save.failed", { error: String(e) }));
        return;
      }
      await run(async () => {
        // On screen before the close: shown above when it had unsaved changes.
        const shown = tab.dirty ? tab.id : now.active;
        const left = await closeTab(tab.id, tab.dirty);
        setFileTabs(left.tabs);
        setActiveTab(left.active);
        if (left.active === null) showNoDrawing();
        else if (left.active !== shown) {
          const gen = beginShow();
          const next = await drawingInfo();
          if (!isStale(gen, switchGen.current)) await showDrawing(next, gen);
        }
      });
    });

  /** Writes the drawing with `write`, then shows its new layers and entities. */
  async function saveWith(write: () => Promise<void>) {
    const gen = switchGen.current;
    await run(async () => {
      let next: DrawingInfo | null = null;
      try {
        await write().catch((e) => {
          throw t("save.failed", { error: String(e) });
        });
      } finally {
        // Even a failed save may have written the drawing and moved to its new path.
        next = await drawingInfo().catch(() => null);
        if (next && !isStale(gen, switchGen.current)) setInfo(next);
      }
      // The save wrote the project file too: autosave can try again.
      autosaveOff.current = false;
      await redraw();
      setNotice(t("save.done", { path: next?.path ?? "" }));
    });
  }

  /** A drawing that opened with problems is rewritten with only what Qiyas could read, so a save
   *  over its own file asks first; the core keeps the original as a `.bak` either way. True to
   *  save. */
  async function confirmRecoveredSave(of: DrawingInfo): Promise<boolean> {
    // An xref's problems are not the drawing's own: saving never touches the xref. Nor is an
    // empty model something the reader lost.
    if (readProblems(of).length <= of.xrefs.reduce((n, x) => n + x.problems.length, 0)) return true;
    return ask(t("save.confirmRecovered"), { title: t("appName"), kind: "warning" });
  }

  /** Saves over the drawing's own file, after `confirmRecoveredSave`. */
  async function saveFile() {
    if (!info) return;
    if (!(await confirmRecoveredSave(info))) return;
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


  /** The palette's edit of the picked entity (a drawing's own, at the top of the sheet). */
  const setPickedProperty = useCallback(
    (prop: EntityProp, value: boolean | string | number | [number, number]) => {
      if (!pickedInfo) return;
      runLayerCommand({ type: "set_property", target: { handle: pickedInfo.handle, chain: [] }, prop, value });
    },
    [pickedInfo, info],
  );



  /** Sets the chosen image's brightness, contrast and fade (one undo step), then draws it. */
  async function setChosenDisplay(handle: number, display: ImageDisplay) {
    await runCommand({ type: "set_image_display", handle, ...display });
    await redraw();
  }


  /** A click in the drawing: the one a PDF or image insert point or a calibration waits for, else
   *  the tool's. */
  function clickDrawing(world: Point, screen: Point, worldPerPixel: number, shift: boolean) {
    // The drawing under the click may be about to go.
    if (switching.current > 0) return;
    lastPoint.current = world;
    if (pending?.kind === "fieldTarget") {
      // The entity at the top of the sheet a field reads.
      run(async () => {
        const picked = await pick(world[0], world[1], 6 * worldPerPixel);
        if (!picked) return;
        setFieldTarget({ handle: picked.top, kind: picked.kind ?? "" });
        setPending(null);
      });
      return;
    }
    if (!pendingPoint(world)) measure.onClick(world, screen, worldPerPixel, shift);
  }


  /** Escape: drops what is waiting (an insert point, a measure, a prompt) and every highlight.
   *  `escKey` for the Esc key itself (or ESC typed), which keeps a polyline drawn so far. */
  function cancelAll(escKey = false) {
    preset.current = null;
    setPending(null);
    setTextEditor(null);
    setInfoCard(null);
    if (escKey) measure.escape();
    else measure.cancel();
    setSelectedId(null);
    setBoqCode(null);
    setAiHighlight([]);
    setConflictIds([]);
  }

  /** Answers divide's number of parts or measure along's step (metres). */
  function answerAlong(value: number) {
    if (measure.asking === "divide") measure.answer(value);
    else if (measure.asking === "measureAlong" && info) measure.answer(value / metresPerUnit(info.units));
  }

  // DIVIDE n and MEASURE step: the number typed with the command answers once the entity is
  // clicked. It is used once, and dropped when the prompt closes or another tool is chosen, so it
  // never answers a later prompt.
  useEffect(() => {
    const typed = preset.current;
    preset.current = null;
    if (typed && measure.asking && typed.tool === tool) answerAlong(typed.value);
  }, [measure.asking]);
  useEffect(() => {
    if (preset.current?.tool !== tool) preset.current = null;
  }, [tool]);
  useEffect(() => {
    if (commandLineOn && focusCommandLine.current) commandInput.current?.focus();
    focusCommandLine.current = false;
  }, [commandLineOn]);

  /** The layer named `name` in any case, as the drawing spells it; null (and an error) when none. */
  function layerNamed(name: string): string | null {
    const found = info?.layers.find((l) => l.name.toUpperCase() === name.toUpperCase());
    if (!found) setError(t("cmd.noLayer", { name }));
    return found?.name ?? null;
  }

  /** Runs what the command line asks for, through the same handlers as the ribbon and keys. */
  function onCommand(a: Action) {
    setNotice(null);
    const needDrawing = () => {
      if (!info) setError(t("cmd.noDrawing"));
      return !info;
    };
    switch (a.type) {
      case "repeat":
        return;
      case "finish":
        measure.finish();
        return;
      case "cancel":
        cancelAll(true);
        return;
      case "point":
        if (switching.current > 0) return;
        lastPoint.current = a.p;
        if (!pendingPoint(a.p)) measure.typePoint(a.p);
        return;
      case "number":
        answerAlong(a.value);
        return;
      case "option":
        measure.drawInput({ type: "option", option: a.option });
        return;
      case "distance":
        measure.drawInput({ type: "number", value: a.value });
        return;
      case "draw":
        if (!needDrawing()) apply({ type: "draw", ...a.spec });
        return;
      case "tool":
        if (needDrawing()) return;
        // DIVIDE n while divide already asks for its number: that is the answer.
        if (a.preset !== undefined && a.tool === tool && measure.asking === tool) {
          answerAlong(a.preset);
          return;
        }
        preset.current = a.preset !== undefined ? { tool: a.tool, value: a.preset } : null;
        setTool(a.tool);
        return;
      case "count":
        if (!needDrawing()) apply({ type: "count_block", block_name: a.block, layer: null });
        return;
      case "layers": {
        if (needDrawing()) return;
        const names = a.names.map(layerNamed);
        if (names.includes(null)) return;
        const next = new Set(hidden);
        for (const n of names as string[]) {
          if (a.hide) next.add(n);
          else next.delete(n);
        }
        applyHidden(next);
        return;
      }
      case "isolate": {
        const name = needDrawing() ? null : layerNamed(a.name);
        if (name) isolateLayer(name);
        return;
      }
      case "showAll":
        if (!needDrawing()) showAll();
        return;
      case "zoom":
        if (a.how === "extents") viewport.current?.fitExtents();
        else viewport.current?.zoomBy(a.how === "in" ? 0.5 : 2);
        return;
      case "toggle":
        if (!needDrawing()) toggleDrafting(a.toggle);
        return;
      case "sheet": {
        if (needDrawing()) return;
        const want = a.name.toUpperCase();
        const found = info!.sheets.find(
          (s) => s.name.toUpperCase() === want || s.block.toUpperCase() === want,
        );
        if (found) chooseSheet(found.block);
        else setError(t("cmd.noSheet", { name: a.name }));
        return;
      }
      case "ai":
        setSideTab("assistant");
        assistant.send(a.text);
        return;
      case "run":
        runCommandLine(a.what);
    }
  }

  /** The command line's file, view and BOQ commands: what the ribbon's buttons do, when they
   *  are enabled. */
  function runCommandLine(what: Extract<Action, { type: "run" }>["what"]) {
    if (what === "open") {
      openFile();
      return;
    }
    // Both open drawings: none needs to be open.
    if (what === "recover") {
      recoverFile();
      return;
    }
    if (what === "sheetSet") {
      setSheetSetShown(true);
      return;
    }
    if (!info) {
      setError(t("cmd.noDrawing"));
      return;
    }
    switch (what) {
      case "save":
        return saveFile();
      case "saveAs":
        return saveFileAs();
      case "export":
        return exportFile();
      case "importBoq":
        return importBoqFile();
      case "compare":
        return compareNow();
      case "undo":
        return history(undo);
      case "redo":
        return history(redo);
      case "attachPdf":
        setPickedInsert(null);
        return setAttachOpen(true);
      case "attachImage":
        setPickedInsert(null);
        return setAttachImageOpen(true);
      case "xrefs":
        return setXrefsOpen(true);
      case "extract":
        return setExtractOpen(true);
      case "find":
        return setFindOpen(true);
      case "dimStyle":
        return openDimStyle();
      case "updateFields":
        return updateFields();
      case "insertBlock":
      case "writeBlock":
        return setBlockPaletteOpen(true);
      case "refClose":
        return closeBlockEdit(true);
      case "attributeManager":
        return openAttributeManager();
      case "layers":
        document.querySelector<HTMLElement>(".layers button, .layers input")?.focus();
        return;
      case "layerStates":
        return setLayerStatesOpen(true);
      case "layerTranslate":
        return setTranslatorOpen(true);
      case "vports":
        return setSplit(!split);
      case "vports1":
        return setSplit(false);
      case "vports2":
        return setSplit(true);
      case "ucsWorld":
        return apply({ type: "ucs", origin: [0, 0], x_axis: [1, 0] });
      case "vpLayer": {
        const box = document.querySelector<HTMLElement>(".layers .vp-freeze");
        if (box) box.focus();
        else setError(t("layer.vpNone"));
        return;
      }
      case "properties":
        document.querySelector<HTMLElement>(".properties h2 button")?.focus();
        return;
      case "calibrate":
        if (!batch || batch.imageTable.length === 0) {
          setError(t("cmd.notNow", { name: "CALIBRATE" }));
          return;
        }
        measure.cancel();
        setPending({ kind: "calibrate", first: null });
    }
  }

  // Keyboard: tool letters, Escape, Enter, undo, redo and save. Re-bound each render, so it sees fresh state.
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const action = shortcutFor(e);
      // Ctrl+9 works from the command line too.
      if (action === "commandLine") {
        e.preventDefault();
        if (e.repeat) return;
        focusCommandLine.current = !commandLineOn;
        setCommandLineOn(!commandLineOn);
        return;
      }
      // The file tabs, from anywhere: Ctrl+Tab and Ctrl+W type nothing.
      if (action === "nextTab" || action === "prevTab" || action === "closeTab") {
        e.preventDefault();
        if (e.repeat) return;
        // From the core's tabs at the time the change runs, never from what this render saw.
        if (action === "closeTab") closeFileTab(null);
        else cycleTabs(action === "nextTab" ? 1 : -1);
        return;
      }
      // A drawing change is under way: tools wait for the drawing it brings.
      if (switching.current > 0) return;
      if (typing(e)) return;
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
        cancelAll(true);
      } else if (action === "finish") {
        // Enter on a focused button presses it; don't also finish the measure.
        if (e.target instanceof HTMLButtonElement) return;
        measure.finish();
      } else if (action && !e.repeat) {
        // Text selected in a panel: Ctrl+C copies it, as anywhere else.
        const text = window.getSelection()?.toString() ?? "";
        if ((action === "copyclip" || action === "copybase") && text !== "") return;
        chooseTool(action);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  // A modify tool left dimensions or tables out of what was picked: say so.
  useEffect(() => {
    if (measure.skipped > 0) setNotice(t("modify.skipped", { n: measure.skipped }));
  }, [measure.skipped]);

  // An undo or a redo may take selected entities away: keep only those still drawn.
  useEffect(() => {
    if (!batch || measure.selection.length === 0) return;
    const drawn = new Set<number>();
    for (let i = 0; i < batch.ranges.length; i++) drawn.add(Number(batch.ranges.top(i)));
    measure.prune((h) => drawn.has(h));
  }, [batch]);

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

  // The select tool's pick, for the properties palette; read again when the batch changes (an
  // undo, a redo, another sheet).
  const pickedTarget = tool === "select" ? measure.highlighted : null;
  const pickedKey = pickedTarget ? `${pickedTarget.handle}:${pickedTarget.chain.join(",")}` : null;
  useEffect(() => {
    if (!pickedTarget) {
      setPickedInfo(null);
      return;
    }
    let gone = false;
    entityInfo(pickedTarget.handle, pickedTarget.chain)
      .then((found) => !gone && setPickedInfo(found))
      .catch(() => !gone && setPickedInfo(null));
    return () => {
      gone = true;
    };
  }, [pickedKey, batch]);

  const highlight = useMemo(() => {
    const rows = [...(selected ? [selected] : []), ...boqLinked, ...conflictLinked];
    const targets: Target[] = [
      ...rows.flatMap(measurementTargets),
      ...aiHighlight.map((handle) => ({ handle, chain: [] })),
    ];
    if (measure.highlighted !== null) targets.push(measure.highlighted);
    for (const handle of measure.selection) targets.push({ handle, chain: [] });
    return targets;
  }, [selected, boqLinked, conflictLinked, measure.highlighted, measure.selection, aiHighlight]);

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
      measure.preview && measure.preview.length > 0
        ? measure.preview
        : measure.points.length > 0
          ? overlaySegments(measure.points, measure.cursor, tool)
          : null,
    [measure.preview, measure.points, measure.cursor, tool],
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
  // An empty model is said next to the file name, not counted as a reading problem.
  const problems = info ? readProblems(info) : [];
  const emptyModel = info?.problems.includes(EMPTY_MODEL) ?? false;
  const message = opening
    ? t("opening")
    : pending
      ? t(
          pending.kind === "insert"
            ? pending.of === "image"
              ? "attachImage.pickHint"
              : "attachPdf.pickHint"
            : pending.kind === "fieldTarget"
              ? "insertField.pickHint"
              : pending.first
              ? "calibrate.second"
              : "calibrate.first",
        )
      : renderProgress !== null && !attachOpen
        ? t("pdf.drawing", { n: renderProgress })
        : measure.hint
      ? // A notice (what a modify left out) goes beside the tool's prompt.
        t(measure.hint, measure.hintArgs ?? undefined) + (notice ? ` — ${notice}` : "")
      : (notice ??
        (info
          ? emptyModel
            ? `${fileName} — ${t("problem.emptyModel")}`
            : fileName
          : t("noDrawing")));
  // What the command line waits for, and its prompt.
  let expect: Expect = "command";
  let promptKey = "cmd.prompt.command";
  if (pending) {
    expect = "point";
    promptKey =
      pending.kind === "insert"
        ? "cmd.prompt.insert"
        : pending.kind === "fieldTarget"
          ? "cmd.prompt.select"
          : pending.first
          ? "cmd.prompt.calibrateSecond"
          : "cmd.prompt.calibrateFirst";
  } else if (measure.asking) {
    expect = measure.asking === "divide" ? "count" : "step";
    promptKey = `cmd.prompt.${expect}`;
  } else if (measure.draw || measure.modify || measure.edit || measure.text || measure.dim) {
    expect = "point";
    promptKey = measure.hint;
  } else if (tool === "distance" || tool === "area") {
    expect = "point";
    promptKey = measure.points.length > 0 ? "cmd.prompt.nextPoint" : "cmd.prompt.firstPoint";
  } else if (tool === "idPoint") {
    expect = "point";
    promptKey = "cmd.prompt.point";
  } else if (tool === "zoomWindow") {
    expect = "point";
    promptKey = measure.corner ? "cmd.prompt.otherCorner" : "cmd.prompt.firstCorner";
  } else if (tool === "pasteclip" || tool === "pasteblock" || tool === "mview" || tool === "ucs") {
    expect = "point";
    promptKey = measure.hint;
  } else if (tool === "ucsObject") {
    promptKey = measure.hint;
  } else if (
    tool === "count" ||
    tool === "geometry" ||
    tool === "divide" ||
    tool === "measureAlong" ||
    tool === "textEdit"
  ) {
    promptKey = "cmd.prompt.select";
  }
  const commandState: ParseState = {
    expect,
    last: measure.lastDrawPoint ?? measure.points[measure.points.length - 1] ?? lastPoint.current,
    aliases: commandAliases,
    options: measure.drawOptions,
    numberToo: measure.takesNumber,
    ucs: info?.ucs,
  };
  const sheetShown = info?.sheets.find((s) => s.block.toLowerCase() === sheet.toLowerCase());
  const sheetName = sheetShown?.name ?? sheet;
  // Named views are of model space.
  const onModel = sheetShown?.is_model ?? false;
  // Unknown units are measured as millimetres; the status bar says so.
  const units = info ? t(`unit.${info.units === "unknown" ? "millimetres" : info.units}`) : "";

  return (
    <div className="app">
      <header className="top">
        <Ribbon
          active={tool}
          onTool={chooseTool}
          onOpen={openFile}
          loadRecents={readRecents}
          onOpenRecent={openPath}
          onClearRecents={clearRecents}
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
                  measure.cancel();
                  setPending({ kind: "calibrate", first: null });
                  focusDrawing();
                }
              : undefined
          }
          onExtract={info ? () => setExtractOpen(true) : undefined}
          onFind={info ? () => setFindOpen(true) : undefined}
          onDimStyle={info ? openDimStyle : undefined}
          onUpdateFields={info ? updateFields : undefined}
          onInsertBlock={info ? () => setBlockPaletteOpen(true) : undefined}
          onWriteBlock={info ? () => setBlockPaletteOpen(true) : undefined}
          onAttributeManager={info ? openAttributeManager : undefined}
          onDesignCenter={info ? openDesignCenter : undefined}
          onToolPalettes={info ? openToolPalettes : undefined}
          onLayerStates={info ? () => setLayerStatesOpen(true) : undefined}
          onTranslateLayers={info ? () => setTranslatorOpen(true) : undefined}
          groupSelect={groupSelect}
          onGroupSelect={setGroupSelect}
          split={split}
          onSplit={info ? setSplit : undefined}
          onUcsWorld={info ? () => apply({ type: "ucs", origin: [0, 0], x_axis: [1, 0] }) : undefined}
          onSheetSet={() => setSheetSetShown(true)}
          onRecover={recoverFile}
          arabicReversed={arabicReversed}
          onArabicReversed={info ? chooseArabicReversed : undefined}
          {...(onModel && {
            loadViews: readViews,
            onGoToView: (v: NamedView) => {
              viewport.current?.fitTo(v.bounds);
              focusDrawing();
            },
            onSaveView: saveView,
            onDeleteView: deleteView,
          })}
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
      {findOpen && info && (
        <FindReplaceDialog
          onFind={findTexts}
          onReplaceAll={replaceTexts}
          onShow={(m) => run(() => showMatch(m))}
          onClose={() => setFindOpen(false)}
        />
      )}
      {textDialog?.kind === "mtext" && info && (
        <TextDialog
          heading="text.newMtext"
          value=""
          multiline
          styles={info.text_styles}
          style={currentTextStyle}
          height={textDialog.request.height ?? currentTextHeight}
          onSubmit={async (d) => {
            const r = textDialog.request;
            const { height, value, style } = d;
            await runText({ type: "mtext", at: r.at, width: r.width, height, value, style });
            setTextHeight(d.height);
            setTextStyle(d.style);
          }}
          onClose={() => {
            setTextDialog(null);
            focusDrawing();
          }}
        />
      )}
      {textDialog?.kind === "edit" && (
        <TextDialog
          heading="text.editTitle"
          value={textDialog.value}
          multiline={textDialog.multiline}
          onSubmit={async (d) => {
            // Unchanged: nothing to do (an MText keeps its formatting).
            if (d.value === textDialog.value) return;
            await runText({
              type: "edit_text",
              target: { handle: textDialog.handle, chain: [] },
              value: d.value,
              tag: null,
            });
          }}
          onClose={() => {
            setTextDialog(null);
            focusDrawing();
          }}
        />
      )}
      {groupAsk && (
        <GroupNameDialog
          names={info?.groups?.map((g) => g.name) ?? []}
          onSubmit={async (name) => {
            await runDrawingCommand({ ...groupAsk, name });
          }}
          onClose={() => {
            setGroupAsk(null);
            focusDrawing();
          }}
        />
      )}
      {layerStatesOpen && info && (
        <LayerStatesDialog
          states={info.layer_states ?? []}
          onSave={async (name) => {
            await runDrawingCommand({ type: "save_layer_state", name });
          }}
          onRestore={async (name) => {
            await runDrawingCommand({ type: "restore_layer_state", name });
          }}
          onRename={async (from, to) => {
            await runDrawingCommand({ type: "rename_layer_state", from, to });
          }}
          onDelete={async (name) => {
            await runDrawingCommand({ type: "delete_layer_state", name });
          }}
          onClose={() => {
            setLayerStatesOpen(false);
            focusDrawing();
          }}
        />
      )}
      {translatorOpen && info && (
        <LayerTranslator
          layers={info.layers}
          loadStandard={loadLayerStandard}
          onTranslate={async (from_path, mappings) => {
            await runDrawingCommand({ type: "translate_layers", from_path, mappings });
          }}
          onClose={() => {
            setTranslatorOpen(false);
            focusDrawing();
          }}
        />
      )}
      {createBlockAsk && (
        <CreateBlockDialog
          names={createBlockAsk.names}
          onSubmit={async (name, convert) => {
            const outcome = await runBlock({ ...createBlockAsk.cmd, name, convert_to_insert: convert });
            if (outcome.type === "edited" && outcome.unlinked > 0) {
              setNotice(t("block.unlinked", { n: outcome.unlinked }));
            }
          }}
          onClose={() => {
            setCreateBlockAsk(null);
            focusDrawing();
          }}
        />
      )}
      {attributesAsk && (
        <AttributesDialog
          heading={attributesAsk.heading}
          fields={attributesAsk.fields}
          onSubmit={async (values) => {
            const cmd = attributesAsk.command(values);
            if (cmd) await runBlock(cmd);
          }}
          onClose={() => {
            setAttributesAsk(null);
            focusDrawing();
          }}
        />
      )}
      {attDefAt && (
        <AttDefDialog
          height={attDefAt.height ?? currentTextHeight}
          onSubmit={async (d) => {
            setTextHeight(d.height);
            const { tag, prompt, height } = d;
            await runBlock({ type: "att_def", tag, prompt, default: d.default, at: attDefAt.at, height });
          }}
          onClose={() => {
            setAttDefAt(null);
            focusDrawing();
          }}
        />
      )}
      {blockPaletteOpen && info && (
        <BlockPalette
          load={readBlocks}
          thumbnail={blockPicture}
          onInsert={(block) => startInsert(block)}
          onWrite={writeBlockFile}
          onAddToPalette={addToPalette}
          onClose={closeBlockPalette}
        />
      )}
      {designCenter && info && (
        <DesignCenter
          choose={async () => {
            const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
            return typeof path === "string" ? path : null;
          }}
          list={drawingBlocks}
          has={(name) => designCenter.includes(name.toUpperCase())}
          onInsert={(block, path) => {
            // A block this drawing has is inserted as this drawing defines it.
            const want = block.name.toUpperCase();
            if (!designCenter.includes(want)) {
              startInsert(block, path);
              return;
            }
            run(async () => {
              const own = (await readBlocks()).find((b) => b.name.toUpperCase() === want);
              startInsert(own ?? block);
            });
          }}
          onRedefine={async (name, path) => {
            await runBlock({ type: "redefine_block", name, from_path: path });
            setNotice(t("block.redefined", { name }));
          }}
          onClose={() => {
            setDesignCenter(null);
            focusDrawing();
          }}
        />
      )}
      {palette && info && (
        <ToolPalettes
          items={palette}
          onRun={runPaletteItem}
          onSave={setToolPalette}
          onClose={() => {
            setPalette(null);
            focusDrawing();
          }}
        />
      )}
      {managerBlocks && (
        <AttributeManager
          blocks={managerBlocks}
          onSave={async (block, tag, prompt, value) => {
            await runBlock({ type: "edit_att_def", block, tag, prompt, default: value });
          }}
          onSync={async (block) => {
            const outcome = await runBlock({ type: "sync_attributes", block });
            const locked = outcome.type === "updated" ? outcome.skipped : 0;
            const notes = [t("block.synced")];
            if (locked > 0) notes.push(t("locked.skipped", { n: locked }));
            setNotice(notes.join(" · "));
          }}
          onClose={() => {
            setManagerBlocks(null);
            focusDrawing();
          }}
        />
      )}
      {dimStyleOpen && info && (
        <DimStylePanel
          styles={info.dim_styles}
          current={info.dim_style}
          annotationScale={info.annotation_scale}
          onSave={(name, fields, current) => runDimStyle({ type: "set_dim_style", name, fields, current })}
          onScale={(name) => runDimStyle({ type: "annotation_scale", name })}
          onClose={() => {
            setDimStyleOpen(false);
            focusDrawing();
          }}
        />
      )}
      {(dimText?.kind === "leader" || dimText?.kind === "mleader" || dimText?.kind === "override") && (
        <TextDialog
          heading={dimText.kind === "override" ? "dim.text.override" : "dim.text.leader"}
          value={dimText.kind === "override" ? "<>" : ""}
          multiline={dimText.kind !== "override"}
          onSubmit={async (d) => {
            if (dimText.kind === "override") {
              const target = { handle: dimText.handle, chain: [] };
              await runText({ type: "dim_edit", target, op: { text_override: d.value } });
            } else {
              await runText({ type: dimText.kind, points: dimText.points, text: d.value });
            }
          }}
          onClose={() => {
            setDimText(null);
            focusDrawing();
          }}
        />
      )}
      {newTableAt && (
        <NewTableDialog
          textHeight={currentTextHeight}
          onSubmit={(table) => runText({ type: "table", at: newTableAt, ...table, data: [] })}
          onClose={() => {
            setNewTableAt(null);
            focusDrawing();
          }}
        />
      )}
      {tableEditing && (
        <TableEditor
          key={tableEditing.handle}
          handle={tableEditing.handle}
          table={tableEditing.table}
          cell={tableEditing.cell}
          picking={pending?.kind === "fieldTarget"}
          onRun={runTable}
          onField={(cell) => {
            setFieldTarget(null);
            setFieldAsk({ mode: "cell", cell });
          }}
          onClose={() => {
            setTableEditing(null);
            focusDrawing();
          }}
        />
      )}
      {fieldAsk && (
        <FieldDialog
          mode={fieldAsk.mode}
          target={fieldTarget}
          picking={pending?.kind === "fieldTarget"}
          onPick={() => setPending({ kind: "fieldTarget" })}
          onSubmit={insertField}
          onClose={() => {
            setFieldAsk(null);
            setPending(null);
            if (!tableEditing) focusDrawing();
          }}
        />
      )}
      {dimText?.kind === "tolerance" && (
        <ToleranceDialog
          onSubmit={(frame) => runText({ type: "tolerance", at: dimText.at, frame })}
          onClose={() => {
            setDimText(null);
            focusDrawing();
          }}
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
            loadAliases();
          }}
        />
      )}
      <div className="middle">
        <div className="left">
          <LayersPanel
            layers={info?.layers ?? []}
            hidden={hidden}
            theme={theme}
            busy={layersBusy}
            onToggle={toggleLayer}
            onIsolate={isolateLayer}
            onShowAll={showAll}
            current={info?.current.layer}
            onMakeCurrent={info ? makeCurrent : undefined}
            linetypes={info?.linetypes}
            onSetLayer={info ? setLayerProp : undefined}
            onRenameLayer={info ? renameLayer : undefined}
            onDeleteLayer={info ? deleteLayer : undefined}
            onNewLayer={info ? newLayer : undefined}
            onLayerStates={info ? openLayerStates : undefined}
            onTranslate={info ? openTranslator : undefined}
            viewports={sheetShown?.is_model ? NO_VIEWPORTS : info?.viewports}
            onViewportFrozen={info ? setViewportFrozen : undefined}
          />
          <PropertiesPanel
            entity={pickedInfo}
            units={info?.units ?? "metres"}
            precision={drafting?.precision ?? 3}
            sheet={sheetName}
            layers={ownLayers}
            linetypes={info?.linetypes}
            onSet={info && pickedInfo ? setPickedProperty : undefined}
            current={info?.current}
            onSetCurrent={info ? setCurrentState : undefined}
          />
        </div>
        <main className="work">
          {info?.block_edit && (
            <BlockEditBar
              name={info.block_edit}
              onSave={() => closeBlockEdit(true)}
              onDiscard={() => closeBlockEdit(false)}
            />
          )}
          <FileTabs
            tabs={fileTabs}
            active={activeTab}
            onSelect={(id) => id !== activeTab && switchTab(id)}
            onClose={(id) => closeFileTab(id)}
          />
          <div className="views">
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
                boxes={measure.boxes && !pending}
                onBox={(bounds, crossing, shift) => {
                  if (switching.current === 0) measure.onBox(bounds, crossing, shift);
                }}
                grid={drafting?.grid ? drafting.grid_spacing : null}
                ucs={info?.ucs}
                inset={inset}
                onMove={measure.onMove}
                onWindowZoomed={measure.cancel}
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
                    measure.cancel();
                    focusDrawing();
                  }}
                />
              )}
              {textEditor && info && (
                <InPlaceText
                  key={textEditor.key}
                  at={textEditor.at}
                  styles={info.text_styles}
                  style={currentTextStyle}
                  height={textEditor.request.height ?? currentTextHeight}
                  onSubmit={(d) => placeInPlace(textEditor.request, d)}
                  onCancel={() => {
                    setTextEditor(null);
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
            {split && info && (
              <SplitPane
                sheets={info.sheets}
                mainSheet={sheet}
                mainBatch={batch}
                mainInset={inset}
                atlas={atlas}
                theme={theme}
                ucs={info.ucs}
                syncAtlas={syncAtlas}
                onError={(e) => setError(String(e))}
                onClose={() => setSplit(false)}
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
      {commandLineOn && (
        <CommandLine
          state={commandState}
          prompt={t(promptKey, promptKey === measure.hint ? (measure.hintArgs ?? undefined) : undefined)}
          onAction={onCommand}
          inputRef={commandInput}
        />
      )}
      <StatusBar
        cursor={measure.cursor}
        precision={drafting?.precision ?? 3}
        metresPerUnit={info ? metresPerUnit(info.units) : 1}
        snapLabel={measure.snap ? t(`snap.${measure.snap.kind}`) : null}
        drafting={info ? drafting : null}
        onToggle={toggleDrafting}
        onDraftingSettings={() => setDraftingOpen(true)}
        units={units}
        unitsWarning={info?.units === "unknown"}
        problems={problems}
        message={message}
        ucs={info?.ucs}
      />
      {recovery && (
        <RecoveryDialog
          key={recovery.id}
          items={recovery.items}
          onRecover={recoverFrom}
          onDiscard={discardFrom}
          onClose={() => {
            const then = recovery.then;
            setRecoveries((q) => q.filter((r) => r.id !== recovery.id));
            if (then) openPath(then, false);
          }}
        />
      )}
      {recoverReport && (
        <RecoverReportDialog
          path={recoverReport.path}
          report={recoverReport.report}
          onClose={() => setRecoverReport(null)}
        />
      )}
      {sheetSetShown && (
        <SheetSetManager api={sheetSetApi} onOpenAll={openAll} onClose={() => setSheetSetShown(false)} />
      )}
    </div>
  );
}

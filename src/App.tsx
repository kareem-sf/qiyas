import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, message as messageDialog, open, save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  activateTab,
  aiUndo,
  aliases as readAliases,
  blocks as readBlocks,
  blockThumbnail,
  clearRecents,
  drawingBlocks,
  drawingLayers,
  getSettings,
  setToolPalette,
  writeBlock,
  closeTab,
  EMPTY_MODEL,
  recents as readRecents,
  tabs as readTabs,
  views as readViews,
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
  tableCellAt,
  getDrafting,
  setDrafting,
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
  atlasPages,
  batchVersion,
  boundaryAt,
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
  autosaveDrawings,
  discardAutosave,
  paperViews,
  recoverableDrawings,
  recoverAutosave,
  recoverDrawing,
  sheetBatch,
  sheetBatchVersion,
  sheetSetEntry,
  sheetSetFromTabs,
  sheetSetOpen,
  sheetSetSave,
  type PaperView,
  type ReadReport,
  type AiUi,
  type BlockInfo,
  type Boq,
  type Command,
  type Conflict,
  type Current,
  type Drafting,
  type DrawingInfo,
  type EntityFilter,
  type EntityInfo,
  type EntityProp,
  type FieldKind,
  type Field,
  type ImageDisplay,
  type LayerProp,
  type ViewportInfo,
  type TableInfo,
  type Measurement,
  type NamedView,
  type PaletteItem,
  type TabInfo,
  type TextMatch,
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
import {
  AttachPdfDialog,
  CalibrateDialog,
  metresPerUnit,
  type AttachRequest,
} from "./shell/AttachPdfDialog";
import { blurAfterClick } from "./shell/blurAfterClick";
import {
  AttDefDialog,
  AttributeManager,
  AttributesDialog,
  BlockEditBar,
  CreateBlockDialog,
  promptsFor,
  type AttributeField,
} from "./shell/BlockDialogs";
import { BlockPalette } from "./shell/BlockPalette";
import { CommandLine } from "./shell/CommandLine";
import { DraftingSettings } from "./shell/DraftingSettings";
import { ExtractDialog } from "./shell/ExtractDialog";
import { FindReplaceDialog, type FindQuery } from "./shell/FindReplaceDialog";
import { cycleTab, FileTabs } from "./shell/FileTabs";
import { isStale, serial } from "./shell/switcher";
import { AlongPrompt, InfoCard, type Info } from "./shell/InfoCard";
import { GroupNameDialog, LayerStatesDialog } from "./shell/LayerDialogs";
import { LayersPanel } from "./shell/LayersPanel";
import { LayerTranslator } from "./shell/LayerTranslator";
import { PropertiesPanel } from "./shell/PropertiesPanel";
import { RecoverReportDialog, RecoveryDialog } from "./shell/RecoveryDialog";
import { drop, enqueue, type RecoveryAsk } from "./shell/recoveryQueue";
import { SheetSetManager, type SheetSetApi } from "./shell/SheetSetManager";
import { Ribbon, type Tool } from "./shell/Ribbon";
import { SettingsDialog } from "./shell/SettingsDialog";
import { DimStylePanel } from "./shell/DimStylePanel";
import { DesignCenter } from "./shell/DesignCenter";
import { ToolPalettes } from "./shell/ToolPalettes";
import { InPlaceText, TextDialog, type TextDraft } from "./shell/TextEditor";
import { ToleranceDialog } from "./shell/ToleranceDialog";
import { NewTableDialog, TableEditor } from "./shell/TableEditor";
import { FieldDialog, fieldCode } from "./shell/FieldDialog";
import { SheetTabs } from "./shell/SheetTabs";
import { StatusBar } from "./shell/StatusBar";
import { TakeoffPanel } from "./shell/TakeoffPanel";
import { XrefManager } from "./shell/XrefManager";
import { leaveDrawing, readChoice } from "./shell/unsaved";
import { useAutoClear } from "./shell/useAutoClear";
import {
  handlesBounds,
  markSize,
  matchBounds,
  measurementBounds,
  measurementTargets,
  overlaySegments,
  pointMarks,
} from "./tools/overlay";
import { mergeAliases } from "./tools/aliases";
import { parse, type Action, type Expect, type ParseState } from "./tools/commandParser";
import { shortcutFor, type DraftingToggle } from "./tools/shortcuts";
import type { DimRequest } from "./tools/dimTools";
import type { TextRequest } from "./tools/textTools";
import { useTool } from "./tools/useTool";
import {
  parseAtlas,
  parseBatch,
  fetchPictures,
  newPictureBounds,
  parseThumbnails,
  type AtlasPage,
  type Batch,
  type HeldPicture,
  type Target,
} from "./viewport/batch";
import type { Bounds, Point } from "./viewport/camera";
import type { Theme } from "./viewport/gl";
import { refreshSheet, type HeldSheet } from "./viewport/inset";
import { SplitPane } from "./viewport/SplitPane";
import { Viewport, type ViewportHandle } from "./viewport/Viewport";

const THEME_KEY = "qiyas.theme";

const MODEL = "*Model_Space";

/** The drafting setting each switch turns on and off. */
const TOGGLED: Record<DraftingToggle, "osnap" | "grid" | "ortho" | "polar"> = {
  osnap: "osnap",
  grid: "grid",
  ortho: "ortho",
  polar: "polar",
};

/** A click in the drawing the window is waiting for instead of the tool's: the insert point of a
 *  PDF page or an image, or the two points of a calibration. */
type Pending =
  | { kind: "insert"; of: "pdf" | "image" }
  | { kind: "calibrate"; first: Point | null }
  /** The entity a field reads: a click in the drawing picks it. */
  | { kind: "fieldTarget" };

/** Focus is in a text field: keys are typing, not shortcuts. */
const typing = (e: KeyboardEvent) =>
  e.target instanceof HTMLElement &&
  (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));

/** The drawing's reading problems: all but the empty-model warning, which is said on its own. */
const readProblems = (info: DrawingInfo) => info.problems.filter((p) => p !== EMPTY_MODEL);

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

/** No viewports: the layers panel's VP freeze column hides (one array, so the memo holds). */
const NO_VIEWPORTS: ViewportInfo[] = [];

export default function App() {
  const t = useT();
  const { lang, setLang } = useLanguage();
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [info, setInfo] = useState<DrawingInfo | null>(null);
  /** The open drawings' file tabs, and the one on screen. */
  const [fileTabs, setFileTabs] = useState<TabInfo[]>([]);
  const [activeTab, setActiveTab] = useState<number | null>(null);
  const [batch, setBatch] = useState<Batch | null>(null);
  /** The batch in hand and the core's version of it (`batchVersion`), so a redraw that changed
   *  nothing on the sheet doesn't fetch the whole batch again. */
  const batchHeld = useRef<Batch | null>(null);
  const batchVersionRef = useRef<number | null>(null);
  /** On a layout: model space drawn inside its viewports, and the core's version of it. */
  const [inset, setInset] = useState<{ batch: Batch; views: PaperView[] } | null>(null);
  const insetHeld = useRef<HeldSheet | null>(null);
  /** VPORTS: a second view beside the main one. */
  const [split, setSplit] = useState(false);
  /** Autosaves to offer back (on launch, or before opening `then`), one question at a time. */
  const [recoveries, setRecoveries] = useState<RecoveryAsk[]>([]);
  const recovery = recoveries[0] ?? null;
  /** What RECOVER read and dropped from a damaged drawing. */
  const [recoverReport, setRecoverReport] = useState<{ path: string; report: ReadReport } | null>(null);
  const [sheetSetShown, setSheetSetShown] = useState(false);
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
  const [findOpen, setFindOpen] = useState(false);
  const [dimStyleOpen, setDimStyleOpen] = useState(false);
  /** What an annotate tool asked the user to type: a leader's text, a tolerance, a dimension's. */
  const [dimText, setDimText] = useState<DimRequest | null>(null);
  /** TABLE's click: where the new table's top left corner goes. */
  const [newTableAt, setNewTableAt] = useState<Point | null>(null);
  /** The table being edited: its handle, its cells as they are now, and the cell chosen first. */
  const [tableEditing, setTableEditing] = useState<{
    handle: number;
    table: TableInfo;
    cell: [number, number];
  } | null>(null);
  /** The field dialog: into a new text at a click, or into a cell of the table being edited; and
   *  the entity an area or a length field reads, once picked. */
  const [fieldAsk, setFieldAsk] = useState<
    { mode: "text"; at: Point; height: number | null } | { mode: "cell"; cell: [number, number] } | null
  >(null);
  const [fieldTarget, setFieldTarget] = useState<{ handle: number; kind: string } | null>(null);
  /** BLOCK's base point is clicked: the command waiting for the new block's name, and the names
   *  taken. */
  const [createBlockAsk, setCreateBlockAsk] = useState<{
    cmd: Extract<Command, { type: "create_block" }>;
    names: string[];
  } | null>(null);
  /** The block INSERT places, and the drawing its definition comes from when this one lacks it. */
  const [insertChoice, setInsertChoice] = useState<{ block: BlockInfo; fromPath: string | null } | null>(null);
  /** Attribute values asked for (INSERT's prompts, ATTEDIT): the command they make, if any. */
  const [attributesAsk, setAttributesAsk] = useState<{
    heading: string;
    fields: AttributeField[];
    command(values: Record<string, string>): Command | null;
  } | null>(null);
  /** ATTDEF's click, waiting for the definition. */
  const [attDefAt, setAttDefAt] = useState<{ at: Point; height: number | null } | null>(null);
  const [blockPaletteOpen, setBlockPaletteOpen] = useState(false);
  /** The Design Center is open; the drawing's own block names (upper case). */
  const [designCenter, setDesignCenter] = useState<string[] | null>(null);
  /** The tool palette is open, with its buttons. */
  const [palette, setPalette] = useState<PaletteItem[] | null>(null);
  /** BATTMAN is open on these blocks. */
  const [managerBlocks, setManagerBlocks] = useState<BlockInfo[] | null>(null);
  /** LAYSTATE's and LAYTRANS's dialogs are open. */
  const [layerStatesOpen, setLayerStatesOpen] = useState(false);
  const [translatorOpen, setTranslatorOpen] = useState(false);
  /** GROUP's entities are picked: the command waiting for the group's name. */
  const [groupAsk, setGroupAsk] = useState<Extract<Command, { type: "create_group" }> | null>(null);
  /** Clicking a grouped entity selects its group (AutoCAD's PICKSTYLE); Ctrl picks one alone. */
  const [groupSelect, setGroupSelect] = useState(true);
  /** The block palette's thumbnails, revoked when it closes. */
  const thumbUrls = useRef<string[]>([]);
  /** The text height and style new text takes: the last ones used; null until then (2.5 mm in
   *  drawing units, and the drawing's current style). */
  const [textHeight, setTextHeight] = useState<number | null>(null);
  const [textStyle, setTextStyle] = useState<string | null>(null);
  /** TEXT's in-place editor: where the text goes, and where that is on the canvas. */
  const [textEditor, setTextEditor] = useState<{
    request: Extract<TextRequest, { kind: "text" }>;
    at: Point;
    key: number;
  } | null>(null);
  /** The MText dialog (a new MText's box), or the text edit dialog (an entity's text). */
  const [textDialog, setTextDialog] = useState<
    | { kind: "mtext"; request: Extract<TextRequest, { kind: "mtext" }> }
    | { kind: "edit"; handle: number; value: string; multiline: boolean }
    | null
  >(null);
  /** The command line shows (Ctrl+9), its input, and whether to focus it once shown. */
  const [commandLineOn, setCommandLineOn] = useState(true);
  const commandInput = useRef<HTMLInputElement>(null);
  const focusCommandLine = useRef(false);
  /** The user's command line aliases from the settings. */
  const [userAliases, setUserAliases] = useState<Record<string, string>>({});
  const commandAliases = useMemo(() => mergeAliases(userAliases), [userAliases]);
  /** The last point clicked or typed, for the command line's `@`. */
  const lastPoint = useRef<Point | null>(null);
  /** DIVIDE n or MEASURE step typed on the command line: the answer once the entity is clicked. */
  const preset = useRef<{ tool: Tool; value: number } | null>(null);
  /** What the select tool picked, for the properties palette. */
  const [pickedInfo, setPickedInfo] = useState<EntityInfo | null>(null);
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
  /** The find match the next fit shows (Find next on another sheet). */
  const fitMatch = useRef<number | null>(null);
  /** True while a layer change talks to the core; clicks meanwhile are dropped (the panel shows its
   *  buttons waiting, `layersBusy`), so they can't interleave. */
  const layerBusy = useRef(false);
  /** The same, for the layers panel to show: its buttons wait and the cursor says so. */
  const [layersBusy, setLayersBusy] = useState(false);
  /** Set when autosave failed for this drawing, so the error shows once, not after every click. A new
   *  open or a successful save turns autosave back on. */
  const autosaveOff = useRef(false);
  /** Bumped whenever another drawing comes on screen (open, tab switch, close): work started
   *  before is stale (`isStale`) and drops its results. */
  const switchGen = useRef(0);
  /** Drawing changes waiting or running; while any is, clicks and tool input on the canvas are
   *  ignored (they would land on a drawing about to go). */
  const switching = useRef(0);
  /** Opens, switches and closes reach the core one at a time, in the order asked. */
  const inTurn = useRef(serial()).current;
  /** The drawing on screen now, for checks inside queued work. */
  const infoRef = useRef(info);
  infoRef.current = info;

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  // Space pans the drawing; a button the mouse just clicked must not take it as a press.
  useEffect(blurAfterClick, []);
  // A notice goes with the next command (`run`), or on its own after a few seconds.
  useAutoClear(notice, () => setNotice(null));

  /** Reads the user's command line aliases (at start and after the settings close). */
  const loadAliases = () =>
    readAliases()
      .then(setUserAliases)
      .catch(() => {});
  useEffect(() => {
    loadAliases();
  }, []);

  // Runs after the viewport has taken the new batch (child effects run first).
  useEffect(() => {
    if (!fitNext.current) return;
    fitNext.current = false;
    const ids = fitIds.current;
    fitIds.current = null;
    // A find's match on the sheet just shown wins over the take-off rows and the whole sheet.
    const match = fitMatch.current;
    fitMatch.current = null;
    const box =
      match !== null ? matchBounds(match, batch) : ids && unionBounds(items.filter((m) => ids.includes(m.id)), batch);
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
    const gen = switchGen.current;
    const [nextItems, nextTotals, nextBoq, nextConflicts] = await Promise.all([
      takeoff(),
      totals(),
      readBoq(),
      readConflicts(),
    ]);
    if (isStale(gen, switchGen.current)) return;
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
  async function apply(given: Command) {
    // Plain text pastes as a text as tall as new text is.
    const cmd: Command =
      given.type === "paste_from_clipboard" ? { ...given, height: currentTextHeight } : given;
    // BLOCK's base point is clicked: the name comes from its dialog.
    if (cmd.type === "create_block" && cmd.name === "") {
      await run(async () => setCreateBlockAsk({ cmd, names: (await readBlocks()).map((b) => b.name) }));
      return;
    }
    // GROUP's entities are picked: the name comes from its dialog.
    if (cmd.type === "create_group" && cmd.name === "") {
      setGroupAsk(cmd);
      return;
    }
    // A copy goes onto the system clipboard and changes nothing; back to Select.
    if (cmd.type === "copy_to_clipboard") {
      await run(async () => {
        const outcome = await runCommand(cmd);
        if (outcome.type === "clipboard") setNotice(t("clip.copied", { n: outcome.count }));
        chooseTool("select");
      });
      return;
    }
    // The UCS is view state: the info carries it to the cursor, typed points and the grid; the
    // project file keeps it.
    if (cmd.type === "ucs" || cmd.type === "ucs_object") {
      let set = false;
      await run(async () => {
        await runCommand(cmd);
        await syncInfo();
        set = true;
        const world = cmd.type === "ucs" && cmd.origin[0] === 0 && cmd.origin[1] === 0 && cmd.x_axis[0] > 0 && cmd.x_axis[1] === 0;
        setNotice(t(world ? "ucs.worldSet" : "ucs.set"));
      });
      if (set) await autosave();
      return;
    }
    // Groups and matched properties change what the layers panel and the palette show.
    if (cmd.type === "ungroup" || cmd.type === "match_properties") {
      await run(async () => {
        await runDrawingCommand(cmd);
      });
      return;
    }
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
      // Divide, measure along, the draws and the modifies change the drawing itself.
      const drawingChanged = [
        "divide_along",
        "measure_along",
        "draw",
        "modify",
        "edit",
        "delete_entities",
        "text",
        "mtext",
        "edit_text",
        "find_replace",
        "dimension",
        "dim_edit",
        "dim_break",
        "dim_space",
        "dim_jog_line",
        "leader",
        "mleader",
        "tolerance",
        "insert_block",
        "edit_block_in_place",
        "paste_from_clipboard",
        "mview",
      ];
      if (drawingChanged.includes(cmd.type)) {
        await redraw();
      }
      // A new viewport shows in the layers panel's VP freeze column.
      if (cmd.type === "mview") await syncInfo();
      if (cmd.type === "paste_from_clipboard" && outcome.type === "added") {
        setNotice(t("clip.pasted", { n: outcome.handles.length }));
      }
      // An edit in place began: the bar to end it shows, and Select works on the block.
      if (cmd.type === "edit_block_in_place") {
        setInfo(await drawingInfo());
        chooseTool("select");
      }
      if (outcome.type === "measured") setSelectedId(outcome.measurement.id);
      if (outcome.type === "exploded" || outcome.type === "takeoff_followed") {
        const notes: string[] = [];
        if (outcome.type === "exploded" && outcome.left_alone > 0) {
          notes.push(t("modify.leftAlone", { n: outcome.left_alone }));
        }
        if (outcome.remeasured > 0) notes.push(t("modify.remeasured", { n: outcome.remeasured }));
        if (outcome.unlinked > 0) notes.push(t("modify.unlinked", { n: outcome.unlinked }));
        if (notes.length > 0) setNotice(notes.join(" · "));
      }
      if (outcome.type === "edited") {
        const notes: string[] = [];
        if (cmd.type === "edit" && cmd.kind === "overkill") {
          notes.push(t("edit.removed", { n: outcome.removed.length }));
        } else if (outcome.left_alone > 0) {
          notes.push(t("edit.leftAlone", { n: outcome.left_alone }));
        }
        if (outcome.unlinked > 0) notes.push(t("edit.unlinked", { n: outcome.unlinked }));
        if (outcome.joined_rows > 1) notes.push(t("edit.joinedRows", { n: outcome.joined_rows }));
        if (notes.length > 0) setNotice(notes.join(" · "));
      }
    });
    if (changed) await autosave();
  }

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

  /** The height new text takes: the last one used, else 2.5 mm on paper in drawing units at the
   *  annotation scale. */
  const currentTextHeight =
    textHeight ?? (0.0025 / metresPerUnit(info?.units ?? "unknown")) * (info?.annotation_factor ?? 1);
  /** The style new text takes: the last one used when the drawing has it, else its current one. */
  const currentTextStyle =
    textStyle !== null && info?.text_styles.includes(textStyle) ? textStyle : (info?.text_style ?? "Standard");

  /** A text tool placed a text: TEXT opens the in-place editor where it starts, MTEXT the MText
   *  dialog. */
  function placeText(request: TextRequest) {
    if (request.kind === "mtext") {
      setTextDialog({ kind: "mtext", request });
      return;
    }
    if (request.kind === "table") {
      setNewTableAt(request.at);
      return;
    }
    if (request.kind === "field") {
      setFieldTarget(null);
      setFieldAsk({ mode: "text", at: request.at, height: request.height });
      return;
    }
    if (request.kind === "insertBlock") {
      placeBlock(request);
      return;
    }
    if (request.kind === "attdef") {
      setAttDefAt({ at: request.at, height: request.height });
      return;
    }
    const at = viewport.current?.toScreen(request.at) ?? [0, 0];
    setTextEditor({ request, at, key: performance.now() });
  }

  /** Opens the text edit dialog on the text of the top-level entity `handle`, or the table editor
   *  on a table (at the cell under `at`, the click). */
  async function openTextEdit(handle: number, at: Point | null = null) {
    const e = await entityInfo(handle);
    const value = e.plain_text ?? e.text;
    if (e.kind === "Table" && e.table) {
      const cell = (at && (await tableCellAt(handle, at))) ?? [0, 0];
      setTableEditing({ handle, table: e.table, cell });
    } else if ((e.kind === "Text" || e.kind === "MText") && value !== null) {
      setTextDialog({ kind: "edit", handle, value, multiline: e.kind === "MText" });
    } else if (e.kind === "Insert" && e.attributes.length > 0) {
      const defs = (await readBlocks()).find((b) => b.name === e.block_name)?.attdefs ?? [];
      const before = Object.fromEntries(e.attributes);
      setAttributesAsk({
        heading: "block.editAttributesTitle",
        fields: e.attributes.map(([tag, value]) => ({
          tag,
          prompt: defs.find((d) => d.tag === tag)?.prompt ?? "",
          value,
        })),
        // Only what changed; nothing changed sends nothing.
        command: (values) => {
          const changed = Object.entries(values).filter(([tag, v]) => v !== before[tag]);
          if (changed.length === 0) return null;
          return { type: "edit_attributes", target: { handle, chain: [] }, values: Object.fromEntries(changed) };
        },
      });
    } else {
      setNotice(t("text.noText"));
    }
  }

  /** Runs a dimension style or annotation scale change from the style panel, then shows the
   *  drawing's styles again and draws the dimensions the style rebuilt; an error goes back to
   *  the panel. */
  async function runDimStyle(cmd: Command) {
    const gen = switchGen.current;
    await runCommand(cmd);
    const next = await drawingInfo();
    if (isStale(gen, switchGen.current)) return;
    setInfo(next);
    await redraw();
    await autosave();
  }

  /** Opens the dimension style panel on the drawing's styles as they are now. */
  function openDimStyle() {
    const gen = switchGen.current;
    run(async () => {
      const next = await drawingInfo();
      if (isStale(gen, switchGen.current)) return;
      setInfo(next);
      setDimStyleOpen(true);
    });
  }

  /** Runs a text command from a dialog, then draws it; an error goes back to the dialog. */
  async function runText(cmd: Command) {
    await runCommand(cmd);
    await redraw();
    await autosave();
  }

  /** Runs a command on the table being edited, draws it, and hands the editor its cells as they
   *  are now; an error goes back to the editor. */
  async function runTable(cmd: Command) {
    const editing = tableEditing;
    if (!editing) return;
    await runText(cmd);
    const e = await entityInfo(editing.handle);
    if (e.table) setTableEditing({ ...editing, table: e.table });
  }

  /** Inserts the field the dialog chose: a new text where the user clicked, or the field's code
   *  after the text of the table cell being edited. */
  async function insertField(field: FieldKind) {
    const ask = fieldAsk;
    if (ask?.mode === "text") {
      await runText({ type: "field", at: ask.at, height: ask.height ?? currentTextHeight, field });
    } else if (ask?.mode === "cell" && tableEditing) {
      const [row, col] = ask.cell;
      const now = tableEditing.table.cells[row]?.[col] ?? "";
      const text = now ? `${now} ${fieldCode(field)}` : fieldCode(field);
      await runTable({ type: "edit_cell", target: { handle: tableEditing.handle, chain: [] }, row, col, text });
    }
  }

  /** UPDATEFIELD: works every field out again (one undo step when one changed), then draws it. */
  function updateFields() {
    run(async () => {
      const outcome = await runCommand({ type: "update_fields" });
      await redraw();
      await autosave();
      const locked = outcome.type === "updated" ? outcome.skipped : 0;
      const notes = [t("insertField.updated")];
      if (locked > 0) notes.push(t("locked.skipped", { n: locked }));
      setNotice(notes.join(" · "));
    });
  }

  /** Runs a block command from a dialog or the edit bar, then draws it and reads the take-off
   *  (rows may lose their links) and the drawing's state (an edit in place may end) again; an
   *  error goes back to the caller. */
  /** Runs a layer, group or property command (its error goes to the caller), then reads the
   *  drawing again: the batch, the take-off, the layers, groups and hidden layers. */
  async function runDrawingCommand(cmd: Command) {
    const gen = switchGen.current;
    const outcome = await runCommand(cmd);
    await redraw();
    await refreshTakeoffAndBoq();
    const next = await drawingInfo();
    if (!isStale(gen, switchGen.current)) {
      setInfo(next);
      setHidden(new Set(next.hidden_layers));
    }
    await autosave();
    return outcome;
  }

  /** `runDrawingCommand` with its error in the status bar. */
  const runLayerCommand = (cmd: Command) =>
    run(async () => {
      await runDrawingCommand(cmd);
    });

  /** The group the top-level entity `handle` is in, while group selection is on. */
  function groupOf(handle: number) {
    if (!groupSelect) return null;
    return info?.groups?.find((g) => g.selectable && g.handles.includes(handle)) ?? null;
  }

  /** Asks for a drawing and reads its layers, for the layer translator's standard. */
  async function loadLayerStandard() {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return null;
    return { path, layers: await drawingLayers(path) };
  }

  async function runBlock(cmd: Command) {
    const gen = switchGen.current;
    const outcome = await runCommand(cmd);
    await redraw();
    await refreshTakeoffAndBoq();
    const next = await drawingInfo();
    if (!isStale(gen, switchGen.current)) setInfo(next);
    await autosave();
    return outcome;
  }

  /** A block's picture for the block palette, as a URL revoked when the palette closes. */
  async function blockPicture(name: string) {
    const png = await blockThumbnail(name, 96);
    const url = URL.createObjectURL(new Blob([png], { type: "image/png" }));
    thumbUrls.current.push(url);
    return url;
  }

  function closeBlockPalette() {
    thumbUrls.current.forEach((url) => URL.revokeObjectURL(url));
    thumbUrls.current = [];
    setBlockPaletteOpen(false);
    focusDrawing();
  }

  /** Starts INSERT with `block`; with `fromPath` its definition is copied from that drawing when
   *  this one lacks it. */
  function startInsert(block: BlockInfo, fromPath: string | null = null) {
    setInsertChoice({ block, fromPath });
    chooseTool("insertBlock");
  }

  /** INSERT's click: the block goes there, after its attribute prompts when it has any. */
  function placeBlock(r: Extract<TextRequest, { kind: "insertBlock" }>) {
    const choice = insertChoice;
    if (!choice) {
      setBlockPaletteOpen(true);
      return;
    }
    const command = (values: Record<string, string>): Command => ({
      type: "insert_block",
      name: choice.block.name,
      at: r.at,
      scale: r.scale,
      rotation: r.rotation,
      attribute_values: values,
      from_path: choice.fromPath,
    });
    const fields = promptsFor(choice.block.attdefs);
    if (fields.length === 0) {
      apply(command({}));
      return;
    }
    setAttributesAsk({ heading: "block.attributesTitle", fields, command });
  }

  /** WBLOCK: writes the block to the .dwg the Save dialog names. */
  function writeBlockFile(name: string) {
    run(async () => {
      const path = await save({ defaultPath: `${name}.dwg`, filters: [{ name: "DWG", extensions: ["dwg"] }] });
      if (!path) return;
      await writeBlock(name, path);
      setNotice(t("block.written", { path }));
    });
  }

  /** REFCLOSE and the edit bar: ends the edit in place, saved into the block or discarded. */
  function closeBlockEdit(keep: boolean) {
    const name = info?.block_edit;
    if (!name) {
      setError(t("cmd.notNow", { name: "REFCLOSE" }));
      return;
    }
    run(async () => {
      const outcome = await runBlock({ type: keep ? "save_block_edits" : "discard_block_edits" });
      if (outcome.type !== "block_edit_closed") return;
      const notes = [t(keep ? "block.saved" : "block.discarded", { name })];
      if (outcome.unlinked > 0) notes.push(t("block.closedUnlinked", { n: outcome.unlinked }));
      if (outcome.dropped > 0) notes.push(t("block.closedDropped", { n: outcome.dropped }));
      setNotice(notes.join(" · "));
    });
  }

  function openAttributeManager() {
    run(async () => setManagerBlocks(await readBlocks()));
  }

  function openDesignCenter() {
    run(async () => setDesignCenter((await readBlocks()).map((b) => b.name.toUpperCase())));
  }

  function openToolPalettes() {
    run(async () => setPalette((await getSettings()).tool_palette ?? []));
  }

  /** Puts a button for the block `name` on the tool palette. */
  function addToPalette(name: string) {
    run(async () => {
      const now = (await getSettings()).tool_palette ?? [];
      await setToolPalette([...now, { kind: "block", label: name, value: name }]);
    });
  }

  /** A tool palette button: its block is inserted, its command line run. */
  function runPaletteItem(item: PaletteItem) {
    if (item.kind === "command") {
      const a = parse(item.value, { expect: "command", last: null, aliases: commandAliases });
      if (a.type === "error") setError(t(a.key, a.args));
      else onCommand(a);
      return;
    }
    run(async () => {
      const block = (await readBlocks()).find((b) => b.name.toUpperCase() === item.value.toUpperCase());
      if (block) startInsert(block);
      else setError(t("block.missing", { name: item.value }));
    });
  }

  /** Places TEXT's text, remembering its height and style for the next one. */
  function placeInPlace(request: Extract<TextRequest, { kind: "text" }>, draft: TextDraft) {
    setTextEditor(null);
    setTextHeight(draft.height);
    setTextStyle(draft.style);
    apply({
      type: "text",
      at: request.at,
      height: draft.height,
      rotation: request.rotation,
      value: draft.value,
      style: draft.style,
    });
    focusDrawing();
  }

  /** The find dialog's search: the matches, nothing changed. */
  async function findTexts(q: FindQuery) {
    const outcome = await runCommand({ type: "find_text", ...q });
    return outcome.type === "found" ? outcome : { count: 0, matches: [] };
  }

  /** Replaces every match (one undo step), draws the change and highlights what changed. */
  async function replaceTexts(q: FindQuery, replace: string) {
    const outcome = await runCommand({ type: "find_replace", ...q, replace });
    if (outcome.type !== "replaced") return { count: 0, skipped: 0 };
    if (outcome.count > 0) {
      await redraw();
      setAiHighlight(outcome.handles);
      await autosave();
    }
    return { count: outcome.count, skipped: outcome.skipped };
  }

  /** Shows one entity the find found, highlighted and zoomed to with room around; on another
   *  sheet, that sheet is shown and its first fit goes to the match. */
  async function showMatch(m: TextMatch) {
    setAiHighlight([m.handle]);
    if (m.sheet.toLowerCase() !== sheet.toLowerCase()) {
      fitMatch.current = m.handle;
      await chooseSheet(m.sheet);
      return;
    }
    const box = matchBounds(m.handle, batchHeld.current);
    if (box) viewport.current?.fitTo(box);
  }

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
      // Or a layer change: the panel shows the layers and hidden ones as they are now.
      await syncInfo();
    });
    if (changed) await autosave();
  }

  /** Reads the drawing's info again (layers, groups, current state) and the hidden layers. */
  async function syncInfo() {
    const gen = switchGen.current;
    const next = await drawingInfo();
    if (isStale(gen, switchGen.current)) return;
    setInfo(next);
    setHidden(new Set(next.hidden_layers));
  }

  /** Reads the file tabs again (titles, unsaved dots, the active one). */
  async function refreshTabs() {
    try {
      const next = await readTabs();
      setFileTabs(next.tabs);
      setActiveTab(next.active);
    } catch {
      // The tabs keep what they showed; the next refresh tries again.
    }
  }

  /** Fetches the atlas pages `next` needs that the window doesn't hold yet. */
  async function syncAtlas(next: Batch) {
    if (next.atlasVersion === 0 || next.atlasVersion === atlasVersionRef.current) return;
    const gen = switchGen.current;
    const update = parseAtlas(await atlasPages(atlasVersionRef.current ?? undefined));
    // Another drawing's atlas: never mixed into this one's.
    if (isStale(gen, switchGen.current)) return;
    for (const page of update.pages) atlasHeld.current.set(page.index, page);
    atlasVersionRef.current = update.version;
    setAtlas([...atlasHeld.current.values()]);
  }

  /** Draws the core's current batch. When the core says the drawing, sheet and layers are as they
   *  were at the last draw, the batch in hand stays and only the fit (if asked) happens. A fit draws
   *  the glyphs for the core's own guess at the zoom; otherwise for the zoom on screen. */
  async function redraw(fit = false) {
    const gen = switchGen.current;
    // A drawing change, its undo or a save may have changed the tab's unsaved dot.
    const [version] = await Promise.all([batchVersion(), refreshTabs()]);
    if (isStale(gen, switchGen.current)) return;
    if (version === batchVersionRef.current && batchHeld.current) {
      // Nothing to send; the effect below still fits the view once the window has rendered.
      if (fit) {
        fitNext.current = true;
        setFitTick((n) => n + 1);
      }
      return;
    }
    const bytes = await renderBatch(undefined, fit ? undefined : viewport.current?.pxPerUnit());
    if (!bytes || isStale(gen, switchGen.current)) return;
    const next = parseBatch(bytes);
    await syncAtlas(next);
    if (isStale(gen, switchGen.current)) return;
    batchVersionRef.current = version;
    batchHeld.current = next;
    fitNext.current = fit;
    setBatch(next);
    await syncInset();
  }

  /** On a layout, fetches model space for its viewports (when the core's changed) and how each
   *  shows it; elsewhere drops it. Its glyphs are drawn for the largest viewport's zoom. */
  async function syncInset(pxPerUnit?: number) {
    const gen = switchGen.current;
    let views = await paperViews();
    if (isStale(gen, switchGen.current)) return;
    if (views.length === 0) {
      insetHeld.current = null;
      setInset(null);
      return;
    }
    const scale = Math.max(...views.map((v) => v.scale));
    const held = insetHeld.current;
    const next = await refreshSheet(held, pxPerUnit === undefined ? undefined : pxPerUnit * scale, {
      version: () => sheetBatchVersion(MODEL),
      fetch: async (px) => {
        const bytes = await sheetBatch(MODEL, undefined, px ?? (viewport.current?.pxPerUnit() ?? 1) * scale);
        if (!bytes || isStale(gen, switchGen.current)) return null;
        const batch = parseBatch(bytes);
        await syncAtlas(batch);
        return batch;
      },
    });
    if (!next || isStale(gen, switchGen.current)) return;
    // Glyphs drawn again move the viewports' text spans: ask for them again.
    if (next !== held) views = await paperViews();
    if (isStale(gen, switchGen.current)) return;
    insetHeld.current = next;
    setInset({ batch: next.batch, views });
  }

  /** Fetches the underlay pictures in `visible` that `next` needs for a zoom of `pxPerUnit` and
   *  the window doesn't hold at that version and about that zoom, and lets go of those out of view
   *  (their textures go too). One fetch runs at a time; the viewport asks again when the view
   *  settles. */
  async function syncPictures(next: Batch | null, pxPerUnit: number, visible: Bounds) {
    if (picturesBusy.current) return;
    picturesBusy.current = true;
    const gen = switchGen.current;
    const held = picturesHeld.current;
    // A drawing change replaces the map held; nothing fetched before it is kept.
    const stale = () => isStale(gen, switchGen.current) || picturesHeld.current !== held;
    try {
      const fetched = await fetchPictures(
        next,
        held,
        pxPerUnit,
        visible,
        (id) => underlayImage(id, pxPerUnit),
        stale,
      );
      if (fetched && !stale()) {
        picturesHeld.current = fetched;
        setPictures(new Map(fetched));
      }
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
    const gen = switchGen.current;
    await run(async () => {
      const version = await batchVersion();
      const bytes = await renderBatch(undefined, pxPerUnit);
      if (!bytes || isStale(gen, switchGen.current)) return;
      let next = parseBatch(bytes);
      await syncAtlas(next);
      if (isStale(gen, switchGen.current)) return;
      const held = batchHeld.current;
      if (held && version === batchVersionRef.current) {
        // The same lines and fills: keep the arrays the viewport has uploaded already.
        const { segments, colors, ranges, chains, fills, fillRanges } = held;
        next = { ...next, segments, colors, ranges, chains, fills, fillRanges };
      }
      batchVersionRef.current = version;
      batchHeld.current = next;
      setBatch(next);
      if (insetHeld.current) await syncInset(pxPerUnit);
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
        measure.cancel();
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
      const gen = switchGen.current;
      const next = await drawingInfo();
      if (isStale(gen, switchGen.current)) return;
      setInfo(next);
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
          await syncInfo();
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
    batchVersionRef.current = null;
    batchHeld.current = null;
    insetHeld.current = null;
    setInset(null);
    // The second view showed another drawing's sheet.
    setSplit(false);
    atlasVersionRef.current = null;
    atlasHeld.current = new Map();
    setAtlas([]);
    picturesHeld.current = new Map();
    setPictures(new Map());
    autosaveOff.current = false;
    setSelectedId(null);
    setBoqCode(null);
    setAiHighlight([]);
    setConflictIds([]);
    setCompared(false);
    setInfoCard(null);
    setPending(null);
    setPickedInsert(null);
    setCalibration(null);
    setAttachOpen(false);
    setAttachImageOpen(false);
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
    setBatch(null);
    setInfo(null);
    setDraftingState(null);
    setSheetBlock("");
    setHidden(new Set());
    setItems([]);
    setItemTotals([]);
    setBill(null);
    setConflicts([]);
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

  /** Saves the view on screen as the named view `name` (a drawing change). Errors go to the menu. */
  async function saveView(name: string) {
    const box = viewport.current?.visible();
    if (!box) return;
    await runCommand({ type: "save_view", name, bounds: box });
    await redraw();
  }

  /** Removes the named view `name` (a drawing change). Errors go to the menu. */
  async function deleteView(name: string) {
    await runCommand({ type: "delete_view", name });
    await redraw();
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
      // The layers panel's VP freeze column lists this sheet's viewports.
      setInfo(await drawingInfo());
      measure.cancel();
      fitIds.current = fit ?? null;
      await redraw(true);
    });

  /** Moves the hidden set to `target`, one core call per changed layer. Whatever happens, the panel
   *  shows the calls that went through and the canvas is redrawn from the core. */
  async function applyHidden(target: Set<string>) {
    if (layerBusy.current) return;
    layerBusy.current = true;
    setLayersBusy(true);
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
    setLayersBusy(false);
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

  /** One property of the drawing's layer `name` (one undo step). */
  const setLayerProp = useCallback(
    (name: string, prop: LayerProp, value: boolean | string | number) =>
      runLayerCommand({ type: "set_layer", name, prop, value }),
    [info],
  );

  // The layers panel's handlers, stable between pointer moves so the memoised panel doesn't
  // render again with the cursor.
  const renameLayer = useCallback(
    (from: string, to: string) => runLayerCommand({ type: "rename_layer", from, to }),
    [info],
  );
  const deleteLayer = useCallback((name: string) => runLayerCommand({ type: "delete_layer", name }), [info]);
  const newLayer = useCallback((name: string) => runLayerCommand({ type: "new_layer", name }), [info]);
  const openLayerStates = useCallback(() => setLayerStatesOpen(true), []);
  const openTranslator = useCallback(() => setTranslatorOpen(true), []);

  /** The viewport `viewport` freezes `layer` inside it, or thaws it. */
  const setViewportFrozen = useCallback(
    (viewport: number, layer: string, frozen: boolean) =>
      runLayerCommand({ type: "set_viewport_frozen", viewport: { handle: viewport, chain: [] }, layer, frozen }),
    [info],
  );

  /** The palette's edit of the picked entity (a drawing's own, at the top of the sheet). */
  const setPickedProperty = useCallback(
    (prop: EntityProp, value: boolean | string | number | [number, number]) => {
      if (!pickedInfo) return;
      runLayerCommand({ type: "set_property", target: { handle: pickedInfo.handle, chain: [] }, prop, value });
    },
    [pickedInfo, info],
  );

  /** What new entities are drawn with, from the palette (not an undo step). */
  const setCurrentState = useCallback(
    (current: Current) => {
      const gen = switchGen.current;
      run(async () => {
        await runCommand({ type: "set_current", current });
        const next = await drawingInfo();
        if (isStale(gen, switchGen.current)) return;
        setInfo(next);
        await autosave();
      });
    },
    [info],
  );

  /** The drawing's own layers (not its xrefs'), for the palette. */
  const ownLayers = useMemo(() => info?.layers.map((l) => l.name).filter((n) => !n.includes("|")) ?? [], [info]);

  /** Draws new entities on layer `name` from now on (saved with the project, not an undo step). */
  const makeCurrent = useCallback(
    (name: string) => {
      if (!info) return;
      const gen = switchGen.current;
      run(async () => {
        await runCommand({ type: "set_current", current: { ...info.current, layer: name } });
        const next = await drawingInfo();
        if (isStale(gen, switchGen.current)) return;
        setInfo(next);
        await autosave();
      });
    },
    [info],
  );

  /** Attaches a PDF page from the dialog, then draws it: an underlay's picture is fetched before
   *  the dialog closes, so its progress shows there. Errors go to the dialog. */
  async function attachPdfPage(r: AttachRequest) {
    const before = batchHeld.current;
    const added = await attachPdf(r.path, r.page, r.at, r.scale, r.mode);
    setInfo(await drawingInfo());
    await redraw();
    const view = viewport.current;
    if (r.mode === "underlay" && view) {
      showNewPicture(before);
      await syncPictures(batchHeld.current, view.pxPerUnit(), view.visible());
    } else {
      setNotice(t("attachPdf.imported", { n: added }));
    }
  }

  /** Attaches an image from the dialog, then draws it with its picture. Errors go to the dialog. */
  async function attachImageFile(r: AttachImageRequest) {
    const before = batchHeld.current;
    await attachImage(r.path, r.at, r.width);
    setInfo(await drawingInfo());
    await redraw();
    const view = viewport.current;
    showNewPicture(before);
    if (view) await syncPictures(batchHeld.current, view.pxPerUnit(), view.visible());
  }

  /** Fits the view to the picture the batch on screen has and `before` did not (one just
   *  attached), so it never lands off screen. */
  function showNewPicture(before: Batch | null) {
    const box = newPictureBounds(before, batchHeld.current);
    if (box) viewport.current?.fitTo(box);
  }

  /** Sets the chosen image's brightness, contrast and fade (one undo step), then draws it. */
  async function setChosenDisplay(handle: number, display: ImageDisplay) {
    await runCommand({ type: "set_image_display", handle, ...display });
    await redraw();
  }

  /** Gives `p` to the PDF or image insert point or the calibration waiting for one. False when
   *  none is. */
  function pendingPoint(p: Point): boolean {
    if (pending?.kind === "insert") {
      setPickedInsert(p);
      setPending(null);
    } else if (pending?.kind === "calibrate" && !pending.first) {
      setPending({ kind: "calibrate", first: p });
    } else if (pending?.kind === "calibrate" && pending.first) {
      setCalibration([pending.first, p]);
      setPending(null);
    } else {
      return false;
    }
    return true;
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

  // Hand the keyboard back to the drawing, so Space pans instead of pressing a button.
  const focusDrawing = () => viewportBox.current?.focus();

  function chooseTool(next: Tool) {
    preset.current = null;
    setTool(next);
    focusDrawing();
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

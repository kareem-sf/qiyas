import { useRef, useState } from "react";
import {
  entityInfo,
  snap,
  pick,
  pickWindow,
  boundaryAt,
  undo,
  type DrawingInfo,
} from "../api";
import { useLanguage, useT } from "../i18n";
import {
} from "../shell/AttachImageDialog";
import {
  metresPerUnit,
} from "../shell/AttachPdfDialog";
import {
} from "../shell/BlockDialogs";
import { CommandLine } from "../shell/CommandLine";
import { StatusBar } from "../shell/StatusBar";
import type { Action } from "../tools/commandParser";
import { useTool } from "../tools/useTool";
import { readProblems } from "./shared";
import { useDrafting } from "./useDrafting";
import { Dialogs, FileDialogs } from "./Dialogs";
import { Header } from "./Header";
import { LeftPanels, SidePanel, WorkArea } from "./Panels";
import { useAnnotate } from "./useAnnotate";
import { useAssistantWiring } from "./useAssistantWiring";
import { useAttach } from "./useAttach";
import { useCommandLine } from "./useCommandLine";
import { useBlocks } from "./useBlocks";
import { type Commands, useCommands, useToolChoice } from "./useCommands";
import { useAutosave, useDrawings, useFileTabs } from "./useDrawings";
import { useLayers } from "./useLayers";
import { useSheets } from "./useSheets";
import { useOverlay } from "./useOverlay";
import { useSelection } from "./useSelection";
import { useShell } from "./useShell";
import { useTakeoff, useTakeoffState } from "./useTakeoff";

export default function App() {
  const t = useT();
  const { lang } = useLanguage();
  const [info, setInfo] = useState<DrawingInfo | null>(null);
  const shell = useShell();
  const {
    theme,
    settingsOpen,
    setSettingsOpen,
    commandAliases,
    setError,
    setNotice,
    setSideTab,
    viewport,
    run,
    focusDrawing,
  } = shell;
  const { fileTabs, setFileTabs, activeTab, setActiveTab, refreshTabs } = useFileTabs();
  const { autosaveOff, autosave } = useAutosave(t, setError);
  /** Bumped whenever another drawing comes on screen (open, tab switch, close): work started
   *  before is stale (`isStale`) and drops its results. */
  const switchGen = useRef(0);
  const draftingState = useDrafting({ run, autosave });
  const { drafting, setDraftingState, setDraftingOpen, toggleDrafting } = draftingState;
  const takeoffState = useTakeoffState(switchGen);
  const {
    items,
    setSelectedId,
    setCompared,
    aiHighlight,
    setAiHighlight,
    setInfoCard,
    refreshTakeoffAndBoq,
  } = takeoffState;
  /** The tool hook comes after the sheets (it takes their state); the sheets cancel it through this. */
  const measureRef = useRef<{ cancel(): void } | null>(null);
  const sheets = useSheets({ switchGen, viewport, run, autosave, setInfo, refreshTabs, items, measure: measureRef });
  const {
    batch,
    batchHeld,
    setRenderProgress,
    setArabicReversed,
    sheet,
    setSheetBlock,
    hidden,
    setHidden,
    fitMatch,
    redraw,
    syncPictures,
    chooseSheet,
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
    setGroupAsk,
    runDrawingCommand,
    runLayerCommand,
    groupOf,
    syncInfo,
  } = layers;
  const toolChoice = useToolChoice(focusDrawing);
  const { tool, lastPoint, preset, chooseTool } = toolChoice;
  const attach = useAttach({ t, setInfo, setNotice, viewport, batchHeld, redraw, syncPictures });
  const {
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
    setCreateBlockAsk,
    setAttributesAsk,
    setAttDefAt,
    placeBlock,
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
    setDimText,
    currentTextHeight,
    placeText,
    openTextEdit,
  } = annotate;

  const commands = useCommands({
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
  const { apply, history } = commands;
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
  const ai = useAssistantWiring({
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
  const { assistant } = ai;
  const drawings = useDrawings({
    t,
    switchGen,
    info,
    setInfo,
    setError,
    setNotice,
    run,
    settingsOpen,
    fileTabs,
    setFileTabs,
    setActiveTab,
    refreshTabs,
    autosaveOff,
    autosave,
    setDraftingState,
    sheets,
    setArabicReversed,
    setSheetBlock,
    setHidden,
    redraw,
    takeoffState,
    refreshTakeoffAndBoq,
    attach,
    preset,
    lastPoint,
    measure,
  });
  const takeoff = useTakeoff({
    t,
    info,
    setNotice,
    run,
    autosave,
    setSideTab,
    viewport,
    sheet,
    batch,
    chooseSheet,
    takeoffState,
    apply,
  });
  const {
    selected,
    boqLinked,
    conflictLinked,
  } = takeoff;
  const { commandLineOn, commandInput, onCommand, clickDrawing, commandState, prompt, message } = useCommandLine({
    t,
    info,
    measure,
    assistant,
    shell,
    drafting: { toggleDrafting },
    toolChoice,
    commands,
    sheets,
    layers,
    attach,
    blocks,
    annotate,
    drawings,
    takeoffState,
    takeoff,
  });
  onCommandRef.current = onCommand;
  const selection = useSelection({
    info,
    tool,
    measure,
    batch,
    redraw,
    runLayerCommand,
  });
  const { highlight, overlay } = useOverlay({ tool, measure, batch, selected, boqLinked, conflictLinked, aiHighlight });




  // An empty model is said next to the file name, not counted as a reading problem.
  const problems = info ? readProblems(info) : [];
  const sheetShown = info?.sheets.find((s) => s.block.toLowerCase() === sheet.toLowerCase());
  // Named views are of model space.
  const onModel = sheetShown?.is_model ?? false;
  // Unknown units are measured as millimetres; the status bar says so.
  const units = info ? t(`unit.${info.units === "unknown" ? "millimetres" : info.units}`) : "";

  return (
    <div className="app">
      <Header
        info={info}
        onModel={onModel}
        measure={measure}
        shell={shell}
        toolChoice={toolChoice}
        commands={commands}
        sheets={sheets}
        layers={layers}
        attach={attach}
        blocks={blocks}
        annotate={annotate}
        drawings={drawings}
        takeoff={takeoff}
      />
      <Dialogs
        info={info}
        shell={shell}
        assistant={assistant}
        drafting={draftingState}
        sheets={sheets}
        layers={layers}
        attach={attach}
        blocks={blocks}
        annotate={annotate}
        drawings={drawings}
      />
      <div className="middle">
        <LeftPanels
          info={info}
          sheetShown={sheetShown}
          theme={theme}
          drafting={drafting}
          sheets={sheets}
          layers={layers}
          selection={selection}
        />
        <WorkArea
          info={info}
          fileTabs={fileTabs}
          activeTab={activeTab}
          tool={tool}
          measure={measure}
          clickDrawing={clickDrawing}
          highlight={highlight}
          overlay={overlay}
          drafting={drafting}
          shell={shell}
          sheets={sheets}
          attach={attach}
          blocks={blocks}
          annotate={annotate}
          drawings={drawings}
          selection={selection}
        />
        <SidePanel
          info={info}
          drafting={drafting}
          shell={shell}
          takeoffState={takeoffState}
          takeoff={takeoff}
          ai={ai}
        />
      </div>
      {commandLineOn && (
        <CommandLine
          state={commandState}
          prompt={prompt}
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
      <FileDialogs drawings={drawings} />
    </div>
  );
}

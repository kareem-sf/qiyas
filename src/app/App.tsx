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
import { AssistantPanel } from "../ai/AssistantPanel";
import { BoqPanel } from "../boq/BoqPanel";
import { ConflictsPanel } from "../boq/ConflictsPanel";
import { openCount } from "../boq/conflictsMath";
import { useLanguage, useT } from "../i18n";
import {
  ImageDisplayPanel,
} from "../shell/AttachImageDialog";
import {
  metresPerUnit,
} from "../shell/AttachPdfDialog";
import {
  BlockEditBar,
} from "../shell/BlockDialogs";
import { CommandLine } from "../shell/CommandLine";
import { FileTabs } from "../shell/FileTabs";
import { AlongPrompt, InfoCard } from "../shell/InfoCard";
import { LayersPanel } from "../shell/LayersPanel";
import { PropertiesPanel } from "../shell/PropertiesPanel";
import { InPlaceText } from "../shell/TextEditor";
import { SheetTabs } from "../shell/SheetTabs";
import { StatusBar } from "../shell/StatusBar";
import { TakeoffPanel } from "../shell/TakeoffPanel";
import type { Action } from "../tools/commandParser";
import { useTool } from "../tools/useTool";
import { SplitPane } from "../viewport/SplitPane";
import { Viewport } from "../viewport/Viewport";
import { NO_VIEWPORTS, readProblems } from "./shared";
import { useDrafting } from "./useDrafting";
import { Dialogs, FileDialogs } from "./Dialogs";
import { Header } from "./Header";
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
    error,
    setError,
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
  const draftingState = useDrafting({ run, autosave });
  const { drafting, setDraftingState, setDraftingOpen, toggleDrafting } = draftingState;
  const takeoffState = useTakeoffState(switchGen);
  const {
    items,
    itemTotals,
    selectedId,
    setSelectedId,
    bill,
    boqCode,
    conflicts,
    compared,
    setCompared,
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
    setRenderProgress,
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
    layersBusy,
    runDrawingCommand,
    runLayerCommand,
    groupOf,
    syncInfo,
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
  const { tool, lastPoint, preset, chooseTool } = toolChoice;
  const attach = useAttach({ t, setInfo, setNotice, viewport, batchHeld, redraw, syncPictures });
  const {
    pending,
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
    closeBlockEdit,
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
    textEditor,
    setTextEditor,
    currentTextHeight,
    currentTextStyle,
    placeText,
    openTextEdit,
    placeInPlace,
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
  const {
    switching,
    switchTab,
    closeFileTab,
  } = drawings;
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
    onImportBoq,
    onCompare,
    onExportConflicts,
    selected,
    boqLinked,
    conflictLinked,
    selectRow,
    selectBoq,
    showConflict,
    resolveConflict,
    openBoqTab,
    linkRow,
    setTolerance,
    removeRow,
    renameRow,
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
  const { imageChosen, setImageChosen, pickedInfo, setPickedProperty, setChosenDisplay } = useSelection({
    info,
    tool,
    measure,
    batch,
    redraw,
    runLayerCommand,
  });
  const { highlight, overlay } = useOverlay({ tool, measure, batch, selected, boqLinked, conflictLinked, aiHighlight });




  const openConflicts = openCount(conflicts);
  // An empty model is said next to the file name, not counted as a reading problem.
  const problems = info ? readProblems(info) : [];
  const sheetShown = info?.sheets.find((s) => s.block.toLowerCase() === sheet.toLowerCase());
  const sheetName = sheetShown?.name ?? sheet;
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

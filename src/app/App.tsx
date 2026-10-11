import { open } from "@tauri-apps/plugin-dialog";
import { useRef, useState } from "react";
import {
  blocks as readBlocks,
  drawingBlocks,
  setToolPalette,
  calibrate,
  IMAGE_EXTENSIONS,
  imageSize,
  pdfPageCount,
  pdfPageSize,
  pdfThumbnails,
  entityInfo,
  snap,
  pick,
  pickWindow,
  reloadXrefs,
  boundaryAt,
  setXrefPath,
  undo,
  type DrawingInfo,
} from "../api";
import { AssistantPanel } from "../ai/AssistantPanel";
import { BoqPanel } from "../boq/BoqPanel";
import { ConflictsPanel } from "../boq/ConflictsPanel";
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
import { FileTabs } from "../shell/FileTabs";
import { AlongPrompt, InfoCard } from "../shell/InfoCard";
import { GroupNameDialog, LayerStatesDialog } from "../shell/LayerDialogs";
import { LayersPanel } from "../shell/LayersPanel";
import { LayerTranslator } from "../shell/LayerTranslator";
import { PropertiesPanel } from "../shell/PropertiesPanel";
import { RecoverReportDialog, RecoveryDialog } from "../shell/RecoveryDialog";
import { SheetSetManager } from "../shell/SheetSetManager";
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
import type { Action } from "../tools/commandParser";
import { useTool } from "../tools/useTool";
import { parseThumbnails } from "../viewport/batch";
import { SplitPane } from "../viewport/SplitPane";
import { Viewport } from "../viewport/Viewport";
import { NO_VIEWPORTS, readProblems } from "./shared";
import { useDrafting } from "./useDrafting";
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
    aboutOpen,
    setAboutOpen,
    settingsOpen,
    setSettingsOpen,
    commandAliases,
    loadAliases,
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
    renderProgress,
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
    layerStatesOpen,
    setLayerStatesOpen,
    translatorOpen,
    setTranslatorOpen,
    groupAsk,
    setGroupAsk,
    layersBusy,
    runDrawingCommand,
    runLayerCommand,
    groupOf,
    loadLayerStandard,
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
    attachOpen,
    setAttachOpen,
    attachImageOpen,
    setAttachImageOpen,
    pending,
    setPending,
    pickedInsert,
    calibration,
    setCalibration,
    attachPdfPage,
    attachImageFile,
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
    runText,
    runTable,
    insertField,
    placeInPlace,
    findTexts,
    replaceTexts,
    showMatch,
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
    recovery,
    setRecoveries,
    recoverReport,
    setRecoverReport,
    sheetSetShown,
    setSheetSetShown,
    xrefsOpen,
    setXrefsOpen,
    extractOpen,
    setExtractOpen,
    switching,
    openPath,
    recoverFrom,
    discardFrom,
    sheetSetApi,
    openAll,
    switchTab,
    closeFileTab,
    xrefsChanged,
    locateXref,
    extractRows,
    exportExtract,
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

/** The window's top bar: the ribbon, and the language, theme, settings and About buttons. */
import { clearRecents, recents as readRecents, redo, undo, views as readViews, type DrawingInfo, type NamedView } from "../api";
import { useLanguage, useT } from "../i18n";
import { Ribbon } from "../shell/Ribbon";
import type { Measure } from "./shared";
import type { Annotate } from "./useAnnotate";
import type { Attach } from "./useAttach";
import type { Blocks } from "./useBlocks";
import type { Commands, ToolChoice } from "./useCommands";
import type { Drawings } from "./useDrawings";
import type { Layers } from "./useLayers";
import type { Shell } from "./useShell";
import type { Sheets } from "./useSheets";
import type { Takeoff } from "./useTakeoff";

type Props = {
  info: DrawingInfo | null;
  /** The sheet on screen is model space: named views are of model space. */
  onModel: boolean;
  measure: Pick<Measure, "cancel">;
  shell: Shell;
  toolChoice: ToolChoice;
  commands: Commands;
  sheets: Sheets;
  layers: Layers;
  attach: Attach;
  blocks: Blocks;
  annotate: Annotate;
  drawings: Drawings;
  takeoff: Takeoff;
};

export function Header({
  info,
  onModel,
  measure,
  shell,
  toolChoice,
  commands,
  sheets,
  layers,
  attach,
  blocks,
  annotate,
  drawings,
  takeoff,
}: Props) {
  const t = useT();
  const { lang, setLang } = useLanguage();
  const { theme, chooseTheme, setAboutOpen, setSettingsOpen, viewport, focusDrawing } = shell;
  const { tool, chooseTool } = toolChoice;
  const { apply, history } = commands;
  const { batch, split, setSplit, arabicReversed, chooseArabicReversed, saveView, deleteView } = sheets;
  const { setLayerStatesOpen, setTranslatorOpen, groupSelect, setGroupSelect } = layers;
  const { setAttachOpen, setAttachImageOpen, setPending, setPickedInsert } = attach;
  const { setBlockPaletteOpen, openAttributeManager, openDesignCenter, openToolPalettes } = blocks;
  const { setFindOpen, openDimStyle, updateFields } = annotate;
  const {
    openFile,
    openPath,
    saveFile,
    saveFileAs,
    exportFile,
    setXrefsOpen,
    setExtractOpen,
    setSheetSetShown,
    recoverFile,
    runScriptFile,
    scriptLine,
  } = drawings;
  const { importBoqFile, compareNow, exportConflictsFile } = takeoff;

  return (
    // A running script: the ribbon waits (its Stop is beside the status bar).
    <header className="top" inert={scriptLine !== null}>
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
        onRunScript={runScriptFile}
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
  );
}

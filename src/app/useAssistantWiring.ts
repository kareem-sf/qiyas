/** Mizan (the in-app assistant) and outside AI apps over MCP: what their tools ask of the window
 *  (another sheet, other layers, a zoom), the take-off and drawing they changed, and the tool
 *  cards' Show and Undo. */
import { useCallback, useEffect, useRef, type RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  aiUndo,
  drawingInfo,
  PDF_PROGRESS,
  SESSION_CHANGED,
  TAKEOFF_CHANGED,
  type AiUi,
  type DrawingInfo,
} from "../api";
import type { ToolCardItem } from "../ai/messages";
import { useAssistant } from "../ai/useAssistant";
import type { Lang } from "../i18n";
import { isStale } from "../shell/switcher";
import { handlesBounds } from "../tools/overlay";
import type { Layers } from "./useLayers";
import type { Shell } from "./useShell";
import type { Sheets } from "./useSheets";
import type { TakeoffState } from "./useTakeoff";

type Deps = Pick<Shell, "run" | "setSettingsOpen" | "viewport"> &
  Pick<Sheets, "batch" | "setSheetBlock" | "setHidden" | "setRenderProgress" | "redraw"> &
  Pick<TakeoffState, "refreshTakeoffAndBoq" | "setCompared" | "setAiHighlight"> &
  Pick<Layers, "syncInfo"> & {
    lang: Lang;
    switchGen: RefObject<number>;
    info: DrawingInfo | null;
    setInfo(info: DrawingInfo): void;
    autosave(): Promise<void>;
    /** The tool in use: another sheet cancels it. */
    measure: { cancel(): void };
  };

export function useAssistantWiring({
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
}: Deps) {
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

  return { assistant, showCard, undoCard, openSettings };
}

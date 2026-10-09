import { useEffect, useRef, useState } from "react";
import { paperViews, sheetBatch, type PaperView, type Sheet, type Ucs } from "../api";
import { useT } from "../i18n";
import { parseBatch, type AtlasPage, type Batch } from "./batch";
import type { Theme } from "./gl";
import { Viewport, type ViewportHandle } from "./Viewport";

const MODEL = "*Model_Space";
const NO_TARGETS: never[] = [];

type Props = {
  /** The drawing's sheets. */
  sheets: Sheet[];
  /** The sheet on screen in the main view, its batch and what its viewports show: used as they
   *  are when this view shows the same sheet, and its change draws this one again. */
  mainSheet: string;
  mainBatch: Batch | null;
  mainInset: { batch: Batch; views: PaperView[] } | null;
  atlas: AtlasPage[];
  theme: Theme;
  ucs?: Ucs | null;
  /** Fetches the glyph atlas pages `b` needs that the window doesn't hold yet. */
  syncAtlas(b: Batch): Promise<void>;
  onError(e: unknown): void;
  onClose(): void;
};

/** VPORTS: the second view of a split window, beside the main one, on the drawing on screen. It has
 *  its own camera and sheet (the main one's batch shared when it shows the same sheet) and only
 *  looks: pan and zoom here, tools work in the main view. */
export function SplitPane({
  sheets,
  mainSheet,
  mainBatch,
  mainInset,
  atlas,
  theme,
  ucs = null,
  syncAtlas,
  onError,
  onClose,
}: Props) {
  const t = useT();
  const view = useRef<ViewportHandle>(null);
  const [sheet, setSheet] = useState(mainSheet);
  const [own, setOwn] = useState<{ batch: Batch; inset: { batch: Batch; views: PaperView[] } | null } | null>(
    null,
  );
  const fit = useRef(true);
  const same = sheet.toLowerCase() === mainSheet.toLowerCase();

  // Another sheet, or the drawing changed (the main batch is new): draw this sheet again.
  useEffect(() => {
    if (same) {
      setOwn(null);
      return;
    }
    let gone = false;
    (async () => {
      const bytes = await sheetBatch(sheet);
      if (!bytes || gone) return;
      const batch = parseBatch(bytes);
      await syncAtlas(batch);
      const views = await paperViews(sheet);
      let inset = null;
      if (views.length > 0) {
        const model = await sheetBatch(MODEL);
        if (model) {
          const parsed = parseBatch(model);
          await syncAtlas(parsed);
          inset = { batch: parsed, views };
        }
      }
      if (!gone) setOwn({ batch, inset });
    })().catch(onError);
    return () => {
      gone = true;
    };
  }, [sheet, same, mainBatch]);

  const batch = same ? mainBatch : (own?.batch ?? null);
  const inset = same ? mainInset : (own?.inset ?? null);

  // A new sheet here is fitted once its batch is in.
  useEffect(() => {
    if (fit.current && batch) {
      fit.current = false;
      view.current?.fitExtents();
    }
  }, [batch]);

  return (
    <div className="split-pane">
      <div className="split-bar">
        <select
          aria-label={t("vport.sheet")}
          value={sheet}
          onChange={(e) => {
            fit.current = true;
            setSheet(e.target.value);
          }}
        >
          {sheets.map((s) => (
            <option key={s.block} value={s.block}>
              {s.name}
            </option>
          ))}
        </select>
        <button type="button" onClick={onClose} aria-label={t("vport.one")} title={t("vport.one")}>
          ×
        </button>
      </div>
      <div className="split-view">
        <Viewport
          ref={view}
          batch={batch}
          atlas={atlas}
          highlight={NO_TARGETS}
          tool="select"
          theme={theme}
          ucs={ucs}
          inset={inset}
          onClick={() => {}}
          onMove={() => {}}
        />
      </div>
    </div>
  );
}

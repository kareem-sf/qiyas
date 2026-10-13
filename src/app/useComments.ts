/** The Comments tab: the comment clouds of every sheet, read again whenever the tab is shown or
 *  the drawing on screen is drawn again, and a click that shows one (switching sheet when it is
 *  on another). */
import { useEffect, useState, type RefObject } from "react";
import { listComments, type CommentInfo, type DrawingInfo } from "../api";
import { isStale } from "../shell/switcher";
import type { Batch } from "../viewport/batch";
import type { Shell } from "./useShell";
import type { Sheets } from "./useSheets";

type Deps = Pick<Shell, "sideTab" | "viewport"> &
  Pick<Sheets, "sheet" | "chooseSheet"> & {
    switchGen: RefObject<number>;
    info: DrawingInfo | null;
    batch: Batch | null;
  };

export function useComments({ sideTab, viewport, sheet, chooseSheet, switchGen, info, batch }: Deps) {
  const [comments, setComments] = useState<CommentInfo[] | null>(null);

  function refresh() {
    const gen = switchGen.current;
    listComments()
      .then((list) => {
        if (!isStale(gen, switchGen.current)) setComments(list);
      })
      .catch(() => setComments([]));
  }

  useEffect(() => {
    if (sideTab !== "comments") return;
    if (info) refresh();
    else setComments(null);
  }, [sideTab, info, batch]);

  /** Fills the view with the cloud, with room round it; another sheet's is shown whole. */
  function show(c: CommentInfo) {
    if (c.sheet.toLowerCase() !== sheet.toLowerCase()) {
      chooseSheet(c.sheet);
      return;
    }
    const [[x0, y0], [x1, y1]] = c.bounds;
    const pad = Math.max(x1 - x0, y1 - y0) * 0.6;
    viewport.current?.fitTo([
      [x0 - pad, y0 - pad],
      [x1 + pad, y1 + pad],
    ]);
  }

  return { comments, refresh: info ? refresh : undefined, show };
}

export type Comments = ReturnType<typeof useComments>;

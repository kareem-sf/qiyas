/** Drawings the OS asks Qiyas to open: a double click on a `.dwg`, `.dxf` or `.qiyas` file, at
 *  launch or while Qiyas runs (the second launch hands them over and quits). The core keeps them
 *  until the window takes them; each then opens as File, Open does (`openPath`), one after
 *  another, so a newer autosave is offered first. */
import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { OPEN_REQUESTED, takeOpenRequests } from "../api";

export function useOpenRequests(openPath: (path: string) => Promise<void>) {
  const open = useRef(openPath);
  open.current = openPath;
  const queue = useRef(Promise.resolve());

  useEffect(() => {
    let gone = false;
    let off: (() => void) | null = null;
    /** Takes what waits and opens it, after the files taken before. */
    const take = () => {
      queue.current = queue.current.then(async () => {
        const paths = (await takeOpenRequests().catch(() => null)) ?? [];
        for (const path of paths) if (!gone) await open.current(path).catch(() => {});
      });
    };
    // Listening first, then taking, so files sent in between still open.
    listen(OPEN_REQUESTED, take)
      .then((unlisten) => (gone ? unlisten() : (off = unlisten)))
      .catch(() => {})
      .finally(() => {
        if (!gone) take();
      });
    return () => {
      gone = true;
      off?.();
    };
  }, []);
}

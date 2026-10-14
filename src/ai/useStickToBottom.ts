import { useCallback, useLayoutEffect, useRef, useState } from "react";

/** How close to the end, in pixels, still counts as being at it. */
const NEAR = 32;

/** A scrolling thread that follows its newest message: while the reader is at the bottom, what is
 *  added (a streamed word, a new card) scrolls into view; once they have scrolled up to read, it
 *  leaves them where they are, and `away` says they are not at the bottom, for a "Jump to latest" button.
 *  `jump` goes to the bottom and follows again. `changed` lists what changes when the content does
 *  (always the same number of things).
 *  Put `ref` and `onScroll` on the scrolling box. */
export function useStickToBottom(changed: readonly unknown[]) {
  const box = useRef<HTMLDivElement | null>(null);
  const pinned = useRef(true);
  const [away, setAway] = useState(false);

  // A new box (the conversation came back from the log) starts at the bottom again.
  const ref = useCallback((el: HTMLDivElement | null) => {
    box.current = el;
    pinned.current = true;
    setAway(false);
  }, []);

  const toBottom = (smooth: boolean) => {
    const el = box.current;
    if (!el) return;
    el.scrollTo?.({ top: el.scrollHeight, behavior: smooth ? "smooth" : "auto" });
  };

  const onScroll = () => {
    const el = box.current;
    if (!el) return;
    pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight <= NEAR;
    setAway(!pinned.current);
  };

  // After the content changed, before it is painted: follow it if the reader is at the bottom.
  useLayoutEffect(() => {
    if (pinned.current) toBottom(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, changed);

  const jump = (smooth = true) => {
    pinned.current = true;
    setAway(false);
    toBottom(smooth);
  };

  return { ref, onScroll, away, jump };
}

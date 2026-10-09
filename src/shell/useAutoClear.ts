import { useEffect } from "react";

/** How long a status bar notice stays when nothing else clears it. */
export const NOTICE_MS = 6000;

/** Calls `clear` `ms` after `value` last changed to something other than null. */
export function useAutoClear(value: string | null, clear: () => void, ms = NOTICE_MS) {
  useEffect(() => {
    if (value === null) return;
    const timer = setTimeout(clear, ms);
    return () => clearTimeout(timer);
    // `clear` is a state setter in practice; only a new value restarts the clock.
  }, [value, ms]);
}

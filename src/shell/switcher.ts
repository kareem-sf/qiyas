/** Drawing changes in the window: every open, tab switch or close that puts another drawing on
 *  screen bumps a generation. Work started under an older generation is stale: its results are
 *  dropped, never shown on the drawing now on screen. */
export const isStale = (gen: number, current: number) => gen !== current;

/** Runs jobs one after another in the order asked, so the core switches tabs in the order the user
 *  asked for them and the last one asked is the one left on screen. A failed job does not stop
 *  the next. */
export function serial() {
  let tail: Promise<unknown> = Promise.resolve();
  return function run<T>(job: () => Promise<T>): Promise<T> {
    const next = tail.then(job);
    tail = next.catch(() => {});
    return next;
  };
}

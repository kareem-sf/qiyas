/** The recovery questions waiting their turn: one per drawing about to open (`then`), or the
 *  launch one (`then` null), each shown alone in `RecoveryDialog`, first in, first asked; inside
 *  one, the drawings Qiyas crashed with come first. Pure. */
import type { Recoverable } from "../api";

export type RecoveryAsk = { id: number; items: Recoverable[]; then: string | null };

let next = 1;

/** `q` with a question for `items` (opening `then` after), crashed ones first, each kind in the
 *  order given; `first` puts it ahead of the others. A drawing is asked about once: one already
 *  in `q` is left out (a drawing double-clicked at launch is found by the launch question and by
 *  its open), and when nothing is left `then` opens after the launch question holding it. */
export function enqueue(q: RecoveryAsk[], items: Recoverable[], then: string | null, first = false): RecoveryAsk[] {
  const asked = new Set(q.flatMap((a) => a.items.map((i) => i.drawing)));
  items = items.filter((i) => !asked.has(i.drawing));
  if (items.length === 0) {
    const k = q.findIndex((a) => a.then === null && a.items.some((i) => i.drawing === then));
    return then !== null && k >= 0 ? q.map((a, j) => (j === k ? { ...a, then } : a)) : q;
  }
  const ask = { id: next++, items: [...items.filter((i) => i.crashed), ...items.filter((i) => !i.crashed)], then };
  return first ? [ask, ...q] : [...q, ask];
}

/** `secs` (seconds since 1970) as a date and time in the user's own locale. */
export const localTime = (secs: number) =>
  new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

/** `q` with `drawing` dealt with in the question asked now; that question goes once empty. */
export function drop(q: RecoveryAsk[], drawing: string): RecoveryAsk[] {
  if (q.length === 0) return q;
  const [head, ...rest] = q;
  const items = head.items.filter((i) => i.drawing !== drawing);
  return items.length > 0 ? [{ ...head, items }, ...rest] : rest;
}

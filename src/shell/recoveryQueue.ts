/** The recovery questions waiting their turn: one per drawing about to open (`then`), or the
 *  launch one (`then` null), each shown alone in `RecoveryDialog`, first in, first asked. Pure. */
import type { Recoverable } from "../api";

export type RecoveryAsk = { id: number; items: Recoverable[]; then: string | null };

let next = 1;

/** `q` with a question for `items` (opening `then` after); `first` puts it ahead of the others. */
export function enqueue(q: RecoveryAsk[], items: Recoverable[], then: string | null, first = false): RecoveryAsk[] {
  const ask = { id: next++, items, then };
  return first ? [ask, ...q] : [...q, ask];
}

/** `q` with `drawing` dealt with in the question asked now; that question goes once empty. */
export function drop(q: RecoveryAsk[], drawing: string): RecoveryAsk[] {
  if (q.length === 0) return q;
  const [head, ...rest] = q;
  const items = head.items.filter((i) => i.drawing !== drawing);
  return items.length > 0 ? [{ ...head, items }, ...rest] : rest;
}

/** One request in flight at a time: requests made meanwhile are dropped but for the latest, which
 *  is sent when the one in flight answers. For the snap asked for on every pointer move. */
export type Coalescer<A> = {
  request(arg: A): void;
  /** Forgets the waiting request; an answer still in flight is dropped. */
  cancel(): void;
};

export function coalesce<A, R>(
  send: (arg: A) => Promise<R>,
  onResult: (result: R, arg: A) => void,
  onError: (e: unknown) => void = () => {},
): Coalescer<A> {
  let busy = false;
  let waiting: { arg: A } | null = null;
  /** Bumped by `cancel`, so an answer to a request made before it is not delivered. */
  let epoch = 0;

  function start(arg: A) {
    busy = true;
    const mine = epoch;
    send(arg).then(
      (result) => {
        if (mine === epoch) onResult(result, arg);
      },
      (e) => {
        if (mine === epoch) onError(e);
      },
    ).finally(() => {
      busy = false;
      const next = waiting;
      waiting = null;
      if (next) start(next.arg);
    });
  }

  return {
    request(arg) {
      if (busy) waiting = { arg };
      else start(arg);
    },
    cancel() {
      waiting = null;
      epoch++;
    },
  };
}

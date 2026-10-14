/** The ribbon's tooltip: one small dark card, shown after a short delay when the pointer rests on
 *  a button or when the keyboard focuses it (not when a mouse click did), with the command's name
 *  in bold, a line saying what it does and its command line alias. It hides on Escape, on
 *  blur, on leaving and on a click, never leaves the window (it is placed `fixed`, kept inside the
 *  window's edges, and goes above the button when there is no room below) and is tied to its
 *  button with `aria-describedby` while it shows. Wrap a button in `<Tip>`; without a
 *  `<TooltipLayer>` around it the button works as it is, with no tooltip. */
import { createContext, useCallback, useContext, useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";

export type TipInfo = {
  name: string;
  /** What the command does, one line. */
  text?: string;
  /** The alias typed in the command line. */
  alias?: string;
  /** What is the matter with a disabled button ("Coming soon"). */
  note?: string;
};

/** Milliseconds the pointer rests before the card shows, and the keyboard's. */
export const HOVER_DELAY = 400;
export const FOCUS_DELAY = 150;
/** The card of the next button shows at once when the pointer left a card this recently. */
const NEXT_WITHIN = 300;

type Shown = { id: string; anchor: HTMLElement; info: TipInfo };
type Api = {
  /** The id of the tip showing (the button's `aria-describedby` while it is its own). */
  shown: string | null;
  cardId: string;
  show(id: string, anchor: HTMLElement, info: TipInfo, delay: number): void;
  /** `soon`: the pointer left it for somewhere else (the next button's card shows at once). */
  hide(id?: string, soon?: boolean): void;
  keyboard: { current: boolean };
};

const TipContext = createContext<Api | null>(null);

/** Holds the one card; wrap the ribbon in it. */
export function TooltipLayer({ children }: { children: ReactNode }) {
  const cardId = useId();
  const [shown, setShown] = useState<Shown | null>(null);
  const timer = useRef<number | null>(null);
  /** The tip whose card is about to show (its delay is running). */
  const pending = useRef<string | null>(null);
  const current = useRef<Shown | null>(null);
  current.current = shown;
  /** When the pointer last left a button with a card showing. */
  const left = useRef(-Infinity);
  /** Whether the last thing the user did was a key (a focus then is the keyboard's). */
  const keyboard = useRef(false);

  const clear = () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
    pending.current = null;
  };
  /** Hides the card, or stops it from showing; with an `id`, only if it is that tip's. */
  const hide = useCallback((id?: string, soon = false) => {
    if (id !== undefined && current.current?.id !== id && pending.current !== id) return;
    if (soon && current.current) left.current = Date.now();
    clear();
    setShown(null);
  }, []);
  const show = useCallback((id: string, anchor: HTMLElement, info: TipInfo, delay: number) => {
    clear();
    const open = () => {
      clear();
      setShown({ id, anchor, info });
    };
    // Moving from one button to the next while a card shows: the next shows at once.
    if (current.current || Date.now() - left.current < NEXT_WITHIN || delay === 0) open();
    else {
      pending.current = id;
      timer.current = window.setTimeout(open, delay);
    }
  }, []);

  useEffect(() => {
    if (!shown) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") hide();
    };
    const onBlur = () => hide();
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onBlur);
    };
  }, [shown, hide]);
  useEffect(() => clear, []);
  // A focus is the keyboard's unless a pointer was the last to act (a click focuses its button too).
  useEffect(() => {
    const key = () => (keyboard.current = true);
    const pointer = () => (keyboard.current = false);
    document.addEventListener("keydown", key, true);
    document.addEventListener("pointerdown", pointer, true);
    document.addEventListener("mousedown", pointer, true);
    return () => {
      document.removeEventListener("keydown", key, true);
      document.removeEventListener("pointerdown", pointer, true);
      document.removeEventListener("mousedown", pointer, true);
    };
  }, []);

  return (
    <TipContext value={{ shown: shown?.id ?? null, cardId, show, hide, keyboard }}>
      {children}
      {shown && <Card id={cardId} shown={shown} />}
    </TipContext>
  );
}

const MARGIN = 8;

function Card({ id, shown }: { id: string; shown: Shown }) {
  const ref = useRef<HTMLDivElement>(null);
  // Placed before it is painted: under its button, centred, kept inside the window; above the
  // button when it would run off the bottom.
  useLayoutEffect(() => {
    const card = ref.current;
    if (!card) return;
    const a = shown.anchor.getBoundingClientRect();
    const w = card.offsetWidth;
    const h = card.offsetHeight;
    const left = Math.max(MARGIN, Math.min(a.left + a.width / 2 - w / 2, window.innerWidth - w - MARGIN));
    const below = a.bottom + 6;
    const top = below + h + MARGIN > window.innerHeight && a.top - h - 6 >= MARGIN ? a.top - h - 6 : below;
    card.style.left = `${left}px`;
    card.style.top = `${top}px`;
  });
  const { name, text, alias, note } = shown.info;
  return (
    <div ref={ref} id={id} role="tooltip" className="rb-tooltip">
      <b>{name}</b>
      {text && <span>{text}</span>}
      {note && <span className="rb-tooltip-note">{note}</span>}
      {alias && <kbd dir="auto">{alias}</kbd>}
    </div>
  );
}

/** Gives its button a tooltip. `children` receives the `aria-describedby` to put on the button:
 *  the card's id while this tip shows. The wrapper (not the button) hears the pointer, so a
 *  disabled button, which hears nothing, still has one. */
export function Tip({ info, children }: { info: TipInfo; children(describedBy: string | undefined): ReactNode }) {
  const api = useContext(TipContext);
  const id = useId();
  const box = useRef<HTMLSpanElement>(null);
  const hide = api?.hide;
  // Gone from the screen (another tab, a layout saved) while its card shows.
  useEffect(() => () => hide?.(id), [hide, id]);
  if (!api) return <>{children(undefined)}</>;
  const at = () => box.current!;
  return (
    <span
      ref={box}
      className="rb-tip"
      onMouseEnter={() => api.show(id, at(), info, HOVER_DELAY)}
      onMouseLeave={() => api.hide(id, true)}
      onFocus={() => {
        // Keyboard focus only: a click focuses its button too, and hides the card.
        if (api.keyboard.current) api.show(id, at(), info, FOCUS_DELAY);
      }}
      onBlur={() => api.hide(id)}
      onPointerDown={() => api.hide(id)}
      onClickCapture={() => api.hide(id)}
    >
      {children(api.shown === id ? api.cardId : undefined)}
    </span>
  );
}

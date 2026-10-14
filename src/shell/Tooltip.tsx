/** The ribbon's tooltip, on Base UI's Tooltip: one small dark card, shown after a short delay when
 *  the pointer rests on a button or when the keyboard focuses it (not when a mouse click did),
 *  with the command's name in bold, a line saying what it does and its command line alias. It
 *  hides on Escape, on blur, on leaving and on a click, never leaves the window (Base UI places it
 *  under the button, flips it above when there is no room and keeps it inside the window's edges,
 *  mirrored in Arabic) and is tied to its button with `aria-describedby` while it shows (Base UI
 *  leaves that to us, a tooltip being only supplementary). Wrap a button in `<Tip>`; without a
 *  `<TooltipLayer>` around it the button works as it is, with no tooltip. */
import { Tooltip } from "@base-ui/react/tooltip";
import { createContext, useContext, useId, useRef, useState, type ReactNode } from "react";

export type TipInfo = {
  name: string;
  /** What the command does, one line. */
  text?: string;
  /** The alias typed in the command line. */
  alias?: string;
  /** What is the matter with a disabled button ("Coming soon"). */
  note?: string;
};

/** Milliseconds the pointer rests before the card shows. */
export const HOVER_DELAY = 400;
/** The card of the next button shows at once when the pointer left a card this recently. */
const NEXT_WITHIN = 300;

/** Whether a `<TooltipLayer>` is above: a `Tip` outside one passes its button through. */
const Layer = createContext(false);

/** Holds the delays every tip shares; wrap the ribbon in it. */
export function TooltipLayer({ children }: { children: ReactNode }) {
  return (
    <Layer value>
      <Tooltip.Provider delay={HOVER_DELAY} closeDelay={0} timeout={NEXT_WITHIN}>
        {children}
      </Tooltip.Provider>
    </Layer>
  );
}

/** Gives its button a tooltip. `children` receives the `aria-describedby` to put on the button:
 *  the card's id while this tip shows. The wrapper (not the button) hears the pointer, so a
 *  disabled button, which hears nothing, still has one. */
export function Tip({ info, children }: { info: TipInfo; children(describedBy: string | undefined): ReactNode }) {
  const layered = useContext(Layer);
  const id = useId();
  const [open, setOpen] = useState(false);
  /** A press is what focused the button: a click is not a reason to show the card. */
  const pressed = useRef(false);
  if (!layered) return <>{children(undefined)}</>;
  const { name, text, alias, note } = info;
  return (
    <Tooltip.Root
      open={open}
      onOpenChange={(next, details) => {
        if (next && details.reason === "trigger-focus" && pressed.current) return;
        setOpen(next);
      }}
    >
      <Tooltip.Trigger
        className="rb-tip"
        render={<span />}
        onPointerDown={() => (pressed.current = true)}
        onBlur={() => (pressed.current = false)}
      >
        {children(open ? id : undefined)}
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Positioner className="rb-tooltip-pos" side="bottom" sideOffset={6} collisionPadding={8}>
          <Tooltip.Popup id={id} role="tooltip" className="rb-tooltip">
            <b>{name}</b>
            {text && <span>{text}</span>}
            {note && <span className="rb-tooltip-note">{note}</span>}
            {alias && <kbd dir="auto">{alias}</kbd>}
          </Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

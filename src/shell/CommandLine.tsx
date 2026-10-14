import { useId, useMemo, useState, type KeyboardEvent, type Ref } from "react";
import { useT } from "../i18n";
import { parse, suggest, type Action, type ParseState } from "../tools/commandParser";

/** Lines of history kept for ↑ and ↓. */
const HISTORY = 50;
/** Lines shown above the input. */
const LOG_LINES = 2;

type Props = {
  /** What the line waits for, the last point and the aliases (see `parse`). */
  state: ParseState;
  /** The prompt before the input, translated, e.g. "Specify first point:". */
  prompt: string;
  onAction(action: Action): void;
  /** The input, so the app can focus it (Ctrl+9). */
  inputRef?: Ref<HTMLInputElement>;
};

/** Actions an empty Enter never repeats: answers to a prompt, and those that would add a second
 *  count or send the question again. */
const NOT_REPEATED: Action["type"][] = ["point", "number", "finish", "cancel", "count", "ai"];

/** The command line above the status bar: type a command (or its alias), a point or a number and
 *  press Enter. An empty Enter repeats the last command (or finishes a measure at a point prompt);
 *  ↑ and ↓ go through the last 50 lines; typing shows the matching commands (↑, ↓, Tab, Enter);
 *  Escape cancels. The two lines above show what was typed and any error. */
export function CommandLine({ state, prompt, onAction, inputRef }: Props) {
  const t = useT();
  const id = useId();
  const [text, setText] = useState("");
  const [log, setLog] = useState<string[]>([]);
  const [history, setHistory] = useState<string[]>([]);
  /** Where ↑ and ↓ are in `history`; null while typing a new line. */
  const [recalled, setRecalled] = useState<number | null>(null);
  const [lastCommand, setLastCommand] = useState<string | null>(null);
  /** The suggestions show while typing a command name; Escape, a pick or a recall hides them. */
  const [suggesting, setSuggesting] = useState(false);
  const [active, setActive] = useState(-1);

  const suggestions = useMemo(
    () => (suggesting && state.expect === "command" ? suggest(text, state.aliases) : []),
    [suggesting, text, state.expect, state.aliases],
  );
  const open = suggestions.length > 0;

  const write = (...lines: string[]) => setLog((l) => [...l, ...lines].slice(-LOG_LINES));

  function submit(line: string) {
    setText("");
    setRecalled(null);
    setSuggesting(false);
    setActive(-1);
    let action = parse(line, state);
    let typed = line.trim();
    if (action.type === "repeat") {
      if (!lastCommand) return;
      typed = lastCommand;
      action = parse(lastCommand, state);
    }
    if (typed) setHistory((h) => (h[h.length - 1] === typed ? h : [...h, typed].slice(-HISTORY)));
    const echo = typed ? `${prompt} ${typed}` : prompt;
    if (action.type === "error") {
      write(echo, t(action.key, action.args));
      return;
    }
    write(echo);
    // COUNT <block> and AI <text> clear the repeat: Enter after them does nothing.
    if (action.type === "count" || action.type === "ai") setLastCommand(null);
    else if (!NOT_REPEATED.includes(action.type)) setLastCommand(typed);
    onAction(action);
  }

  function recall(step: -1 | 1) {
    if (history.length === 0) return;
    const at = recalled === null ? (step < 0 ? history.length - 1 : null) : recalled + step;
    if (at === null || at >= history.length) {
      setRecalled(null);
      setText("");
    } else {
      const i = Math.max(0, at);
      setRecalled(i);
      setText(history[i]);
    }
    setSuggesting(false);
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const step = e.key === "ArrowUp" ? -1 : 1;
      const n = suggestions.length;
      if (open) setActive((a) => (step > 0 ? (a + 1) % n : a <= 0 ? n - 1 : a - 1));
      else recall(step);
    } else if (e.key === "Tab" && open) {
      e.preventDefault();
      setText(`${suggestions[Math.max(0, active)].name} `);
      setSuggesting(false);
      setActive(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit(open && active >= 0 ? suggestions[active].name : text);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (open) {
        setSuggesting(false);
        setActive(-1);
        return;
      }
      setText("");
      setRecalled(null);
      onAction({ type: "cancel" });
    }
  }

  const listId = `${id}-list`;
  return (
    <section className="command-line" aria-label={t("cmd.label")}>
      <ol className="command-log" aria-live="polite">
        {log.map((line, i) => (
          <li key={i} dir="auto">
            {line}
          </li>
        ))}
      </ol>
      <div className="command-input">
        <label htmlFor={`${id}-input`}>{prompt}</label>
        <input
          ref={inputRef}
          id={`${id}-input`}
          role="combobox"
          aria-expanded={open}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={open && active >= 0 ? `${id}-opt-${active}` : undefined}
          autoComplete="off"
          spellCheck={false}
          placeholder={t("cmd.placeholder")}
          dir="auto"
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setRecalled(null);
            setSuggesting(true);
            setActive(-1);
          }}
          onKeyDown={onKeyDown}
          onBlur={() => setSuggesting(false)}
        />
        {open && (
          <ul id={listId} role="listbox" aria-label={t("cmd.suggestions")}>
            {suggestions.map((s, i) => (
              <li
                key={s.name}
                id={`${id}-opt-${i}`}
                role="option"
                aria-selected={i === active}
                // Before the input's blur, so the click lands.
                onMouseDown={(e) => {
                  e.preventDefault();
                  submit(s.name);
                }}
              >
                <span>{s.name}</span>
                {s.target && <span className="target">{s.target}</span>}
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

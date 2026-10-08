import { memo, useEffect, useRef, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { Markdown } from "./Markdown";
import { startNewOffered, TOO_LONG, undoableCard, type Item, type ToolCardItem } from "./messages";
import { ToolCard } from "./ToolCard";
import type { Assistant } from "./useAssistant";

type Props = {
  assistant: Pick<
    Assistant,
    "transcript" | "busy" | "error" | "status" | "send" | "cancel" | "clear"
  >;
  onShow(card: ToolCardItem): void;
  onUndo(card: ToolCardItem): void;
  onSettings(): void;
};

const EXAMPLES = ["ai.example1", "ai.example2", "ai.example3"];

/** The in-app assistant: the conversation, the tools it ran, and a box to write in. Memoised: the
 *  app re-renders on every pointer move. */
export const AssistantPanel = memo(function AssistantPanel({ assistant, onShow, onUndo, onSettings }: Props) {
  const t = useT();
  const { transcript, busy, error, status } = assistant;
  const [draft, setDraft] = useState("");
  const log = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLTextAreaElement>(null);

  // Keep the newest message in view.
  useEffect(() => {
    log.current?.scrollTo?.({ top: log.current.scrollHeight });
  }, [transcript, busy]);

  async function submit() {
    const text = draft.trim();
    if (!text || busy) return;
    if (await assistant.send(text)) setDraft("");
  }

  // Undo is offered on the card whose change is the assistant's newest still in the take-off.
  const undoable = undoableCard(transcript);

  // Said once per turn to screen readers, instead of reading out the streaming log.
  const [announce, setAnnounce] = useState("");
  const wasBusy = useRef(busy);
  useEffect(() => {
    if (wasBusy.current && !busy) setAnnounce(t("ai.finished"));
    if (busy) setAnnounce("");
    wasBusy.current = busy;
  }, [busy]);

  function stopText(reason: string) {
    if (reason === "cancelled") return t("ai.cancelled");
    if (reason === TOO_LONG) return t("ai.tooLong");
    if (reason === "max_tokens") return t("ai.maxTokens");
    const steps = /^stopped after (\d+) steps$/.exec(reason);
    if (steps) return t("ai.steps", { n: steps[1] });
    return `${t("ai.stopped")}: ${reason}`;
  }

  function errorText(e: string) {
    if (e === "busy") return t("ai.busy");
    if (e === "no_api_key") return t("ai.noKey");
    if (e === "no_base_url") return t("ai.noBaseUrl");
    if (e === "restarted") return t("ai.restarted");
    if (e === "crashed") return t("ai.crashed");
    return e;
  }

  function show(item: Item, i: number): ReactNode {
    switch (item.kind) {
      case "user":
        return (
          <div key={i} className="msg user" dir="auto">
            {item.text}
          </div>
        );
      case "assistant":
        return (
          <div key={i} className="msg assistant">
            <Markdown text={item.text} />
          </div>
        );
      case "thinking":
        return (
          <details key={i} className="msg thinking">
            <summary>{t("ai.thinking")}</summary>
            <p dir="auto">{item.text}</p>
          </details>
        );
      case "tool": {
        const canShow = item.handles.length > 0 || !!item.ui?.bounds;
        const canUndo = !busy && item === undoable;
        return (
          <ToolCard
            key={i}
            card={item}
            onShow={canShow ? () => onShow(item) : undefined}
            onUndo={canUndo ? () => onUndo(item) : undefined}
          />
        );
      }
      case "refused":
        return (
          <div key={i} className="msg notice refused" role="note">
            <strong className="heading">{t("ai.refused")}</strong>
            {item.text && <p dir="auto">{item.text}</p>}
            {item.category && <span className="category">{item.category}</span>}
          </div>
        );
      case "stopped":
        return (
          <div key={i} className="msg notice stopped" role="note">
            {stopText(item.reason)}
          </div>
        );
    }
  }

  const empty = transcript.items.length === 0;
  return (
    <section className="assistant-panel" aria-label={t("assistant")}>
      <div className="assistant-head">
        {status && (
          <span className="model-badge" dir="ltr" title={t("settings.model")}>
            {status.model}
          </span>
        )}
        <button
          type="button"
          className="icon gear"
          aria-haspopup="dialog"
          aria-label={t("settings")}
          title={t("settings")}
          onClick={onSettings}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="8" cy="8" r="2.2" />
            <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
            <circle cx="8" cy="8" r="4.6" />
          </svg>
        </button>
      </div>
      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>
      <div className="assistant-log" ref={log}>
        {empty ? (
          <div className="assistant-empty">
            <p>{t("ai.emptyTitle")}</p>
            <ul>
              {EXAMPLES.map((key) => (
                <li key={key}>
                  <button
                    type="button"
                    onClick={() => {
                      setDraft(t(key));
                      box.current?.focus();
                    }}
                  >
                    {t(key)}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          transcript.items.map(show)
        )}
        {busy && (
          <p className="working">
            <span className="spinner" aria-hidden="true" />
            {t("ai.working")}
          </p>
        )}
        {!busy && (startNewOffered(transcript) || error === "crashed") && (
          <button type="button" className="primary new-conversation" onClick={assistant.clear}>
            {t("ai.newConversation")}
          </button>
        )}
      </div>
      {error && (
        <div className="error" role="alert">
          {errorText(error)}
        </div>
      )}
      <form
        className="composer"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <textarea
          ref={box}
          dir="auto"
          rows={3}
          aria-label={t("ai.placeholder")}
          placeholder={t("ai.placeholder")}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            // Enter sends, Shift+Enter is a new line; not while an input method is composing.
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <div className="composer-buttons">
          <button type="button" onClick={assistant.clear} disabled={busy || empty}>
            {t("ai.clear")}
          </button>
          {busy ? (
            <button type="button" className="primary" onClick={assistant.cancel}>
              {t("ai.stop")}
            </button>
          ) : (
            <button type="submit" className="primary" disabled={!draft.trim()}>
              {t("ai.send")}
            </button>
          )}
        </div>
      </form>
    </section>
  );
});

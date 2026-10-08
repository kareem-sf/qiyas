// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AssistantPanel } from "./AssistantPanel";
import { EMPTY, type ToolCardItem, type Transcript } from "./messages";

afterEach(cleanup);

const card: ToolCardItem = {
  kind: "tool",
  id: "t1",
  name: "measure_area",
  input: { handle: 7 },
  status: "done",
  summary: "12.000 m2: wall",
  handles: [7],
  ui: null,
  mutating: true,
  undone: false,
  stale: false,
};

function setup(transcript: Transcript = EMPTY, busy = false, error: string | null = null) {
  const assistant = {
    transcript,
    busy,
    error,
    status: { provider: "mock", model: "mock", has_key: true, busy, message_count: 0 },
    send: vi.fn().mockResolvedValue(true),
    cancel: vi.fn(),
    clear: vi.fn(),
  };
  const onShow = vi.fn();
  const onUndo = vi.fn();
  render(<AssistantPanel assistant={assistant} onShow={onShow} onUndo={onUndo} onSettings={() => {}} />);
  return { assistant, onShow, onUndo };
}

test("empty state offers three examples; one fills the box", () => {
  setup();
  const examples = screen.getAllByRole("listitem");
  expect(examples).toHaveLength(3);
  fireEvent.click(screen.getByText("Count the doors on this sheet"));
  expect((screen.getByLabelText("Type your request…") as HTMLTextAreaElement).value).toBe(
    "Count the doors on this sheet",
  );
  expect((screen.getByText("Clear") as HTMLButtonElement).disabled).toBe(true);
});

test("a transcript shows messages, Markdown-lite and a tool card with Show and Undo", () => {
  const { onShow, onUndo } = setup({
    items: [
      { kind: "user", text: "Measure the wall" },
      { kind: "assistant", text: "Measuring **now**.\n\n- one `ai`\n- two <b>x</b>", finished: true },
      card,
    ],
  });
  expect(screen.getByText("Measure the wall")).toBeTruthy();
  expect(screen.getByText("now").tagName).toBe("STRONG");
  expect(screen.getByText("ai").tagName).toBe("CODE");
  // Markup in the model's text stays text.
  expect(screen.getByText("two <b>x</b>")).toBeTruthy();
  expect(document.querySelector(".msg b")).toBeNull();
  expect(screen.getByText("Measure area")).toBeTruthy();
  expect(screen.getByText("12.000 m2: wall")).toBeTruthy();
  fireEvent.click(screen.getByText("Show"));
  expect(onShow).toHaveBeenCalledWith(card);
  fireEvent.click(screen.getByText("Undo"));
  expect(onUndo).toHaveBeenCalledWith(card);
});

test("Undo is only offered on the newest change, and not while busy", () => {
  const older = { ...card, id: "t0" };
  setup({ items: [older, card] });
  expect(screen.getAllByText("Undo")).toHaveLength(1);
  cleanup();
  setup({ items: [card] }, true);
  expect(screen.queryByText("Undo")).toBeNull();
  expect(screen.getByText("Stop")).toBeTruthy();
  expect(screen.getByText("Working…")).toBeTruthy();
});

test("after an undo the card before it can be undone; a stale card says why", () => {
  const older = { ...card, id: "t0" };
  const { onUndo } = setup({ items: [older, { ...card, undone: true }] });
  expect(screen.getByText("Undone")).toBeTruthy();
  fireEvent.click(screen.getByText("Undo"));
  expect(onUndo).toHaveBeenCalledWith(older);
  cleanup();
  setup({ items: [{ ...card, stale: true }] });
  expect((screen.getByText("Undo") as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText("Can't undo: the take-off changed since")).toBeTruthy();
});

test("a crash says so and offers a new conversation; the status line announces the end", () => {
  const { assistant } = setup({ items: [{ kind: "user", text: "hi" }] }, false, "crashed");
  expect(screen.getByRole("alert").textContent).toBe("The assistant crashed; start a new conversation.");
  fireEvent.click(screen.getByText("Start a new conversation"));
  expect(assistant.clear).toHaveBeenCalled();
  expect(screen.getByRole("status").textContent).toBe("");
});

test("Enter sends and empties the box; Shift+Enter does not send", async () => {
  const { assistant } = setup();
  const box = screen.getByLabelText("Type your request…") as HTMLTextAreaElement;
  fireEvent.change(box, { target: { value: "Count the doors" } });
  fireEvent.keyDown(box, { key: "Enter", shiftKey: true });
  expect(assistant.send).not.toHaveBeenCalled();
  fireEvent.keyDown(box, { key: "Enter" });
  expect(assistant.send).toHaveBeenCalledWith("Count the doors");
  await waitFor(() => expect(box.value).toBe(""));
});

test("a refusal shows its card and a button for a new conversation", () => {
  const { assistant } = setup({
    items: [{ kind: "refused", category: null, text: "Not this one." }],
  });
  expect(screen.getByText("Request declined")).toBeTruthy();
  fireEvent.click(screen.getByText("Start a new conversation"));
  expect(assistant.clear).toHaveBeenCalled();
});

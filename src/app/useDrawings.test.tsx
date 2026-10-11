// @vitest-environment jsdom
/** `useDrawings`' recovery flows with the core (`invoke`) and the file dialogs faked: what reaches
 *  the core, what Save As is asked with, and the status bar notice. */
import { act, cleanup, renderHook } from "@testing-library/react";
import { useRef, useState, type SetStateAction } from "react";
import { afterEach, expect, test, vi } from "vitest";
import type { DrawingInfo, Recoverable } from "../api";
import { LanguageProvider, useT } from "../i18n";
import { enqueue, localTime } from "../shell/recoveryQueue";
import { useDrawings } from "./useDrawings";

const VILLA = "/jobs/villa.dwg";
const RECOVERED = "/jobs/villa (recovered).dwg";

const core = vi.hoisted(() => ({ calls: [] as string[] }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    core.calls.push(`${cmd} ${JSON.stringify(args ?? null)}`);
    switch (cmd) {
      case "recents":
      case "recoverable_drawings":
      case "take_open_requests":
        return [];
      case "get_settings":
        return { autosave_minutes: 0 };
      case "recover_autosave":
        return info(RECOVERED);
      case "open_drawing":
        return info(args!.path as string);
      default:
        return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onCloseRequested: async () => () => {} }),
}));
const dialogs = vi.hoisted(() => ({ save: vi.fn(async (_options: { defaultPath?: string }) => null) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => null),
  save: dialogs.save,
  ask: vi.fn(async () => true),
  message: vi.fn(async () => "No"),
}));

afterEach(() => {
  cleanup();
  core.calls.length = 0;
  dialogs.save.mockClear();
});

function info(path: string) {
  return {
    path,
    sheets: [{ name: "Model", block: "*Model_Space", is_model: true }],
    layers: [],
    problems: [],
    arabic_reversed: false,
    xrefs: [],
    sheet: "*Model_Space",
    hidden_layers: [],
  } as unknown as DrawingInfo;
}

const item = (more: Partial<Recoverable> = {}): Recoverable => ({
  drawing: VILLA,
  autosave: `${VILLA}.sv$`,
  saved_at: 1_760_000_000,
  drawing_at: 1_759_999_000,
  crashed: true,
  locked_elsewhere: null,
  ...more,
});

/** `useDrawings` with the window around it reduced to state and no-ops; nothing open at first. */
function useWindow(notices: string[]) {
  const [info, setInfo] = useState<DrawingInfo | null>(null);
  const switchGen = useRef(0);
  const autosaveOff = useRef(false);
  const preset = useRef(null);
  const lastPoint = useRef(null);
  const nothing = () => {};
  const none = async () => {};
  return useDrawings({
    t: useT(),
    switchGen,
    info,
    setInfo,
    setError: nothing,
    setNotice: (n: SetStateAction<string | null>) => {
      if (typeof n === "string") notices.push(n);
    },
    run: async (step: () => Promise<void>) => step(),
    settingsOpen: false,
    fileTabs: [],
    setFileTabs: nothing,
    setActiveTab: nothing,
    refreshTabs: none,
    autosaveOff,
    autosave: none,
    setDraftingState: nothing,
    sheets: { reset: nothing, clear: nothing },
    setArabicReversed: nothing,
    setSheetBlock: nothing,
    setHidden: nothing,
    redraw: none,
    takeoffState: { reset: nothing, clear: nothing },
    refreshTakeoffAndBoq: none,
    attach: { reset: nothing },
    preset,
    lastPoint,
    measure: { cancel: nothing },
  });
}

async function launch(notices: string[] = []) {
  const view = renderHook(() => useWindow(notices), { wrapper: LanguageProvider });
  // The launch effects (recents, settings, open requests) settle.
  await act(async () => {});
  return view;
}

test("Recover at launch, nothing open: Save As is offered with the recovered drawing's name, then the notice", async () => {
  const notices: string[] = [];
  const { result } = await launch(notices);
  act(() => result.current.setRecoveries((q) => enqueue(q, [item()], null, true)));

  await act(() => result.current.recoverFrom(VILLA));

  expect(core.calls).toContain(`recover_autosave {"path":"${VILLA}"}`);
  expect(dialogs.save).toHaveBeenCalledTimes(1);
  expect(dialogs.save.mock.calls[0][0].defaultPath).toBe(RECOVERED);
  expect(notices.at(-1)).toBe(`Recovered from autosave of ⁨${localTime(1_760_000_000)}⁩`);
  expect(result.current.recovery).toBeNull();
});

test("Recover with another drawing open: Save As suggests the recovered drawing, not the one before", async () => {
  const { result } = await launch();
  await act(() => result.current.openPath("/jobs/tower.dwg", false));
  act(() => result.current.setRecoveries((q) => enqueue(q, [item({ crashed: false })], null, true)));

  await act(() => result.current.recoverFrom(VILLA));

  expect(dialogs.save.mock.calls[0][0].defaultPath).toBe(RECOVERED);
});

test("Open anyway: a drawing open in another Qiyas opens here once, its autosave left alone", async () => {
  const { result } = await launch();
  const locked = item({ crashed: false, locked_elsewhere: { pid: 4242, host: "site-office-pc", started_at: 1 } });
  act(() => result.current.setRecoveries((q) => enqueue(q, [locked], VILLA)));
  core.calls.length = 0;

  await act(() => result.current.openAnyway(VILLA));

  expect(core.calls.filter((c) => c.startsWith("open_drawing"))).toEqual([`open_drawing {"path":"${VILLA}"}`]);
  expect(core.calls.some((c) => c.startsWith("discard_autosave"))).toBe(false);
  expect(result.current.recovery).toBeNull();
});

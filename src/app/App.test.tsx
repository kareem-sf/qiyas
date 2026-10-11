// @vitest-environment jsdom
/** Wiring checks for the window's composition root, the kind the type checker can't make: the
 *  hooks reach each other in the right order with the right state. The core (every IPC call
 *  through `invoke`), the file dialogs and the canvas are faked; `calls` is what reached the
 *  core, in order. */
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, expect, test, vi } from "vitest";
import { LanguageProvider } from "../i18n";
import en from "../i18n/en.json";
import App from "./App";

const PLAN = "/d/plan.dwg";

const core = vi.hoisted(() => ({
  calls: [] as string[],
  /** The drawings with an autosave newer than them. */
  autosaved: new Set<string>(),
  /** What the Open dialog answers. */
  chosen: "/d/plan.dwg",
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    core.calls.push(`${cmd} ${JSON.stringify(args ?? null)}`);
    switch (cmd) {
      case "aliases":
        return {};
      case "recents":
      case "takeoff":
      case "totals":
      case "conflicts":
      case "paper_views":
        return [];
      case "get_settings":
        return { autosave_minutes: 5, tool_palette: [{ kind: "command", label: "Divide in 3", value: "DIVIDE 3" }] };
      case "recoverable_drawings":
        return (args!.paths as string[])
          .filter((p) => core.autosaved.has(p))
          .map((drawing) => ({ drawing, autosave: `${drawing}.sv$`, saved_at: 1 }));
      case "open_drawing":
        return info(args!.path as string);
      case "drawing_info":
        return info(PLAN);
      case "get_drafting":
        return {
          osnap: true,
          snap_modes: {},
          ortho: false,
          polar: false,
          polar_step_deg: 45,
          grid: false,
          grid_spacing: 10,
          grid_snap: false,
          precision: 3,
        };
      case "batch_version":
        return 1;
      case "tabs":
        return { tabs: [{ id: 1, path: PLAN, title: "plan.dwg", dirty: false }], active: 1 };
      case "render_batch":
        // "Not modified": the window keeps the batch it has.
        return new Uint8Array([0x51, 0x49, 0x4e, 0x4d, 0, 0, 0, 0]).buffer;
      case "pick":
        return { top: 42, entity: 42, chain: [], xref: null, kind: "Line" };
      case "run_command":
        return { type: "added", handles: [7, 8, 9, 10] };
      default:
        return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onCloseRequested: async () => () => {} }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => core.chosen),
  save: vi.fn(async () => null),
  ask: vi.fn(async () => true),
  message: vi.fn(async () => "No"),
}));
/** The canvas needs WebGL; the fake keeps its props so a test can click the drawing. */
const canvas = vi.hoisted(() => ({ props: null as null | { onClick(...a: unknown[]): void } }));
vi.mock("../viewport/Viewport", () => ({
  Viewport: (props: { onClick(...a: unknown[]): void }) => {
    canvas.props = props;
    return null;
  },
}));

function info(path: string) {
  return {
    path,
    sheets: [
      { name: "Model", block: "*Model_Space", is_model: true },
      { name: "Layout1", block: "*Paper_Space", is_model: false },
    ],
    layers: [{ name: "0", color: [255, 255, 255], off: false, frozen: false, locked: false }],
    units: "millimetres",
    problems: [],
    arabic_reversed: false,
    xrefs: [],
    sheet: "*Model_Space",
    hidden_layers: [],
    current: { layer: "0", color: "ByLayer", linetype: "ByLayer", lineweight: -1 },
    text_styles: ["Standard"],
    text_style: "Standard",
    dim_styles: [],
    dim_style: "Standard",
    annotation_scale: "1:1",
    annotation_factor: 1,
    block_edit: null,
  };
}

beforeAll(() => {
  window.matchMedia ??= ((media: string) => ({ matches: false, media })) as unknown as typeof window.matchMedia;
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  HTMLDialogElement.prototype.showModal ??= function (this: HTMLDialogElement) {
    this.open = true;
  };
  HTMLDialogElement.prototype.close ??= function (this: HTMLDialogElement) {
    this.open = false;
  };
});

beforeEach(() => {
  core.calls.length = 0;
  core.autosaved.clear();
  core.chosen = PLAN;
});

afterEach(cleanup);

/** Lets the window's async work finish. */
const settle = async () => {
  for (let i = 0; i < 10; i++) await act(async () => {});
};

/** Mounts the window and forgets the calls it made on the way. */
async function mount() {
  render(
    <LanguageProvider>
      <App />
    </LanguageProvider>,
  );
  await settle();
  core.calls.length = 0;
}

const clickOpen = async () => {
  fireEvent.click(screen.getAllByRole("button", { name: en.open })[0]);
  await settle();
};

const typeLine = async (text: string) => {
  const input = document.querySelector<HTMLInputElement>("input[aria-autocomplete]")!;
  fireEvent.change(input, { target: { value: text } });
  fireEvent.keyDown(input, { key: "Enter" });
  await settle();
};

/** What the drawing's open sends to the core after the core has it: its drafting settings, its
 *  take-off, BOQ and conflicts, then its batch. */
const SHOWN = [
  "get_drafting null",
  "takeoff null",
  "totals null",
  "boq null",
  "conflicts null",
  "batch_version null",
  "tabs null",
  "render_batch {}",
];

test("open → an autosave is asked about first → Discard deletes it, then the drawing opens and shows", async () => {
  await mount();
  core.autosaved.add(PLAN);

  await clickOpen();
  // Asked, not opened.
  expect(core.calls).toEqual([`recoverable_drawings {"paths":["${PLAN}"]}`]);
  expect(screen.getByRole("heading", { name: en["recovery.title"] })).toBeTruthy();

  core.calls.length = 0;
  fireEvent.click(screen.getByRole("button", { name: en["recovery.discard"] }));
  await settle();
  // Deleted, then opened without asking again, then shown.
  expect(core.calls).toEqual([
    `discard_autosave {"path":"${PLAN}"}`,
    `open_drawing {"path":"${PLAN}"}`,
    ...SHOWN,
  ]);
  expect(screen.queryByRole("heading", { name: en["recovery.title"] })).toBeNull();
  // The status bar names the drawing on screen.
  expect(screen.getByRole("status").textContent).toBe("plan.dwg");
});

test("DIVIDE 5 answers the divide prompt once: one divide_along for the command, none for the next pick", async () => {
  await mount();
  await clickOpen();
  core.calls.length = 0;

  await typeLine("DIVIDE 5");
  expect(core.calls).toEqual([]);

  // The entity is clicked: the number typed answers the prompt, and the tool's command runs once.
  canvas.props!.onClick([10, 10], [100, 100], 1, false);
  await settle();
  const divides = () => core.calls.filter((c) => c.startsWith("run_command"));
  expect(divides()).toEqual([
    `run_command {"cmd":{"type":"divide_along","handle":42,"chain":[],"count":5}}`,
  ]);
  // Drawn and saved after it.
  expect(core.calls.slice(-1)).toEqual(["save_project null"]);

  // The next pick asks for its own number.
  canvas.props!.onClick([10, 10], [100, 100], 1, false);
  await settle();
  expect(divides()).toHaveLength(1);
  expect(screen.getByRole("form", { name: en.divide })).toBeTruthy();
});

test("hooks made before apply and onCommand reach them: TEXT's text, a tool palette's command line", async () => {
  await mount();
  await clickOpen();
  core.calls.length = 0;

  // TEXT: the in-place editor (useAnnotate) places the text through apply (useCommands).
  await typeLine("TEXT");
  canvas.props!.onClick([20, 20], [200, 200], 1, false);
  await settle();
  const value = document.querySelector<HTMLInputElement>(".text-in-place .text-value")!;
  fireEvent.change(value, { target: { value: "Hi" } });
  fireEvent.submit(value.form!);
  await settle();
  expect(core.calls.filter((c) => c.startsWith("run_command"))).toEqual([
    `run_command {"cmd":{"type":"text","at":[20,20],"height":2.5,"rotation":0,"value":"Hi","style":"Standard"}}`,
  ]);

  // A palette button (useBlocks) runs its command line through onCommand (useCommandLine).
  core.calls.length = 0;
  fireEvent.click(screen.getByRole("tab", { name: en.insert }));
  fireEvent.click(screen.getByRole("button", { name: en["block.toolPalettes"] }));
  await settle();
  fireEvent.click(screen.getByRole("button", { name: "Divide in 3" }));
  await settle();
  canvas.props!.onClick([10, 10], [100, 100], 1, false);
  await settle();
  expect(core.calls.filter((c) => c.startsWith("run_command"))).toEqual([
    `run_command {"cmd":{"type":"divide_along","handle":42,"chain":[],"count":3}}`,
  ]);
});

test("another sheet cancels the tool in use, which is made after the sheets", async () => {
  await mount();
  await clickOpen();
  const prompt = () => document.querySelector<HTMLInputElement>("input[aria-autocomplete]")!.labels![0].textContent;

  await typeLine("L");
  canvas.props!.onClick([10, 10], [100, 100], 1, false);
  await settle();
  expect(prompt()).toBe(en["draw.prompt.nextPoint"]);

  // useSheets reaches the tool's cancel through the root's measureRef.
  await typeLine("SHEET Layout1");
  expect(core.calls).toContain(`set_sheet {"block":"*Paper_Space"}`);
  expect(prompt()).toBe(en["draw.prompt.firstPoint"]);
});

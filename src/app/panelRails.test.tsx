// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import ar from "../i18n/ar.json";
import en from "../i18n/en.json";
import { LanguageProvider } from "../i18n";
import { PanelRail, usePanelRails } from "./panelRails";

const KEY = "qiyas.panels.hidden";

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.restoreAllMocks();
});

test("both panels start shown", () => {
  const { result } = renderHook(() => usePanelRails());
  expect(result.current.hidden).toEqual({ left: false, right: false });
});

test("toggle hides a panel, then shows it again, one side at a time", () => {
  const { result } = renderHook(() => usePanelRails());
  act(() => result.current.toggle("left"));
  expect(result.current.hidden).toEqual({ left: true, right: false });
  act(() => result.current.toggle("right"));
  expect(result.current.hidden).toEqual({ left: true, right: true });
  act(() => result.current.toggle("left"));
  expect(result.current.hidden).toEqual({ left: false, right: true });
});

test("the choice is kept on this computer and read back at the next start", () => {
  const first = renderHook(() => usePanelRails());
  act(() => first.result.current.toggle("right"));
  expect(JSON.parse(localStorage.getItem(KEY)!)).toEqual({ left: false, right: true });
  first.unmount();
  const second = renderHook(() => usePanelRails());
  expect(second.result.current.hidden).toEqual({ left: false, right: true });
});

test("a stored value that is not JSON, not an object or not true/false shows both panels", () => {
  for (const bad of ["{not json", "null", "42", '"left"', '{"left":"yes","right":1}']) {
    localStorage.setItem(KEY, bad);
    const { result, unmount } = renderHook(() => usePanelRails());
    expect(result.current.hidden, bad).toEqual({ left: false, right: false });
    unmount();
  }
});

test("storage that throws still folds the panel for this session", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("blocked");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("blocked");
  });
  const { result } = renderHook(() => usePanelRails());
  expect(result.current.hidden).toEqual({ left: false, right: false });
  act(() => result.current.toggle("left"));
  expect(result.current.hidden).toEqual({ left: true, right: false });
});

test("the rail is one button; its label says hide while shown and show while hidden", () => {
  const onToggle = vi.fn();
  const { rerender } = render(<PanelRail side="left" hidden={false} onToggle={onToggle} />);
  const button = screen.getByRole("button", { name: en["panels.hide.left"] });
  expect(button.getAttribute("aria-expanded")).toBe("true");
  expect(button.getAttribute("title")).toBe(en["panels.hide.left"]);
  fireEvent.click(button);
  expect(onToggle).toHaveBeenCalledTimes(1);

  rerender(<PanelRail side="left" hidden onToggle={onToggle} />);
  const shown = screen.getByRole("button", { name: en["panels.show.left"] });
  expect(shown.getAttribute("aria-expanded")).toBe("false");
});

test("each side has its own label, in English and in Arabic", () => {
  for (const [lang, strings] of [
    ["en", en],
    ["ar", ar],
  ] as const) {
    localStorage.setItem("qiyas.lang", lang);
    for (const side of ["left", "right"] as const) {
      for (const hidden of [false, true]) {
        const name = strings[`panels.${hidden ? "show" : "hide"}.${side}` as keyof typeof strings];
        const { unmount } = render(
          <LanguageProvider>
            <PanelRail side={side} hidden={hidden} onToggle={() => {}} />
          </LanguageProvider>,
        );
        const button = screen.getByRole("button", { name });
        expect(button.className).toContain(`panel-rail ${side}`);
        unmount();
      }
    }
  }
  // The four Arabic labels differ from each other and from the English ones.
  const labels = (s: typeof en) =>
    ["hide.left", "show.left", "hide.right", "show.right"].map((k) => s[`panels.${k}` as keyof typeof s]);
  expect(new Set(labels(ar)).size).toBe(4);
  expect(labels(ar)).not.toEqual(labels(en));
});

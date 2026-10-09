// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { LayerInfo } from "../api";
import { LayersPanel } from "./LayersPanel";

afterEach(cleanup);

const layer = (name: string, frozen = false): LayerInfo => ({
  name,
  color: [255, 0, 0],
  off: false,
  frozen,
  locked: false,
});

test("layers panel toggles and isolates", () => {
  const onToggle = vi.fn();
  const onIsolate = vi.fn();
  render(
    <LayersPanel
      layers={[layer("A-WALL"), layer("A-DOOR"), layer("X-OLD", true)]}
      hidden={new Set(["A-DOOR"])}
      onToggle={onToggle}
      onIsolate={onIsolate}
      onShowAll={() => {}}
    />,
  );
  const button = (name: string) => screen.getByRole("button", { name }) as HTMLButtonElement;

  const doorEye = button("Visible: A-DOOR");
  expect(doorEye.getAttribute("aria-pressed")).toBe("false");
  expect(button("Visible: A-WALL").getAttribute("aria-pressed")).toBe("true");

  fireEvent.click(doorEye);
  expect(onToggle).toHaveBeenCalledWith("A-DOOR");
  fireEvent.click(button("Isolate: A-WALL"));
  expect(onIsolate).toHaveBeenCalledWith("A-WALL");

  // A frozen layer stays hidden whatever the user does: no eye, no isolate into a blank view.
  expect(button("Visible: X-OLD").disabled).toBe(true);
  expect(button("Isolate: X-OLD").disabled).toBe(true);
});

test("while a layer change runs the buttons wait: aria-disabled, clicks ignored", () => {
  const onToggle = vi.fn();
  const onIsolate = vi.fn();
  render(
    <LayersPanel
      layers={[layer("A-WALL")]}
      hidden={new Set()}
      busy
      onToggle={onToggle}
      onIsolate={onIsolate}
      onShowAll={() => {}}
    />,
  );
  const eye = screen.getByRole("button", { name: "Visible: A-WALL" });
  const isolate = screen.getByRole("button", { name: "Isolate: A-WALL" });
  expect(eye.getAttribute("aria-disabled")).toBe("true");
  expect(isolate.getAttribute("aria-disabled")).toBe("true");
  fireEvent.click(eye);
  fireEvent.click(isolate);
  expect(onToggle).not.toHaveBeenCalled();
  expect(onIsolate).not.toHaveBeenCalled();
});

test("a white layer's swatch is Ink on the light theme and white on the dark one", () => {
  const white: LayerInfo = { ...layer("A-TEXT"), color: [255, 255, 255] };
  const props = { layers: [white], hidden: new Set<string>(), onToggle() {}, onIsolate() {}, onShowAll() {} };
  const { container, rerender } = render(<LayersPanel {...props} theme="light" />);
  const swatch = () => container.querySelector<HTMLElement>(".swatch")!;
  expect(swatch().style.background).toBe("rgb(14, 15, 18)");
  rerender(<LayersPanel {...props} theme="dark" />);
  expect(swatch().style.background).toBe("rgb(255, 255, 255)");
});

test("in Arabic, a Latin layer name is isolated so it reads in its own direction", () => {
  const name = "01-IVORY TOWER$0$A-FURN";
  document.documentElement.dir = "rtl";
  try {
    render(
      <LayersPanel
        layers={[layer(name)]}
        hidden={new Set()}
        onToggle={() => {}}
        onIsolate={() => {}}
        onShowAll={() => {}}
      />,
    );
    const shown = screen.getByText(name);
    expect(shown.tagName).toBe("BDI");
    expect(shown.closest(".name")).not.toBeNull();
  } finally {
    document.documentElement.dir = "";
  }
});

test("make current marks the current layer; an xref's layer can't be current", () => {
  const onMakeCurrent = vi.fn();
  render(
    <LayersPanel
      layers={[layer("0"), layer("A-WALL"), layer("PLAN|A-DOOR")]}
      hidden={new Set()}
      current="A-WALL"
      onToggle={() => {}}
      onIsolate={() => {}}
      onShowAll={() => {}}
      onMakeCurrent={onMakeCurrent}
    />,
  );
  const button = (name: string) => screen.getByRole("button", { name }) as HTMLButtonElement;
  expect(button("Make current: A-WALL").getAttribute("aria-pressed")).toBe("true");
  expect(button("Make current: 0").getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(button("Make current: 0"));
  expect(onMakeCurrent).toHaveBeenCalledWith("0");
  expect(button("Make current: PLAN|A-DOOR").disabled).toBe(true);
});

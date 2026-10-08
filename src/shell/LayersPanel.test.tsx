// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { LayerInfo } from "../api";
import { LayersPanel } from "./LayersPanel";

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

// @vitest-environment jsdom
import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { LayerInfo } from "../api";
import { LayersPanel } from "./LayersPanel";

const layer = (name: string): LayerInfo => ({
  name,
  color: [255, 0, 0],
  off: false,
  frozen: false,
  locked: false,
});

test("layers panel toggles and isolates", () => {
  const onToggle = vi.fn();
  const onIsolate = vi.fn();
  render(
    <LayersPanel
      layers={[layer("A-WALL"), layer("A-DOOR")]}
      hidden={new Set(["A-DOOR"])}
      onToggle={onToggle}
      onIsolate={onIsolate}
      onShowAll={() => {}}
    />,
  );
  const row = (name: string) => within(screen.getByText(name).closest("li")!);

  const doorEye = row("A-DOOR").getByRole("button", { name: "Visible" });
  expect(doorEye.getAttribute("aria-pressed")).toBe("false");
  expect(row("A-WALL").getByRole("button", { name: "Visible" }).getAttribute("aria-pressed")).toBe(
    "true",
  );

  fireEvent.click(doorEye);
  expect(onToggle).toHaveBeenCalledWith("A-DOOR");
  fireEvent.click(row("A-WALL").getByRole("button", { name: "Isolate" }));
  expect(onIsolate).toHaveBeenCalledWith("A-WALL");
});

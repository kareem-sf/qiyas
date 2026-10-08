// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { Measurement, Total } from "../api";
import { TakeoffPanel } from "./TakeoffPanel";

afterEach(cleanup);

const item = (id: string, description: string, shown: number, more: Partial<Measurement> = {}): Measurement => ({
  id,
  kind: "length",
  value: shown,
  unit: "m",
  shown,
  description,
  layer: "A-WALL",
  sheet: "*Model_Space",
  handles: [],
  points: [],
  block_name: null,
  self_intersecting: false,
  source: "user",
  note: "",
  created: "2026-10-08T00:00:00Z",
  ...more,
});

test("takeoff panel lists items and totals", () => {
  const onSelect = vi.fn();
  const onRemove = vi.fn();
  const onRename = vi.fn();
  const totals: Total[] = [{ kind: "length", unit: "m", count: 2, sum: 15 }];
  render(
    <TakeoffPanel
      items={[item("a", "Wall run", 10), item("b", "Kerb", 5, { self_intersecting: true })]}
      totals={totals}
      selectedId="a"
      onSelect={onSelect}
      onRemove={onRemove}
      onRename={onRename}
    />,
  );

  const [, body] = screen.getAllByRole("rowgroup");
  const rows = within(body).getAllByRole("row");
  expect(rows).toHaveLength(2);
  expect(rows[0].textContent).toContain("10.000");
  expect(rows[0].getAttribute("aria-current")).toBe("true");
  expect(screen.getByTitle("Self-crossing shape")).toBeTruthy();

  const totalsList = screen.getByRole("list", { name: "Totals" });
  expect(totalsList.textContent).toContain("15.000");
  expect(totalsList.textContent).toContain("2 ×");

  fireEvent.click(within(rows[1]).getByText("Kerb"));
  expect(onSelect).toHaveBeenCalledWith("b");
  fireEvent.click(screen.getByRole("button", { name: "Remove: Kerb" }));
  expect(onRemove).toHaveBeenCalledWith("b");

  fireEvent.click(screen.getByRole("button", { name: "Rename: Wall run" }));
  const input = screen.getByRole("textbox", { name: "Description" });
  fireEvent.change(input, { target: { value: "Ground floor walls" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(onRename).toHaveBeenCalledWith("a", "Ground floor walls");
});

test("counts show as whole numbers", () => {
  const totals: Total[] = [
    { kind: "area", unit: "m2", count: 1, sum: 15 },
    { kind: "count", unit: "pcs", count: 1, sum: 3 },
  ];
  render(
    <TakeoffPanel
      items={[item("c", "Doors", 3, { kind: "count", unit: "pcs" })]}
      totals={totals}
      selectedId={null}
      onSelect={() => {}}
      onRemove={() => {}}
      onRename={() => {}}
    />,
  );
  const [, body] = screen.getAllByRole("rowgroup");
  const [row] = within(body).getAllByRole("row");
  expect(row.textContent).toContain("3pcs");
  expect(row.textContent).not.toContain("3.000");
  const [area, count] = within(screen.getByRole("list", { name: "Totals" })).getAllByRole("listitem");
  expect(area.textContent).toContain("15.000");
  expect(count.textContent).toContain("3 pcs");
  expect(count.textContent).not.toContain("3.000");
});

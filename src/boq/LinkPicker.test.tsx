// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { LinkPicker } from "./LinkPicker";
import { boqItem } from "./testItems";

afterEach(cleanup);

const items = [
  boqItem("A.1", "Block walls 200 mm", "m", 100),
  boqItem("A.2", "Plaster to walls", "m2", 40),
  boqItem("B.1", "أبواب خشبية", "pcs", 3),
  boqItem("", "No code here", "m", 1),
];

test("search, arrows and Enter link the item", () => {
  const onPick = vi.fn();
  render(<LinkPicker title="Wall run" items={items} current={null} onPick={onPick} onClose={() => {}} />);
  expect(screen.getAllByRole("option")).toHaveLength(3);
  expect(screen.queryByRole("button", { name: "Unlink" })).toBeNull();
  expect(screen.getAllByRole("option")[1].textContent).toBe("A.2 · Plaster to walls · m²");

  const search = screen.getByRole("searchbox", { name: "Search…" });
  fireEvent.change(search, { target: { value: "walls" } });
  expect(screen.getAllByRole("option")).toHaveLength(2);
  fireEvent.keyDown(search, { key: "ArrowDown" });
  fireEvent.keyDown(search, { key: "Enter" });
  expect(onPick).toHaveBeenCalledWith("A.2");

  fireEvent.change(search, { target: { value: "ابواب" } });
  fireEvent.click(screen.getByRole("option"));
  expect(onPick).toHaveBeenLastCalledWith("B.1");
});

test("Unlink sends null; Escape and a click outside close", () => {
  const onPick = vi.fn();
  const onClose = vi.fn();
  render(<LinkPicker title="Wall run" items={items} current="A.1" onPick={onPick} onClose={onClose} />);
  fireEvent.click(screen.getByRole("button", { name: "Unlink" }));
  expect(onPick).toHaveBeenCalledWith(null);

  fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);
  fireEvent.pointerDown(document.body);
  expect(onClose).toHaveBeenCalledTimes(2);
  fireEvent.pointerDown(screen.getByRole("searchbox"));
  expect(onClose).toHaveBeenCalledTimes(2);
});

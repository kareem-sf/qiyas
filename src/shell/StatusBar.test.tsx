// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { Drafting } from "../api";
import { StatusBar } from "./StatusBar";

afterEach(cleanup);

const props = { cursor: null, units: "", unitsWarning: false, message: "" };

test("no Problems button without problems", () => {
  render(<StatusBar {...props} problems={[]} />);
  expect(screen.queryByRole("button", { name: /Recovered/ })).toBeNull();
});

test("the Problems button lists them; Escape closes the list and the focus comes back", async () => {
  render(<StatusBar {...props} problems={["Image a.png not found", "Unknown entity"]} />);
  const button = screen.getByRole("button", { name: "Recovered with 2 problems" });
  expect(button.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByRole("list", { name: "Problems" })).toBeNull();

  fireEvent.click(button);
  const list = screen.getByRole("list", { name: "Problems" });
  expect(list.textContent).toContain("Image a.png not found");
  expect(list.textContent).toContain("Unknown entity");
  expect(button.getAttribute("aria-expanded")).toBe("true");
  // The list takes the focus, so the keyboard can scroll it.
  await waitFor(() => expect(document.activeElement).toBe(list));

  fireEvent.keyDown(list, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("list", { name: "Problems" })).toBeNull());
  await waitFor(() => expect(document.activeElement).toBe(button));
});

test("the cursor shows in metres with an m suffix", () => {
  const { container } = render(
    <StatusBar {...props} cursor={[12500, -3000]} metresPerUnit={0.001} precision={2} problems={[]} />,
  );
  expect(container.querySelector(".cursor")?.textContent).toBe("12.50, -3.00 m");
});

test("the cursor shows in the UCS, marked as such", () => {
  const ucs = { origin: [10, 5] as [number, number], x_axis: [0, 1] as [number, number] };
  const { container } = render(
    <StatusBar {...props} cursor={[10, 6]} precision={2} problems={[]} ucs={ucs} />,
  );
  expect(container.querySelector(".cursor")?.textContent).toBe("UCS 1.00, 0.00 m");
});

test("while something waits the dots sit beside the message, which keeps its text", () => {
  const { container, rerender } = render(<StatusBar {...props} message="Opening…" waiting="scan" problems={[]} />);
  const message = container.querySelector(".message") as HTMLElement;
  expect(message.textContent).toBe("Opening…");
  expect(message.querySelector(".dot-loader.dot-scan")?.getAttribute("aria-hidden")).toBe("true");
  rerender(<StatusBar {...props} message="a.dwg" problems={[]} />);
  expect(container.querySelector(".dot-loader")).toBeNull();
});

test("OSNAP, ORTHO, POLAR and GRID are a segmented control of switches: pressed when on, pressing one flips it", () => {
  const onToggle = vi.fn();
  const drafting = {
    osnap: true,
    snap_modes: {},
    ortho: false,
    polar: true,
    polar_step_deg: 45,
    grid: false,
    grid_spacing: 1000,
    grid_snap: false,
    precision: 2,
  } as unknown as Drafting;
  render(<StatusBar {...props} problems={[]} drafting={drafting} onToggle={onToggle} />);
  const group = screen.getByRole("group", { name: "Drafting settings" });
  const names = ["OSNAP", "ORTHO", "POLAR", "GRID"];
  expect(within(group).getAllByRole("button").map((b) => b.textContent)).toEqual(names);
  expect(names.map((n) => within(group).getByRole("button", { name: n }).getAttribute("aria-pressed"))).toEqual([
    "true",
    "false",
    "true",
    "false",
  ]);
  // The tooltip says what it is and its key.
  expect(within(group).getByRole("button", { name: "ORTHO" }).getAttribute("title")).toMatch(/\(F8\)$/);
  fireEvent.click(within(group).getByRole("button", { name: "ORTHO" }));
  expect(onToggle).toHaveBeenLastCalledWith("ortho");
  // Pressing one that is on turns it off: one toggle, not two.
  fireEvent.click(within(group).getByRole("button", { name: "OSNAP" }));
  expect(onToggle).toHaveBeenLastCalledWith("osnap");
  expect(onToggle).toHaveBeenCalledTimes(2);
});

test("without a drawing the switches are off and disabled", () => {
  render(<StatusBar {...props} problems={[]} />);
  const group = screen.getByRole("group", { name: "Drafting settings" });
  for (const b of within(group).getAllByRole("button")) {
    expect((b as HTMLButtonElement).disabled).toBe(true);
    expect(b.getAttribute("aria-pressed")).toBe("false");
  }
});

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { StatusBar } from "./StatusBar";

afterEach(cleanup);

const props = { cursor: null, units: "", unitsWarning: false, message: "" };

test("no Problems button without problems", () => {
  render(<StatusBar {...props} problems={[]} />);
  expect(screen.queryByRole("button", { name: /Recovered/ })).toBeNull();
});

test("the Problems button lists them; Escape closes the list and the focus comes back", () => {
  render(<StatusBar {...props} problems={["Image a.png not found", "Unknown entity"]} />);
  const button = screen.getByRole("button", { name: "Recovered with 2 problems" });
  expect(button.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByRole("list", { name: "Problems" })).toBeNull();

  fireEvent.click(button);
  const list = screen.getByRole("list", { name: "Problems" });
  expect(list.textContent).toContain("Image a.png not found");
  expect(list.textContent).toContain("Unknown entity");
  expect(button.getAttribute("aria-expanded")).toBe("true");
  expect(document.activeElement).toBe(list);

  fireEvent.keyDown(list, { key: "Escape" });
  expect(screen.queryByRole("list", { name: "Problems" })).toBeNull();
  expect(document.activeElement).toBe(button);
});

test("the cursor shows in metres with an m suffix", () => {
  const { container } = render(
    <StatusBar {...props} cursor={[12500, -3000]} metresPerUnit={0.001} precision={2} problems={[]} />,
  );
  expect(container.querySelector(".cursor")?.textContent).toBe("12.50, -3.00 m");
});

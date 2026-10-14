// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { SegmentedChoice, SegmentedToggles } from "./Segmented";

afterEach(cleanup);

function Choice({ onChange = vi.fn() }: { onChange?(v: string): void }) {
  const [value, setValue] = useState("a");
  return (
    <SegmentedChoice
      label="Mode"
      value={value}
      onChange={(v) => {
        setValue(v);
        onChange(v);
      }}
      segments={[
        { value: "a", label: "Underlay" },
        { value: "b", label: "Import" },
        { value: "c", label: "Other", disabled: true },
      ]}
    />
  );
}

test("a choice is a radio group: one is checked, a click changes it, a disabled one cannot be chosen", () => {
  const onChange = vi.fn();
  render(<Choice onChange={onChange} />);
  const group = screen.getByRole("radiogroup", { name: "Mode" });
  expect(group.className).toContain("segmented");
  const radio = (name: string) => screen.getByRole("radio", { name });
  expect(radio("Underlay").getAttribute("aria-checked")).toBe("true");
  fireEvent.click(radio("Import"));
  expect(onChange).toHaveBeenLastCalledWith("b");
  expect(radio("Import").getAttribute("aria-checked")).toBe("true");
  expect(radio("Underlay").getAttribute("aria-checked")).toBe("false");
  fireEvent.click(radio("Underlay"));
  expect(onChange).toHaveBeenLastCalledWith("a");
  fireEvent.click(radio("Other"));
  expect(onChange).toHaveBeenCalledTimes(2);
});

test("toggles are independent switches; pressing reports the one pressed", () => {
  const onToggle = vi.fn();
  render(
    <SegmentedToggles
      label="Snaps"
      on={["x"]}
      onToggle={onToggle}
      segments={[
        { value: "x", label: "X" },
        { value: "y", label: "Y", title: "Why" },
      ]}
    />,
  );
  const group = screen.getByRole("group", { name: "Snaps" });
  expect(group.className).toContain("segmented");
  expect(screen.getByRole("button", { name: "X" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByRole("button", { name: "Y" }).getAttribute("aria-pressed")).toBe("false");
  expect(screen.getByRole("button", { name: "Y" }).title).toBe("Why");
  fireEvent.click(screen.getByRole("button", { name: "Y" }));
  expect(onToggle).toHaveBeenLastCalledWith("y");
  fireEvent.click(screen.getByRole("button", { name: "X" }));
  expect(onToggle).toHaveBeenLastCalledWith("x");
});

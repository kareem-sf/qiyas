// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import type { EntityInfo } from "../api";
import { PropertiesPanel } from "./PropertiesPanel";

afterEach(cleanup);

const door: EntityInfo = {
  handle: 0x2a,
  kind: "Insert",
  layer: "A-DOOR",
  length: null,
  area: null,
  block_name: "DOOR",
  text: null,
  inside_block: null,
  chain: [],
  block_path: [],
  color: "ACI 4",
  color_rgb: [0, 255, 255],
  linetype: "ByLayer",
  lineweight: 35,
  radius: null,
  attributes: [
    ["MARK", "D1"],
    ["FIRE", "60 min"],
  ],
  height: null,
  style: null,
};

/** The panel's label and value pairs, attributes apart. */
const rows = () =>
  [...document.querySelectorAll(".properties > div > dl > div:not(.attributes)")].map((d) => [
    d.querySelector("dt")!.textContent,
    d.querySelector("dd")!.textContent,
  ]);

test("a block reference shows its handle, colour, weight and attributes", () => {
  render(<PropertiesPanel entity={door} units="metres" precision={3} sheet="Model" />);
  expect(rows()).toEqual([
    ["Handle", "2A"],
    ["Type", "Insert"],
    ["Layer", "A-DOOR"],
    ["Colour", "ACI 4"],
    ["Linetype", "ByLayer"],
    ["Lineweight", "0.35 mm"],
    ["Block", "DOOR"],
    ["Sheet", "Model"],
  ]);
  const swatch = document.querySelector(".properties .swatch") as HTMLElement;
  expect(swatch.style.background).toBe("rgb(0, 255, 255)");
  const attributes = [...document.querySelectorAll(".attributes dd dl > div")].map((d) => d.textContent);
  expect(attributes).toEqual(["MARKD1", "FIRE60 min"]);
});

test("sizes are in metres with the precision, ByBlock and texts read out", () => {
  const arc: EntityInfo = {
    ...door,
    kind: "Arc",
    block_name: null,
    attributes: [],
    color: "ByBlock",
    lineweight: -1,
    length: 6283.1853,
    radius: 2000,
    block_path: ["PLAN", "DOOR"],
  };
  render(<PropertiesPanel entity={arc} units="millimetres" precision={2} sheet="Model" />);
  const got = Object.fromEntries(rows());
  expect(got.Colour).toBe("ByBlock");
  expect(got.Lineweight).toBe("ByLayer");
  expect(got.Length).toBe("6.28 m");
  expect(got.Radius).toBe("2.00 m");
  expect(got.Inside).toBe("PLAN › DOOR");
  expect(document.querySelector(".attributes")).toBeNull();
  cleanup();

  const text: EntityInfo = {
    ...arc,
    kind: "Text",
    text: "غرفة",
    height: 0.25,
    style: "ARIAL",
    length: null,
    radius: null,
  };
  render(<PropertiesPanel entity={text} units="metres" precision={3} sheet="Layout1" />);
  const t = Object.fromEntries(rows());
  expect([t.Text, t.Height, t.Style, t.Sheet]).toEqual(["غرفة", "0.250 m", "ARIAL", "Layout1"]);
});

test("nothing picked says how to pick, and the panel collapses", () => {
  render(<PropertiesPanel entity={null} units="metres" precision={3} sheet="Model" />);
  expect(screen.getByText("Pick one entity with Select to see its properties.")).toBeTruthy();
  const toggle = screen.getByRole("button", { name: "Properties" });
  expect(toggle.getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(toggle);
  expect(toggle.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByText("Pick one entity with Select to see its properties.")).toBeNull();
});

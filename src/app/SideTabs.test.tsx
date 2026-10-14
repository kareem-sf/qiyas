// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { Tabs } from "@base-ui/react/tabs";
import { SideTabs } from "./SideTabs";

afterEach(cleanup);

const tabs = [
  { id: "a", label: "Take-off" },
  { id: "b", label: "BOQ" },
  { id: "c", label: "Conflicts", extra: <span className="tab-count">2</span> },
  { id: "d", label: "Comments" },
  { id: "e", label: "Mizan" },
];

function ui(value: string, onChange = vi.fn()) {
  return (
    <SideTabs className="side" tabs={tabs} value={value} onChange={onChange}>
      {tabs.map((x) => (
        <Tabs.Panel key={x.id} value={x.id} keepMounted>
          panel {x.id}
        </Tabs.Panel>
      ))}
    </SideTabs>
  );
}

/** A row `room` px wide, its tabs 80 px each and the "⋯" 28: jsdom has no layout. */
function withRoom(room: number, body: () => Promise<void> | void) {
  const width = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
  const client = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");
  Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
    configurable: true,
    get(this: HTMLElement) {
      return this.hasAttribute("data-side-tab") ? 80 : 28;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "clientWidth", {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList.contains("side-tabs") ? room : 0;
    },
  });
  const done = () => {
    if (width) Object.defineProperty(HTMLElement.prototype, "offsetWidth", width);
    if (client) Object.defineProperty(HTMLElement.prototype, "clientWidth", client);
  };
  return Promise.resolve(body()).finally(done);
}

test("the tabs are one row: the chosen one is selected, the panels stay mounted, one indicator slides", () => {
  render(ui("b"));
  expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual(["Take-off", "BOQ", "Conflicts2", "Comments", "Mizan"]);
  expect(screen.getByRole("tab", { name: "BOQ" }).getAttribute("aria-selected")).toBe("true");
  expect(document.querySelectorAll(".tab-indicator")).toHaveLength(1);
  // Every panel is in the page; only the chosen one shows.
  expect(screen.getAllByRole("tabpanel", { hidden: true })).toHaveLength(5);
  expect(screen.getByRole("tabpanel").textContent).toBe("panel b");
  // The tab's name is its id for the app's own wiring.
  expect(screen.getByRole("tab", { name: "BOQ" }).id).toBe("tab-b");
});

test("a click chooses a tab; the arrows move the focus and Home and End go to the ends", async () => {
  const onChange = vi.fn();
  render(ui("a", onChange));
  fireEvent.click(screen.getByRole("tab", { name: "Comments" }));
  expect(onChange).toHaveBeenCalledWith("d");
  const first = screen.getByRole("tab", { name: "Take-off" });
  first.focus();
  fireEvent.keyDown(first, { key: "ArrowRight" });
  await act(async () => void (await new Promise((r) => setTimeout(r, 30))));
  expect(document.activeElement).toBe(screen.getByRole("tab", { name: "BOQ" }));
  fireEvent.keyDown(document.activeElement!, { key: "End" });
  await act(async () => void (await new Promise((r) => setTimeout(r, 30))));
  expect(document.activeElement).toBe(screen.getByRole("tab", { name: "Mizan" }));
});

test("a row too narrow for every tab folds the last into a ⋯ menu; they are not lost", async () => {
  const onChange = vi.fn();
  await withRoom(250, async () => {
    render(ui("a", onChange));
    const row = screen.getByRole("tablist");
    // Two fit beside the ⋯; the others are folded (disabled and hidden), and the ⋯ holds them.
    const folded = Array.from(row.querySelectorAll(".side-tab.folded")).map((t) => t.textContent);
    expect(folded).toEqual(["Conflicts2", "Comments", "Mizan"]);
    const menuTab = screen.getByRole("tab", { name: "More tabs" });
    expect(menuTab.getAttribute("aria-selected")).toBe("false");
    fireEvent.click(menuTab);
    const menu = await screen.findByRole("menu", { name: "More tabs" });
    expect(within(menu).getAllByRole("menuitemradio").map((i) => i.textContent)).toEqual(["Conflicts2", "Comments", "Mizan"]);
    fireEvent.click(within(menu).getByRole("menuitemradio", { name: "Comments" }));
    expect(onChange).toHaveBeenCalledWith("d");
  });
});

test("while the chosen tab is a folded one, the ⋯ is the selected tab", async () => {
  await withRoom(250, async () => {
    render(ui("e"));
    expect(screen.getByRole("tab", { name: "More tabs" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "Take-off" }).getAttribute("aria-selected")).toBe("false");
  });
});

test("a wide row shows every tab and has no ⋯", async () => {
  await withRoom(1000, async () => {
    render(ui("a"));
    expect(screen.queryByRole("tab", { name: "More tabs" })).toBeNull();
    expect(document.querySelectorAll(".side-tab.folded")).toHaveLength(0);
  });
});

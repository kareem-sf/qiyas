// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import type { XrefInfo } from "../api";
import { XrefManager } from "./XrefManager";

const xref = (name: string, patch: Partial<XrefInfo> = {}): XrefInfo => ({
  name,
  found: true,
  path: `/jobs/${name}.dwg`,
  overlay: false,
  unloaded: false,
  layer_count: 3,
  problems: [],
  ...patch,
});

beforeAll(() => {
  // jsdom may not implement modal dialogs.
  HTMLDialogElement.prototype.showModal ??= function (this: HTMLDialogElement) {
    this.open = true;
  };
  HTMLDialogElement.prototype.close ??= function (this: HTMLDialogElement) {
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
});

afterEach(cleanup);

test("lists each xref's status and locates only the missing ones", async () => {
  const onLocate = vi.fn(async () => {});
  const onReload = vi.fn(async () => {});
  render(
    <XrefManager
      xrefs={[
        xref("plan-arch"),
        xref("grid", { found: false, path: "..\\grid.dwg" }),
        xref("old", { found: false, unloaded: true }),
        xref("site", { overlay: true }),
      ]}
      onReload={onReload}
      onLocate={onLocate}
      onClose={() => {}}
    />,
  );
  const rows = screen.getAllByRole("row").slice(1);
  expect(rows.map((r) => r.textContent)).toEqual([
    "plan-archFound/jobs/plan-arch.dwg",
    "gridMissing..\\grid.dwgLocate…",
    "oldUnloaded/jobs/old.dwg",
    "siteOverlayFound/jobs/site.dwg",
  ]);
  expect(screen.getAllByRole("button", { name: /^Locate…/ })).toHaveLength(1);

  fireEvent.click(screen.getByRole("button", { name: "Locate…: grid" }));
  await waitFor(() => expect(onLocate).toHaveBeenCalledWith("grid"));
  fireEvent.click(screen.getByRole("button", { name: "Reload" }));
  await waitFor(() => expect(onReload).toHaveBeenCalled());
});

test("a failed reload is shown and the dialog stays", async () => {
  render(
    <XrefManager
      xrefs={[]}
      onReload={async () => {
        throw "disk gone";
      }}
      onLocate={async () => {}}
      onClose={() => {}}
    />,
  );
  expect(screen.getByText("This drawing has no external references.")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Reload" }));
  expect((await screen.findByRole("alert")).textContent).toBe("disk gone");
});

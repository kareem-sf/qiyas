// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import type { Command, Outcome, PurgeReport } from "../api";
import { PurgeDialog } from "./PurgeDialog";

beforeAll(() => {
  HTMLDialogElement.prototype.showModal ??= function (this: HTMLDialogElement) {
    this.open = true;
  };
  HTMLDialogElement.prototype.close ??= function (this: HTMLDialogElement) {
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
});

afterEach(cleanup);

const report = (more: Partial<PurgeReport> = {}): PurgeReport => ({
  dry_run: true,
  layers: ["OLD-WALL"],
  blocks: ["DOOR-90", "باب"],
  styles: [],
  linetypes: ["DASHED"],
  dimstyles: [],
  groups: [],
  group_members: 0,
  total: 4,
  limited_reason: null,
  ...more,
});

const purged = (r: PurgeReport): Outcome => ({ type: "purged", report: r });

test("Purge… lists the dry run by kind; Purge takes out the kinds ticked and says what went", async () => {
  const look = vi.fn(async (_: Command) => purged(report()));
  const apply = vi.fn(async (_: Command) => purged(report({ dry_run: false, linetypes: [], total: 3 })));
  render(<PurgeDialog look={look} apply={apply} onClose={() => {}} />);
  expect(look).toHaveBeenCalledWith({
    type: "purge",
    kinds: ["layers", "blocks", "styles", "linetypes", "dimstyles", "groups"],
    dry_run: true,
  });
  const blocks = await screen.findByRole("group", { name: "Blocks (2)" });
  expect(within(blocks).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["DOOR-90", "باب"]);
  // A name is isolated, so an Arabic one reads right in an English sentence and back.
  expect(within(blocks).getByText("باب").tagName).toBe("BDI");
  // Kinds with nothing to purge can't be ticked.
  expect((screen.getByRole("checkbox", { name: "Text styles (0)" }) as HTMLInputElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("checkbox", { name: "Linetypes (1)" }));
  fireEvent.click(screen.getByRole("button", { name: "Purge" }));
  await screen.findByText("Purged 3 entries (one undo step)");
  expect(apply).toHaveBeenCalledWith({ type: "purge", kinds: ["layers", "blocks"], dry_run: false });
  // What went, by kind; nothing left to purge.
  expect(within(screen.getByRole("group", { name: "Layers (1)" })).getByText("OLD-WALL")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Purge" })).toBeNull();
});

test("a drawing with unread data says why only layers were looked at", async () => {
  const reason = "The drawing holds 2 objects Qiyas can't read, which may use blocks, styles or linetypes: only layers were looked at";
  const look = vi.fn(async () => purged(report({ blocks: [], linetypes: [], total: 1, limited_reason: reason })));
  render(<PurgeDialog look={look} apply={vi.fn()} onClose={() => {}} />);
  expect(await screen.findByText(reason)).toBeTruthy();
});

test("nothing to purge says so; a refusal (a block edited in place) is shown", async () => {
  const empty = report({ layers: [], blocks: [], linetypes: [], total: 0 });
  const { unmount } = render(<PurgeDialog look={async () => purged(empty)} apply={vi.fn()} onClose={() => {}} />);
  expect(await screen.findByText("Nothing to purge: everything in the drawing is used")).toBeTruthy();
  expect((screen.getByRole("button", { name: "Purge" }) as HTMLButtonElement).disabled).toBe(true);
  unmount();

  const apply = vi.fn(async () => {
    throw "Close the block being edited in place first";
  });
  render(<PurgeDialog look={async () => purged(report())} apply={apply} onClose={() => {}} />);
  await screen.findByRole("group", { name: "Blocks (2)" });
  fireEvent.click(screen.getByRole("button", { name: "Purge" }));
  expect((await screen.findByRole("alert")).textContent).toBe("Close the block being edited in place first");
});

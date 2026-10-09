// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import type { ReadReport, Recoverable } from "../api";
import { LanguageProvider } from "../i18n";
import { RecoverReportDialog, RecoveryDialog } from "./RecoveryDialog";

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

const items: Recoverable[] = [
  { drawing: "/jobs/villa.dwg", autosave: "/jobs/villa.sv$", saved_at: 1_760_000_000 },
  { drawing: "/jobs/tower.dxf", autosave: "/jobs/tower.sv$", saved_at: 1_760_000_100 },
];

test("each autosave newer than its drawing is offered: recover or discard it", async () => {
  const onRecover = vi.fn(async () => {});
  const onDiscard = vi.fn(async () => {});
  const onClose = vi.fn();
  render(
    <LanguageProvider>
      <RecoveryDialog items={items} onRecover={onRecover} onDiscard={onDiscard} onClose={onClose} />
    </LanguageProvider>,
  );
  expect(screen.getByRole("heading").textContent).toBe("Recover autosaved drawings");
  const rows = screen.getAllByRole("listitem");
  expect(rows).toHaveLength(2);
  expect(rows[0].textContent).toContain("villa.dwg");
  expect(rows[0].textContent).toContain("/jobs");

  fireEvent.click(screen.getAllByRole("button", { name: "Recover" })[1]);
  await waitFor(() => expect(onRecover).toHaveBeenCalledWith("/jobs/tower.dxf"));
  fireEvent.click(screen.getAllByRole("button", { name: "Discard" })[0]);
  await waitFor(() => expect(onDiscard).toHaveBeenCalledWith("/jobs/villa.dwg"));
  expect(onClose).not.toHaveBeenCalled();
});

test("a failed recovery says why and keeps the dialog", async () => {
  render(
    <LanguageProvider>
      <RecoveryDialog
        items={items.slice(0, 1)}
        onRecover={async () => {
          throw "The autosave could not be read";
        }}
        onDiscard={async () => {}}
        onClose={() => {}}
      />
    </LanguageProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Recover" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("could not be read"));
});

test("Recover's report lists what the reader dropped, by kind", () => {
  const report: ReadReport = {
    records: 456,
    decoded: 419,
    skipped: 28,
    entities: 402,
    recovered_errors: 28,
    dropped: [
      { kind: "LINE", count: 3 },
      { kind: "record-catalog-failed", count: 14 },
    ],
  };
  render(
    <LanguageProvider>
      <RecoverReportDialog path="/jobs/damaged.dwg" report={report} onClose={() => {}} />
    </LanguageProvider>,
  );
  const text = screen.getByRole("dialog").textContent ?? "";
  expect(text).toContain("damaged.dwg");
  expect(text).toContain("402");
  expect(text).toContain("28");
  const rows = screen.getAllByRole("row");
  expect(rows.map((r) => r.textContent)).toContain("LINE3");
});

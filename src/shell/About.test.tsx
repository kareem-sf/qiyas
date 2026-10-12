// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { LanguageProvider } from "../i18n";
import { About } from "./About";

const api = vi.hoisted(() => ({ checkUpdate: vi.fn(), installUpdate: vi.fn() }));

vi.mock("../api", async (original) => ({ ...(await original<object>()), ...api }));

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

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

function show(lang: "en" | "ar" = "en") {
  localStorage.setItem("qiyas.lang", lang);
  const onClose = vi.fn();
  render(
    <LanguageProvider>
      <About theme="light" onClose={onClose} />
    </LanguageProvider>,
  );
  return onClose;
}

async function click(name: string) {
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name }));
  });
}

test("check for updates: up to date", async () => {
  api.checkUpdate.mockResolvedValue({ status: "up_to_date" });
  show();
  await click("Check for updates");
  expect(api.checkUpdate).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("status").textContent).toBe("Up to date");
  expect(api.installUpdate).not.toHaveBeenCalled();
});

test("an update found is installed only on a click, and the box stays open meanwhile", async () => {
  api.checkUpdate.mockResolvedValue({ status: "available", version: "0.2.0", notes: "Plotting to PDF." });
  let fail: (e: string) => void = () => {};
  api.installUpdate.mockReturnValue(new Promise((_, reject) => (fail = reject)));
  const onClose = show();
  await click("Check for updates");
  expect(screen.getByRole("status").textContent).toBe("Update available: 0.2.0");
  expect(screen.getByText("Plotting to PDF.")).toBeTruthy();
  expect(api.installUpdate).not.toHaveBeenCalled();

  await click("Install and restart");
  expect(api.installUpdate).toHaveBeenCalledTimes(1);
  // Installing: neither the button nor Close can be pressed, and Escape is ignored.
  expect((screen.getByRole("button", { name: "Installing…" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Close" }) as HTMLButtonElement).disabled).toBe(true);
  const escape = new Event("cancel", { cancelable: true });
  screen.getByRole("dialog", { hidden: true }).dispatchEvent(escape);
  expect(escape.defaultPrevented).toBe(true);
  expect(onClose).not.toHaveBeenCalled();

  // A failed install says why and offers Install again.
  await act(async () => fail("signature does not match"));
  expect(screen.getByRole("alert").textContent).toBe(
    "The update could not be installed: signature does not match",
  );
  expect((screen.getByRole("button", { name: "Install and restart" }) as HTMLButtonElement).disabled).toBe(false);
});

test("a build without a real key says updates are not set up, never nothing", async () => {
  api.checkUpdate.mockRejectedValue("updates_not_configured");
  show();
  await click("Check for updates");
  expect(screen.getByRole("alert").textContent).toBe("Updates are not set up in this build yet.");
  // The check can be asked again.
  expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy();
});

test("unsaved drawings stop the install; a failed check says why", async () => {
  api.checkUpdate.mockResolvedValueOnce({ status: "available", version: "0.2.0", notes: null });
  api.installUpdate.mockRejectedValue("unsaved_drawings");
  show();
  await click("Check for updates");
  await click("Install and restart");
  expect(screen.getByRole("alert").textContent).toBe("Save your drawings first: the update restarts Qiyas.");

  api.installUpdate.mockRejectedValue("no_update");
  await click("Install and restart");
  expect(screen.getByRole("alert").textContent).toBe("Check for updates again first.");
  expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy();

  api.checkUpdate.mockRejectedValue("error sending request");
  await click("Check for updates");
  expect(screen.getByRole("alert").textContent).toBe("The update check failed: error sending request");
});

test("in Arabic", async () => {
  api.checkUpdate.mockResolvedValue({ status: "available", version: "0.2.0", notes: null });
  show("ar");
  await click("البحث عن تحديثات");
  expect(screen.getByRole("status").textContent).toBe("تحديث متاح: 0.2.0");
  expect(screen.getByRole("button", { name: "التثبيت وإعادة التشغيل" })).toBeTruthy();
});

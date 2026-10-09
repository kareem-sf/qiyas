// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { NOTICE_MS, useAutoClear } from "./useAutoClear";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

test("a notice is cleared after 6 seconds, not before", () => {
  const clear = vi.fn();
  renderHook(() => useAutoClear("Saved", clear));
  vi.advanceTimersByTime(NOTICE_MS - 1);
  expect(clear).not.toHaveBeenCalled();
  vi.advanceTimersByTime(1);
  expect(clear).toHaveBeenCalledTimes(1);
});

test("no notice, nothing to clear; a new notice restarts the clock; clearing early cancels", () => {
  const clear = vi.fn();
  const { rerender } = renderHook(({ v }) => useAutoClear(v, clear), {
    initialProps: { v: null as string | null },
  });
  vi.advanceTimersByTime(NOTICE_MS * 2);
  expect(clear).not.toHaveBeenCalled();

  rerender({ v: "A" });
  vi.advanceTimersByTime(4000);
  rerender({ v: "B" });
  vi.advanceTimersByTime(4000);
  expect(clear).not.toHaveBeenCalled();
  vi.advanceTimersByTime(2000);
  expect(clear).toHaveBeenCalledTimes(1);

  rerender({ v: "C" });
  rerender({ v: null }); // the next command cleared it
  vi.advanceTimersByTime(NOTICE_MS * 2);
  expect(clear).toHaveBeenCalledTimes(1);
});

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { coalesce } from "./coalesce";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

/** A request that answers `arg * 10` after 16 ms. */
const slow = (arg: number) =>
  new Promise<number>((resolve) => setTimeout(() => resolve(arg * 10), 16));

test("one request in flight; of those made meanwhile only the latest is sent", async () => {
  const send = vi.fn(slow);
  const results: [number, number][] = [];
  const c = coalesce(send, (r, arg) => results.push([arg, r]));
  for (const arg of [1, 2, 3, 4]) c.request(arg);
  expect(send).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(16);
  // The first answered; the latest of the rest went out.
  expect(results).toEqual([[1, 10]]);
  expect(send.mock.calls.map((call) => call[0])).toEqual([1, 4]);
  await vi.advanceTimersByTimeAsync(16);
  expect(results).toEqual([
    [1, 10],
    [4, 40],
  ]);
  // Idle again: the next request goes at once.
  c.request(5);
  expect(send).toHaveBeenCalledTimes(3);
});

test("cancel drops the waiting request and the answer in flight", async () => {
  const send = vi.fn(slow);
  const onResult = vi.fn();
  const c = coalesce(send, onResult);
  c.request(1);
  c.request(2);
  c.cancel();
  await vi.advanceTimersByTimeAsync(50);
  expect(send).toHaveBeenCalledTimes(1);
  expect(onResult).not.toHaveBeenCalled();
});

test("a failed request reports its error and the next one still goes", async () => {
  let fail = true;
  const send = vi.fn((arg: number) =>
    fail ? Promise.reject(new Error("no")) : Promise.resolve(arg),
  );
  const onResult = vi.fn();
  const onError = vi.fn();
  const c = coalesce(send, onResult, onError);
  c.request(1);
  fail = false;
  c.request(2);
  await vi.runAllTimersAsync();
  expect(onError).toHaveBeenCalledTimes(1);
  expect(onResult).toHaveBeenCalledWith(2, 2);
});

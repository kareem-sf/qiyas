import { expect, test } from "vitest";
import { isStale, serial } from "./switcher";

test("work of an older generation is stale", () => {
  expect(isStale(1, 1)).toBe(false);
  expect(isStale(1, 2)).toBe(true);
});

/** A promise and the function that resolves it. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

test("two tab switches answered out of order leave the last one asked on screen", async () => {
  let current = 0;
  let shown: string | null = null;
  const answers = { a: deferred<string>(), b: deferred<string>() };
  // As App's switch: bump the generation, ask the core, show the answer unless stale.
  const switchTo = async (id: "a" | "b") => {
    const gen = ++current;
    const info = await answers[id].promise;
    if (isStale(gen, current)) return;
    shown = info;
  };
  const first = switchTo("a");
  const second = switchTo("b");
  answers.b.resolve("drawing b");
  await second;
  answers.a.resolve("drawing a");
  await first;
  expect(shown).toBe("drawing b");
});

test("switches reach the core one at a time, in the order asked", async () => {
  const run = serial();
  const order: string[] = [];
  const slow = deferred<void>();
  const a = run(async () => {
    order.push("a start");
    await slow.promise;
    order.push("a end");
  });
  const b = run(async () => {
    order.push("b");
    throw new Error("refused");
  });
  const c = run(async () => order.push("c"));
  slow.resolve();
  await a;
  await expect(b).rejects.toThrow("refused");
  await c;
  expect(order).toEqual(["a start", "a end", "b", "c"]);
});

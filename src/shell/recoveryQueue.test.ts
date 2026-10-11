import { expect, test } from "vitest";
import type { Recoverable } from "../api";
import { drop, enqueue, localTime, type RecoveryAsk } from "./recoveryQueue";

const item = (drawing: string, crashed = false): Recoverable => ({
  drawing,
  autosave: `${drawing}.sv$`,
  saved_at: 1,
  drawing_at: 2,
  crashed,
  locked_elsewhere: null,
});

test("crashed drawings are asked about first, each kind in the order found", () => {
  const q = enqueue([], [item("/a.dwg"), item("/b.dwg", true), item("/c.dwg"), item("/d.dwg", true)], null, true);
  expect(q[0].items.map((i) => i.drawing)).toEqual(["/b.dwg", "/d.dwg", "/a.dwg", "/c.dwg"]);
});

test("a drawing's own question keeps its place in line, crashed or not (Open all opens in order)", () => {
  let q: RecoveryAsk[] = [];
  q = enqueue(q, [item("/a.dwg")], "/a.dwg");
  q = enqueue(q, [item("/b.dwg", true)], "/b.dwg");
  expect(q.map((r) => r.then)).toEqual(["/a.dwg", "/b.dwg"]);
});

test("each drawing's question waits its turn; the launch one goes first", () => {
  let q: RecoveryAsk[] = [];
  q = enqueue(q, [item("/a.dwg")], "/a.dwg");
  q = enqueue(q, [item("/b.dwg")], "/b.dwg");
  q = enqueue(q, [item("/c.dwg")], null, true);
  expect(q.map((r) => r.then)).toEqual([null, "/a.dwg", "/b.dwg"]);
  expect(new Set(q.map((r) => r.id)).size).toBe(3);
});

test("a drawing dealt with leaves the question; an empty one goes, the next comes up", () => {
  let q: RecoveryAsk[] = [];
  q = enqueue(q, [item("/a.dwg"), item("/b.dwg")], null);
  q = enqueue(q, [item("/c.dwg")], "/c.dwg");
  q = drop(q, "/a.dwg");
  expect(q[0].items.map((i) => i.drawing)).toEqual(["/b.dwg"]);
  q = drop(q, "/b.dwg");
  expect(q.map((r) => r.then)).toEqual(["/c.dwg"]);
  expect(drop([], "/x.dwg")).toEqual([]);
});

test("times are the user's own locale's date and time", () => {
  const at = Date.UTC(2026, 9, 12, 11, 3) / 1000;
  expect(localTime(at)).toBe(new Date(at * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }));
});

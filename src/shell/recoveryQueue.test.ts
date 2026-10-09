import { expect, test } from "vitest";
import type { Recoverable } from "../api";
import { drop, enqueue, type RecoveryAsk } from "./recoveryQueue";

const item = (drawing: string): Recoverable => ({ drawing, autosave: `${drawing}.sv$`, saved_at: 1 });

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

import { expect, test } from "vitest";
import type { Recoverable } from "../api";
import { atLaunch, drop, enqueue, laterOpens, localTime, offered, type RecoveryAsk } from "./recoveryQueue";

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

const lockedElsewhere = (drawing: string, autosave: string | null): Recoverable => ({
  ...item(drawing),
  autosave,
  saved_at: autosave ? 1 : null,
  locked_elsewhere: { pid: 7, host: "other", started_at: 1 },
});

test("at launch only what this Qiyas can recover or discard is asked; a drawing open elsewhere waits for its open", () => {
  const found = [
    item("/a.dwg", true),
    lockedElsewhere("/b.dwg", null),
    lockedElsewhere("/c.dwg", "/c.dwg.sv$"),
    { ...item("/d.dwg"), saved_at: 3 },
  ];
  expect(atLaunch(found).map((i) => i.drawing)).toEqual(["/a.dwg", "/d.dwg"]);
});

test("Later opens the drawing asked about, unless it is open in another Qiyas (then Open anyway is the way)", () => {
  const [plain] = enqueue([], [item("/a.dwg", true)], "/a.dwg");
  expect(laterOpens(plain)).toBe("/a.dwg");
  const [locked] = enqueue([], [lockedElsewhere("/b.dwg", null)], "/b.dwg");
  expect(laterOpens(locked)).toBeNull();
  const [launch] = enqueue([], [item("/a.dwg")], null, true);
  expect(laterOpens(launch)).toBeNull();
});

test("a drawing is asked about once: a double-clicked drawing the launch question holds opens after it", () => {
  // The launch question found X first; the double click's open finds it again.
  let q = enqueue([], [item("/x.dwg"), item("/y.dwg")], null, true);
  q = enqueue(q, [item("/x.dwg")], "/x.dwg");
  expect(q).toHaveLength(1);
  expect(q[0].items.map((i) => i.drawing)).toEqual(["/x.dwg", "/y.dwg"]);
  expect(q[0].then).toBe("/x.dwg");

  // The other way round: the launch question leaves out what an open asks already.
  let r = enqueue([], [item("/x.dwg")], "/x.dwg");
  r = enqueue(r, [item("/x.dwg"), item("/y.dwg")], null, true);
  expect(r.map((a) => [a.items.map((i) => i.drawing), a.then])).toEqual([
    [["/y.dwg"], null],
    [["/x.dwg"], "/x.dwg"],
  ]);
  // Nothing left to ask: no empty question.
  expect(enqueue(r, [item("/y.dwg")], null, true)).toBe(r);
  expect(enqueue(r, [item("/x.dwg")], "/x.dwg")).toBe(r);
});

test("before an open: a crash (even older), a newer autosave, a gone drawing or another Qiyas; never an older autosave alone", () => {
  const older = item("/older.dwg"); // saved_at 1, drawing_at 2, not crashed
  const found = [
    older,
    item("/crash.dwg", true),
    { ...item("/newer.dwg"), saved_at: 3 },
    { ...item("/gone.dwg"), drawing_at: null },
    lockedElsewhere("/held.dwg", null),
  ];
  expect(offered(found).map((i) => i.drawing)).toEqual(["/crash.dwg", "/newer.dwg", "/gone.dwg", "/held.dwg"]);
});

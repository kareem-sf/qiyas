/** The draw tools: which points, typed numbers and options become which draw. Pure state machines
 *  with their prompts (i18n keys); `useTool` feeds them clicks and typed input and runs each
 *  finished draw (`spec`) as a `draw` command. */
import type { Boundary, DrawSpec, HatchPattern } from "../api";
import type { Point } from "../viewport/camera";

/** The tools with a ribbon button; the other draws (polygon, donut, ray…) are typed whole on the
 *  command line. */
export const DRAW_TOOLS = [
  "line",
  "polyline",
  "rectangle",
  "circle",
  "arc",
  "ellipse",
  "spline",
  "point",
  "hatch",
  "revcloud",
  "wipeout",
] as const;

export type DrawKind = (typeof DRAW_TOOLS)[number];

export const isDrawTool = (tool: string): tool is DrawKind =>
  (DRAW_TOOLS as readonly string[]).includes(tool);

export type DrawState = {
  kind: DrawKind;
  /** The points placed so far. */
  points: Point[];
  /** `polyline`: the bulge of the segment starting at each point, one per point. */
  bulges: number[];
  /** `circle`: "centre", "2P" or "3P"; `arc`: "3P" or "C"; `polyline`: "L" or "A" (the next
   *  segment straight or an arc). */
  mode: string;
  /** `hatch`: the next hatch's pattern (upper case, SOLID for a fill), its angle in degrees and
   *  its scale (null: the drawing's default, from its units). */
  patternName: string;
  patternAngle: number;
  patternScale: number | null;
  /** `hatch`: the option typed whose number comes next ("S" scale, "A" angle). */
  asking: "S" | "A" | null;
  /** `revcloud`: the arc length typed; null works it out from the outline. */
  arcLen: number | null;
  /** Set by the input that finished a draw; the hook runs it. */
  spec: DrawSpec | null;
  /** `hatch`: the point clicked, waiting for the ring around it (a `ring` input). */
  seek: Point | null;
  /** An i18n key for the status bar and the command line, and what it is filled in with. */
  hint: string;
  hintArgs: Record<string, string> | null;
  /** `line`: set by U; the hook undoes the line drawn last. */
  undo: boolean;
};

export type DrawInput =
  | { type: "point"; p: Point }
  /** A distance typed (a radius, the other axis, an arc length). */
  | { type: "number"; value: number }
  /** An option word typed, in capitals. */
  | { type: "option"; option: string }
  /** Enter, or a double click. */
  | { type: "finish" }
  /** What was found around `seek`, and the pattern scale the drawing's units give. */
  | { type: "ring"; found: Boundary; scale: number };

/** The hatch patterns the command line offers besides SOLID, ANSI31 and ANSI37 (the core's table). */
const TABLE = ["LINE", "NET", "DASH", "BRICK"];

const FIRST: Record<DrawKind, string> = {
  line: "draw.prompt.firstPoint",
  polyline: "draw.prompt.firstPoint",
  rectangle: "draw.prompt.firstCorner",
  circle: "draw.prompt.circleCentre",
  arc: "draw.prompt.arcStart",
  ellipse: "draw.prompt.ellipseCentre",
  spline: "draw.prompt.firstPoint",
  point: "draw.prompt.point",
  hatch: "draw.prompt.hatch",
  revcloud: "draw.prompt.firstPoint",
  wipeout: "draw.prompt.firstPoint",
};

export function initialDrawState(kind: DrawKind): DrawState {
  const s: DrawState = {
    kind,
    points: [],
    bulges: [],
    mode: kind === "circle" ? "centre" : kind === "arc" ? "3P" : "L",
    patternName: "SOLID",
    patternAngle: 0,
    patternScale: null,
    asking: null,
    arcLen: null,
    spec: null,
    seek: null,
    hint: FIRST[kind],
    hintArgs: null,
    undo: false,
  };
  return kind === "hatch" ? hatchPrompt(s) : s;
}

/** The hatch tool's prompt, naming the pattern it hatches with. */
function hatchPrompt(s: DrawState, hint = FIRST.hatch): DrawState {
  const parts = [s.patternName];
  if (s.patternName !== "SOLID") {
    if (s.patternScale !== null) parts.push(`S ${s.patternScale}`);
    if (s.patternAngle !== 0) parts.push(`A ${s.patternAngle}°`);
  }
  return { ...s, hint, hintArgs: { pattern: parts.join(", ") } };
}

/** The pattern the hatch tool's state names, `scale` when it has none of its own. */
function patternOf(s: DrawState, scale: number): HatchPattern {
  if (s.patternName === "SOLID") return "solid";
  return { custom: { name: s.patternName, angle: s.patternAngle, scale: s.patternScale ?? scale } };
}

/** The option words the command line takes now. */
export function drawOptions(s: DrawState): string[] {
  switch (s.kind) {
    case "line":
      return s.points.length >= 3 ? ["C", "U"] : s.points.length === 2 ? ["U"] : [];
    case "polyline":
      return s.points.length >= 2 ? ["A", "L", "C", "U"] : s.points.length === 1 ? ["A", "L", "U"] : [];
    case "circle":
      return s.points.length === 0 ? ["2P", "3P"] : [];
    case "arc":
      return s.points.length === 0 ? ["C"] : [];
    case "hatch":
      return ["SOLID", "ANSI31", "ANSI37", ...TABLE, "S", "A"];
    default:
      return [];
  }
}

/** The tool takes a typed number now: "distance" (a circle's radius, an ellipse's other axis, a
 *  cloud's arc length before its first point, a hatch's scale), "angle" (a hatch's, in degrees,
 *  any sign) or false. */
export function takesNumber(s: DrawState): "distance" | "angle" | false {
  if (s.kind === "hatch" && s.asking) return s.asking === "A" ? "angle" : "distance";
  const distance =
    (s.kind === "circle" && s.mode === "centre" && s.points.length === 1) ||
    (s.kind === "ellipse" && s.points.length === 2) ||
    (s.kind === "revcloud" && s.points.length === 0);
  return distance ? "distance" : false;
}

const same = (a: Point, b: Point) => a[0] === b[0] && a[1] === b[1];
const dist = (a: Point, b: Point) => Math.hypot(b[0] - a[0], b[1] - a[1]);
const degrees = (from: Point, to: Point) => (Math.atan2(to[1] - from[1], to[0] - from[0]) * 180) / Math.PI;

/** The circle through three points; null when they are in a line. */
export function circleThrough(a: Point, b: Point, c: Point): { centre: Point; radius: number } | null {
  const d = 2 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
  const scale = Math.max(1, ...[a, b, c].flatMap((p) => [Math.abs(p[0]), Math.abs(p[1])]));
  if (Math.abs(d) < 1e-12 * scale * scale) return null;
  const sq = (p: Point) => p[0] * p[0] + p[1] * p[1];
  const x = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
  const y = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
  return { centre: [x, y], radius: Math.hypot(a[0] - x, a[1] - y) };
}

/** The direction a polyline heads at its last point: along its last segment (turned by an arc's
 *  bulge), or +X with no segment yet. */
function heading(points: Point[], bulges: number[]): Point {
  const n = points.length;
  if (n < 2) return [1, 0];
  const [a, b] = [points[n - 2], points[n - 1]];
  const len = dist(a, b);
  if (len === 0) return [1, 0];
  const turn = 2 * Math.atan(bulges[n - 2] ?? 0);
  const [cx, cy] = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
  return [cx * Math.cos(turn) - cy * Math.sin(turn), cx * Math.sin(turn) + cy * Math.cos(turn)];
}

/** The bulge of the arc from `a` to `b` leaving `a` along `dir`. */
function tangentBulge(dir: Point, a: Point, b: Point): number {
  const [cx, cy] = [b[0] - a[0], b[1] - a[1]];
  const angle = Math.atan2(dir[0] * cy - dir[1] * cx, dir[0] * cx + dir[1] * cy);
  return Math.tan(angle / 2);
}

const done = (s: DrawState, spec: DrawSpec): DrawState => {
  const next: DrawState = {
    ...initialDrawState(s.kind),
    patternName: s.patternName,
    patternAngle: s.patternAngle,
    patternScale: s.patternScale,
    arcLen: s.arcLen,
    spec,
  };
  return s.kind === "hatch" ? hatchPrompt(next) : next;
};

/** The outline of a closed tool's points, its length. */
const perimeter = (points: Point[]) =>
  points.reduce((sum, p, i) => sum + dist(p, points[(i + 1) % points.length]), 0);

function finishMany(s: DrawState): DrawState {
  const p = s.points;
  switch (s.kind) {
    case "polyline":
      if (p.length < 2) return { ...initialDrawState(s.kind), hint: "draw.prompt.needTwo" };
      return done(s, { kind: "polyline", points: p, bulges: s.bulges, closed: false });
    case "spline":
      if (p.length < 2) return { ...initialDrawState(s.kind), hint: "draw.prompt.needTwo" };
      return done(s, { kind: "spline", fit_points: p });
    case "revcloud":
    case "wipeout": {
      if (p.length < 3) return { ...initialDrawState(s.kind), arcLen: s.arcLen, hint: "draw.prompt.needThree" };
      if (s.kind === "wipeout") return done(s, { kind: "wipeout", points: p });
      // About eight arcs a side of a square when no length was typed.
      const arcLen = s.arcLen ?? perimeter(p) / 32;
      return done(s, { kind: "revcloud", points: p, arc_len: arcLen });
    }
    default:
      return initialDrawState(s.kind);
  }
}

function addPoint(s: DrawState, q: Point): DrawState {
  const p = [...s.points, q];
  const last = s.points[s.points.length - 1];
  switch (s.kind) {
    case "line":
      // The whole chain is kept, for C (close) and U (undo).
      if (!last) return { ...s, points: p, hint: "draw.prompt.nextPoint" };
      if (same(last, q)) return s;
      return { ...s, points: p, hint: "draw.prompt.lineNext", spec: { kind: "line", a: last, b: q } };
    case "polyline": {
      if (last && same(last, q)) return s;
      const bulges = [...s.bulges];
      if (last && s.mode === "A") bulges[bulges.length - 1] = tangentBulge(heading(s.points, s.bulges), last, q);
      return { ...s, points: p, bulges: [...bulges, 0], hint: "draw.prompt.polylineNext" };
    }
    case "rectangle":
      if (!last) return { ...s, points: p, hint: "draw.prompt.otherCorner" };
      if (last[0] === q[0] || last[1] === q[1]) return s;
      return done(s, { kind: "rectangle", a: last, b: q });
    case "circle":
      if (s.mode === "centre") {
        if (!last) return { ...s, points: p, hint: "draw.prompt.radius" };
        return circleRadius(s, dist(last, q));
      }
      if (s.mode === "2P") {
        if (!last) return { ...s, points: p, hint: "draw.prompt.secondPoint" };
        if (same(last, q)) return s;
        const centre: Point = [(last[0] + q[0]) / 2, (last[1] + q[1]) / 2];
        return done(s, { kind: "circle", centre, radius: dist(last, q) / 2 });
      }
      if (p.length < 3) {
        return { ...s, points: p, hint: p.length === 1 ? "draw.prompt.secondPoint" : "draw.prompt.thirdPoint" };
      }
      {
        const c = circleThrough(p[0], p[1], p[2]);
        if (!c) return { ...initialDrawState(s.kind), hint: "draw.prompt.inLine" };
        return done(s, { kind: "circle", centre: c.centre, radius: c.radius });
      }
    case "arc":
      if (s.mode === "C") {
        if (p.length === 1) return { ...s, points: p, hint: "draw.prompt.arcCentreStart" };
        if (p.length === 2) {
          if (same(p[0], p[1])) return s;
          return { ...s, points: p, hint: "draw.prompt.arcEnd" };
        }
        if (same(p[0], q)) return s;
        const [centre, start] = p;
        return done(s, {
          kind: "arc",
          centre,
          radius: dist(centre, start),
          start: degrees(centre, start),
          end: degrees(centre, q),
        });
      }
      if (p.length < 3) {
        return { ...s, points: p, hint: p.length === 1 ? "draw.prompt.arcSecond" : "draw.prompt.arcEnd" };
      }
      if (!circleThrough(p[0], p[1], p[2])) return { ...initialDrawState(s.kind), hint: "draw.prompt.inLine" };
      return done(s, { kind: "three_point_arc", a: p[0], b: p[1], c: p[2] });
    case "ellipse":
      if (p.length === 1) return { ...s, points: p, hint: "draw.prompt.axisEnd" };
      if (p.length === 2) {
        if (same(p[0], p[1])) return s;
        return { ...s, points: p, hint: "draw.prompt.otherAxis" };
      }
      return ellipseAxis(s, dist(s.points[0], q));
    case "point":
      return { ...s, spec: { kind: "point", p: q } };
    case "hatch":
      return { ...s, seek: q, asking: null, hint: "draw.prompt.hatchSeeking", hintArgs: null };
    case "spline":
    case "revcloud":
    case "wipeout":
      if (last && same(last, q)) return s;
      return { ...s, points: p, hint: "draw.prompt.morePoints" };
  }
}

function circleRadius(s: DrawState, radius: number): DrawState {
  if (!(radius > 0)) return s;
  return done(s, { kind: "circle", centre: s.points[0], radius });
}

function ellipseAxis(s: DrawState, other: number): DrawState {
  const [centre, end] = s.points;
  const major: Point = [end[0] - centre[0], end[1] - centre[1]];
  const len = Math.hypot(major[0], major[1]);
  if (!(other > 0)) return s;
  if (other <= len) return done(s, { kind: "ellipse", centre, major, ratio: other / len });
  // The other axis is the longer: it is the major one, square to the first.
  const turned: Point = [(-major[1] / len) * other, (major[0] / len) * other];
  return done(s, { kind: "ellipse", centre, major: turned, ratio: len / other });
}

function option(s: DrawState, word: string): DrawState {
  if (!drawOptions(s).includes(word)) return s;
  switch (s.kind) {
    case "line": {
      const p = s.points;
      if (word === "C") {
        // The closing segment, then a new chain.
        return { ...initialDrawState(s.kind), spec: { kind: "line", a: p[p.length - 1], b: p[0] } };
      }
      // U: the last segment goes (the hook undoes it) and its start is the point to go on from.
      const points = p.slice(0, -1);
      return { ...s, points, undo: true, hint: points.length > 1 ? "draw.prompt.lineNext" : "draw.prompt.nextPoint" };
    }
    case "polyline": {
      if (word === "A" || word === "L") return { ...s, mode: word };
      if (word === "U") {
        const points = s.points.slice(0, -1);
        const bulges = s.bulges.slice(0, -1);
        if (bulges.length > 0) bulges[bulges.length - 1] = 0;
        return { ...s, points, bulges, hint: points.length > 0 ? "draw.prompt.polylineNext" : FIRST.polyline };
      }
      // C: close, the closing segment an arc in arc mode.
      const bulges = [...s.bulges];
      const [first, last] = [s.points[0], s.points[s.points.length - 1]];
      if (s.mode === "A") bulges[bulges.length - 1] = tangentBulge(heading(s.points, s.bulges), last, first);
      return done(s, { kind: "polyline", points: s.points, bulges, closed: true });
    }
    case "circle":
      return { ...s, mode: word, hint: "draw.prompt.firstPoint" };
    case "arc":
      return { ...s, mode: word, hint: "draw.prompt.arcCentre" };
    case "hatch":
      if (word === "S" || word === "A") {
        return { ...s, asking: word, hint: word === "S" ? "draw.prompt.hatchScale" : "draw.prompt.hatchAngle", hintArgs: null };
      }
      return hatchPrompt({ ...s, patternName: word, asking: null });
    default:
      return s;
  }
}

export function drawReducer(state: DrawState, input: DrawInput): DrawState {
  // A finished draw (or an undo) is pending for one state only: the hook sees it once.
  const s = { ...state, spec: null, undo: false };
  switch (input.type) {
    case "point":
      // A hatch waits for its ring before the next click.
      if (s.seek) return s;
      return addPoint(s, input.p);
    case "number": {
      const takes = takesNumber(s);
      if (!Number.isFinite(input.value) || !takes || (takes === "distance" && !(input.value > 0))) {
        return s;
      }
      if (s.kind === "hatch") {
        const typed = s.asking === "A" ? { patternAngle: input.value } : { patternScale: input.value };
        return hatchPrompt({ ...s, ...typed, asking: null });
      }
      if (s.kind === "circle") return circleRadius(s, input.value);
      if (s.kind === "ellipse") return ellipseAxis(s, input.value);
      return { ...s, arcLen: input.value };
    }
    case "option":
      return option(s, input.option);
    case "finish":
      return finishMany(s);
    case "ring": {
      if (!s.seek) return s;
      const found = input.found;
      if (found.found !== "ring" || found.ring.length < 3) {
        const hint = found.found === "too_dense" ? "draw.prompt.zoomIn" : "draw.prompt.noBoundary";
        return { ...s, seek: null, hint, hintArgs: null };
      }
      return done(s, {
        kind: "hatch",
        boundary_handles: [],
        points: found.ring,
        holes: found.holes,
        pattern: patternOf(s, input.scale),
      });
    }
  }
}

/** Steps a circle is drawn with in previews. */
const CIRCLE_STEPS = 64;

/** Points along the arc of `bulge` from `a` to `b`, `a` left out, `b` included. */
function bulgePoints(a: Point, b: Point, bulge: number): Point[] {
  if (Math.abs(bulge) < 1e-9) return [b];
  const len = dist(a, b);
  if (len === 0) return [b];
  const sweep = 4 * Math.atan(bulge);
  const r = (len * (1 + bulge * bulge)) / (4 * Math.abs(bulge));
  const h = (len * (1 - bulge * bulge)) / (4 * bulge);
  const [dx, dy] = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
  const c: Point = [(a[0] + b[0]) / 2 - dy * h, (a[1] + b[1]) / 2 + dx * h];
  const a0 = Math.atan2(a[1] - c[1], a[0] - c[0]);
  const n = Math.max(2, Math.ceil((Math.abs(sweep) / (2 * Math.PI)) * CIRCLE_STEPS));
  const out: Point[] = [];
  for (let i = 1; i <= n; i++) {
    const t = a0 + (sweep * i) / n;
    out.push([c[0] + r * Math.cos(t), c[1] + r * Math.sin(t)]);
  }
  out[out.length - 1] = b;
  return out;
}

function circlePoints(c: Point, r: number, from = 0, sweep = 2 * Math.PI): Point[] {
  const n = Math.max(2, Math.ceil((Math.abs(sweep) / (2 * Math.PI)) * CIRCLE_STEPS));
  return Array.from({ length: n + 1 }, (_, i) => {
    const t = from + (sweep * i) / n;
    return [c[0] + r * Math.cos(t), c[1] + r * Math.sin(t)] as Point;
  });
}

/** Segments `x0 y0 x1 y1` through `path`. */
function segments(path: Point[]): Float64Array {
  const out = new Float64Array(4 * Math.max(0, path.length - 1));
  for (let i = 1; i < path.length; i++) out.set([...path[i - 1], ...path[i]], 4 * (i - 1));
  return out;
}

/** What the tool would draw with the next point at `cursor`, as segments `x0 y0 x1 y1` in drawing
 *  coordinates for the overlay; empty before its first point. */
export function drawPreview(s: DrawState, cursor: Point | null): Float64Array {
  const p = s.points;
  if (p.length === 0 || !cursor) return new Float64Array();
  const last = p[p.length - 1];
  switch (s.kind) {
    case "line":
      return segments([last, cursor]);
    case "polyline": {
      const path: Point[] = [p[0]];
      for (let i = 1; i < p.length; i++) path.push(...bulgePoints(p[i - 1], p[i], s.bulges[i - 1] ?? 0));
      const bulge = s.mode === "A" ? tangentBulge(heading(p, s.bulges), last, cursor) : 0;
      path.push(...bulgePoints(last, cursor, bulge));
      return segments(path);
    }
    case "rectangle": {
      const [a, b] = [p[0], cursor];
      return segments([a, [b[0], a[1]], b, [a[0], b[1]], a]);
    }
    case "circle": {
      if (s.mode === "centre") return segments(circlePoints(p[0], dist(p[0], cursor)));
      if (s.mode === "2P") {
        const c: Point = [(p[0][0] + cursor[0]) / 2, (p[0][1] + cursor[1]) / 2];
        return segments(circlePoints(c, dist(p[0], cursor) / 2));
      }
      const c = p.length === 2 ? circleThrough(p[0], p[1], cursor) : null;
      return c ? segments(circlePoints(c.centre, c.radius)) : segments([...p, cursor]);
    }
    case "arc": {
      if (s.mode === "C") {
        if (p.length === 1) return segments([p[0], cursor]);
        const [c, start] = p;
        const r = dist(c, start);
        const a0 = Math.atan2(start[1] - c[1], start[0] - c[0]);
        const a1 = Math.atan2(cursor[1] - c[1], cursor[0] - c[0]);
        const sweep = (((a1 - a0) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
        return segments(circlePoints(c, r, a0, sweep));
      }
      const c = p.length === 2 ? circleThrough(p[0], p[1], cursor) : null;
      if (!c) return segments([...p, cursor]);
      const angle = (q: Point) => Math.atan2(q[1] - c.centre[1], q[0] - c.centre[0]);
      const ccw = (from: number, to: number) => (((to - from) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
      const [a0, a1, a2] = [angle(p[0]), angle(p[1]), angle(cursor)];
      // Counter-clockwise from the first through the second, or the other way round.
      const sweep = ccw(a0, a1) < ccw(a0, a2) ? ccw(a0, a2) : ccw(a0, a2) - 2 * Math.PI;
      return segments(circlePoints(c.centre, c.radius, a0, sweep));
    }
    case "ellipse": {
      if (p.length === 1) return segments([p[0], cursor]);
      const [c, end] = p;
      const major: Point = [end[0] - c[0], end[1] - c[1]];
      const len = Math.hypot(major[0], major[1]);
      const minor = dist(c, cursor);
      const [ux, uy] = [major[0] / len, major[1] / len];
      return segments(
        Array.from({ length: CIRCLE_STEPS + 1 }, (_, i) => {
          const t = (2 * Math.PI * i) / CIRCLE_STEPS;
          return [
            c[0] + ux * len * Math.cos(t) - uy * minor * Math.sin(t),
            c[1] + uy * len * Math.cos(t) + ux * minor * Math.sin(t),
          ] as Point;
        }),
      );
    }
    case "revcloud":
    case "wipeout":
      return segments(p.length >= 2 ? [...p, cursor, p[0]] : [...p, cursor]);
    case "spline":
      return segments([...p, cursor]);
    default:
      return new Float64Array();
  }
}

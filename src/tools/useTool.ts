/** The measure tools: which clicks become which take-off command. The rules live in a pure reducer;
 *  the hook does the async picking around it and hands each finished command to the app. */
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import type { Command, Drafting, EntityInfo, Picked, Snap, SnapKind } from "../api";
import type { Target } from "../viewport/batch";
import type { Bounds, Point } from "../viewport/camera";
import { markerSegments } from "../viewport/snapMarker";
import type { Tool } from "../viewport/Viewport";
import { coalesce } from "./coalesce";
import { constrain, gridRound, NO_TRACK, type Track } from "./tracking";

/** Two clicks this close in time and on screen are a double click. */
const DOUBLE_CLICK_MS = 300;
const DOUBLE_CLICK_PX = 4;
/** Picking reaches this many CSS pixels around the cursor. */
const PICK_PX = 6;
/** Object snaps reach this many CSS pixels around the cursor. */
const SNAP_PX = 10;

export type ToolState = {
  tool: Tool;
  /** Points placed so far by `distance` or `area`, in drawing coordinates. */
  points: Point[];
  /** Set by the event that finished a measure; the hook runs it. */
  pendingCommand: Command | null;
  /** An i18n key for the status bar, or "". */
  hint: string;
  /** Values for the `{name}` slots of `hint`. */
  hintArgs: Record<string, string> | null;
  /** What `select` picked, waiting to be measured: the entity itself, or with `whole` the
   *  top-level block reference it sits in. `path` names the blocks along its chain. `toggled` is
   *  true when the last click swapped `whole`, so a double click can take that swap back. */
  selected: { picked: Picked; path: string[]; whole: boolean; toggled: boolean } | null;
  lastClick: { screen: Point; time: number } | null;
  /** `divide` and `measureAlong`: the entity clicked, waiting for the number of parts or the
   *  step to be typed. */
  asking: { kind: "divide" | "measureAlong"; target: Target } | null;
  /** `zoomWindow`: the first corner typed, waiting for the other. */
  corner: Point | null;
  /** Set by the event that gave `zoomWindow` its second corner; the hook zooms to it. */
  pendingZoom: Bounds | null;
};

export type ToolEvent =
  | {
      type: "click";
      world: Point;
      screen: Point;
      time: number;
      /** What is under the click (`select` and `count` only), and its info: for `count` the
       *  top-level entity's, for `select` the picked entity's inside its blocks. */
      picked: Picked | null;
      info: EntityInfo | null;
      /** `world` is an object snap's point: placed as it is, without ortho or polar. */
      snapped?: boolean;
      /** The kind of that snap, for `idPoint`. */
      snapKind?: SnapKind | null;
      /** Ortho and polar tracking for `distance` and `area`'s second and later points. */
      track?: Track;
    }
  | { type: "finish" }
  | { type: "escape" }
  | { type: "tool"; tool: Tool }
  /** The number of parts (`divide`) or the step in drawing units (`measureAlong`) was typed. */
  | { type: "answer"; value: number };

const PROMPT: Record<Tool, string> = {
  select: "measure.clickEntity",
  distance: "measure.needTwo",
  area: "measure.needThree",
  count: "measure.clickBlock",
  zoomWindow: "zoom.firstCorner",
  geometry: "inquiry.clickGeometry",
  divide: "inquiry.clickDivide",
  measureAlong: "inquiry.clickAlong",
  idPoint: "inquiry.clickPoint",
};

/** The tools that act on the entity clicked. */
const picksEntity = (tool: Tool) =>
  tool === "select" ||
  tool === "count" ||
  tool === "geometry" ||
  tool === "divide" ||
  tool === "measureAlong";

/** `select` starts quiet, so the status bar shows the file name. */
export const initialToolState = (tool: Tool): ToolState => ({
  tool,
  points: [],
  pendingCommand: null,
  hint: tool === "select" ? "" : PROMPT[tool],
  hintArgs: null,
  selected: null,
  lastClick: null,
  asking: null,
  corner: null,
  pendingZoom: null,
});

const samePick = (a: Picked, b: Picked) =>
  a.top === b.top && a.entity === b.entity && a.chain.join(",") === b.chain.join(",");

/** What a `select` choice draws and measures. */
export function selectedTarget(selected: ToolState["selected"]): Target | null {
  if (!selected) return null;
  const { picked, whole } = selected;
  return whole || picked.chain.length === 0
    ? { handle: picked.top, chain: [] }
    : { handle: picked.entity, chain: picked.chain };
}

/** The status bar line for a `select` choice: which block it is in, or which block it is. */
function selectHint(s: ToolState, selected: NonNullable<ToolState["selected"]>): ToolState {
  const { path, whole } = selected;
  if (path.length === 0) return { ...s, selected, hint: "", hintArgs: null };
  return whole
    ? { ...s, selected, hint: "measure.wholeBlock", hintArgs: { name: path[0] } }
    : { ...s, selected, hint: "measure.insideBlock", hintArgs: { name: path[path.length - 1] } };
}

function measureSelected(s: ToolState): ToolState {
  const target = selectedTarget(s.selected);
  if (!target) return s;
  const pendingCommand: Command = { type: "measure_entity", handle: target.handle, chain: target.chain };
  return { ...s, selected: null, hint: "", hintArgs: null, pendingCommand };
}

function finish(s: ToolState): ToolState {
  if (s.tool === "select") return measureSelected(s);
  const need = s.tool === "area" ? 3 : 2;
  if (s.tool !== "distance" && s.tool !== "area") return s;
  if (s.points.length < need) return { ...s, hint: PROMPT[s.tool] };
  const type = s.tool === "area" ? "measure_area" : "measure_distance";
  return { ...s, points: [], lastClick: null, pendingCommand: { type, points: s.points } };
}

export function toolReducer(state: ToolState, event: ToolEvent): ToolState {
  // A command or a zoom is pending for one state only: the hook sees it once.
  const s = { ...state, pendingCommand: null, pendingZoom: null };
  switch (event.type) {
    case "tool":
      return { ...initialToolState(event.tool), hint: PROMPT[event.tool] };
    case "escape":
      return initialToolState(s.tool);
    case "finish":
      return finish(s);
    case "answer": {
      const asking = s.asking;
      if (!asking || !(event.value > 0) || !Number.isFinite(event.value)) return s;
      const { handle, chain } = asking.target;
      const pendingCommand: Command =
        asking.kind === "divide"
          ? { type: "divide_along", handle, chain, count: Math.round(event.value) }
          : { type: "measure_along", handle, chain, step: event.value };
      return { ...s, asking: null, hint: PROMPT[s.tool], pendingCommand };
    }
    case "click": {
      const last = s.lastClick;
      const double =
        last !== null &&
        event.time - last.time < DOUBLE_CLICK_MS &&
        Math.hypot(event.screen[0] - last.screen[0], event.screen[1] - last.screen[1]) <=
          DOUBLE_CLICK_PX;
      const clicked = { ...s, lastClick: { screen: event.screen, time: event.time } };
      switch (s.tool) {
        case "distance":
        case "area": {
          if (double) return { ...finish(s), lastClick: null };
          const prev = s.points.length > 0 ? s.points[s.points.length - 1] : null;
          const point = event.snapped ? event.world : constrain(prev, event.world, event.track ?? NO_TRACK);
          return { ...clicked, points: [...s.points, point] };
        }
        case "select": {
          const picked = event.picked;
          if (picked === null) return { ...clicked, selected: null, hint: "", hintArgs: null };
          const current = s.selected;
          if (current && samePick(current.picked, picked)) {
            // Inside a block, a slow second click swaps the entity for the whole block and back;
            // a double click (or Enter) measures. Elsewhere the second click measures.
            if (picked.chain.length > 0 && !double) {
              return selectHint(clicked, { ...current, whole: !current.whole, toggled: true });
            }
            // The first click of a double click swapped: measure what was shown before the pair.
            const whole = double && current.toggled ? !current.whole : current.whole;
            return measureSelected({ ...clicked, selected: { ...current, whole } });
          }
          const path = event.info?.block_path ?? [];
          return selectHint(clicked, { picked, path, whole: false, toggled: false });
        }
        case "count":
          // The second click of a double click would count the same block twice.
          if (double) return clicked;
          if (event.picked === null) return { ...clicked, hint: PROMPT.count };
          if (!event.info?.block_name) return { ...clicked, hint: "measure.notABlock" };
          return {
            ...clicked,
            hint: PROMPT.count,
            pendingCommand: { type: "count_block", block_name: event.info.block_name, layer: null },
          };
        case "geometry": {
          if (double) return clicked;
          const picked = event.picked;
          if (picked === null) return { ...clicked, hint: PROMPT.geometry };
          const pendingCommand: Command = {
            type: "measure_geometry",
            handle: picked.entity,
            chain: picked.chain,
            at: event.world,
          };
          return { ...clicked, hint: PROMPT.geometry, pendingCommand };
        }
        case "divide":
        case "measureAlong": {
          const picked = event.picked;
          if (picked === null) return { ...clicked, asking: null, hint: PROMPT[s.tool] };
          const target = { handle: picked.entity, chain: picked.chain };
          const hint = s.tool === "divide" ? "inquiry.askCount" : "inquiry.askStep";
          return { ...clicked, asking: { kind: s.tool, target }, hint };
        }
        case "zoomWindow": {
          // Typed corners (a drag zooms in the viewport itself), in any order.
          const a = s.corner;
          if (!a) return { ...clicked, corner: event.world, hint: "zoom.otherCorner" };
          const b = event.world;
          const pendingZoom: Bounds = [
            [Math.min(a[0], b[0]), Math.min(a[1], b[1])],
            [Math.max(a[0], b[0]), Math.max(a[1], b[1])],
          ];
          return { ...clicked, corner: null, hint: PROMPT.zoomWindow, pendingZoom };
        }
        case "idPoint":
          return {
            ...clicked,
            pendingCommand: { type: "id_point", p: event.world, kind: event.snapKind ?? null },
          };
        default:
          return clicked;
      }
    }
  }
}

export type ToolApi = {
  pick(x: number, y: number, tol: number): Promise<Picked | null>;
  /** The object snap within `tol` of (x, y); `from` is the previous point. */
  snap(x: number, y: number, tol: number, from: Point | null): Promise<Snap | null>;
  entityInfo(handle: number, chain?: number[]): Promise<EntityInfo>;
  /** Applies a finished command. */
  run(cmd: Command): void;
  /** Shows the window `zoomWindow` was given by two typed corners. */
  zoomTo(bounds: Bounds): void;
  onError(e: unknown): void;
};

/** The tools that place points, and so snap and track. */
const placesPoints = (tool: Tool) => tool === "distance" || tool === "area";
/** The tools that snap: those that place points, and `idPoint`. */
const snaps = (tool: Tool) => placesPoints(tool) || tool === "idPoint";

type SnapAsk = { p: Point; tol: number; from: Point | null; wpp: number };

/** Tool state for `tool`, following it as it changes. `drafting` gives the snaps, ortho, polar and
 *  grid snap the point tools place their points with (none while it is null). */
export function useTool(tool: Tool, api: ToolApi, drafting: Drafting | null = null) {
  const [state, dispatch] = useReducer(toolReducer, tool, initialToolState);
  /** The cursor in drawing coordinates as it is, and drawing units per CSS pixel there. */
  const [raw, setRaw] = useState<{ p: Point; wpp: number } | null>(null);
  /** The object snap under the cursor, and the zoom it was found at. */
  const [hover, setHover] = useState<{ snap: Snap; wpp: number } | null>(null);
  const apiRef = useRef(api);
  apiRef.current = api;
  const snapper = useRef(
    coalesce(
      (ask: SnapAsk) => apiRef.current.snap(ask.p[0], ask.p[1], ask.tol, ask.from),
      (snap, ask) => setHover(snap ? { snap, wpp: ask.wpp } : null),
      // A failed snap is no snap; the click asks again and reports its own error.
      () => setHover(null),
    ),
  );
  const snapping = snaps(tool) && !!drafting?.osnap;
  const track: Track = drafting
    ? { ortho: drafting.ortho, polar: drafting.polar, polarStepDeg: drafting.polar_step_deg }
    : NO_TRACK;
  const prev = state.points.length > 0 ? state.points[state.points.length - 1] : null;

  useEffect(() => {
    if (state.tool !== tool) dispatch({ type: "tool", tool });
  }, [tool]);

  useEffect(() => {
    if (snapping) return;
    snapper.current.cancel();
    setHover(null);
  }, [snapping]);

  useEffect(() => {
    if (state.pendingCommand) api.run(state.pendingCommand);
  }, [state.pendingCommand]);

  useEffect(() => {
    if (state.pendingZoom) api.zoomTo(state.pendingZoom);
  }, [state.pendingZoom]);

  /** Where an unsnapped point goes: to the grid when grid snap is on. */
  const onGrid = (p: Point): Point =>
    drafting?.grid_snap ? gridRound(p, drafting.grid_spacing) : p;

  function onMove(world: Point, worldPerPixel: number) {
    setRaw({ p: world, wpp: worldPerPixel });
    if (snapping) snapper.current.request({ p: world, tol: SNAP_PX * worldPerPixel, from: prev, wpp: worldPerPixel });
  }

  async function onClick(world: Point, screen: Point, worldPerPixel: number) {
    const time = performance.now();
    if (snaps(tool)) {
      let found: Snap | null = null;
      try {
        if (snapping) found = await api.snap(world[0], world[1], SNAP_PX * worldPerPixel, prev);
      } catch (e) {
        api.onError(e);
        return;
      }
      const at = found ? found.point : onGrid(world);
      dispatch({
        type: "click",
        world: at,
        screen,
        time,
        picked: null,
        info: null,
        snapped: !!found,
        snapKind: found?.kind ?? null,
        track,
      });
      return;
    }
    let picked: Picked | null = null;
    let info: EntityInfo | null = null;
    try {
      if (picksEntity(tool)) {
        picked = await api.pick(world[0], world[1], PICK_PX * worldPerPixel);
      }
      if (tool === "count" && picked !== null) info = await api.entityInfo(picked.top);
      // Inside a block, the block names go to the status bar.
      if (tool === "select" && picked !== null && picked.chain.length > 0) {
        info = await api.entityInfo(picked.entity, picked.chain);
      }
    } catch (e) {
      api.onError(e);
      return;
    }
    dispatch({ type: "click", world, screen, time, picked, info });
  }

  const snap = snapping ? (hover?.snap ?? null) : null;
  /** Where a click would place the next point now. */
  const cursor = useMemo((): Point | null => {
    if (!raw) return null;
    if (snap) return snap.point;
    return placesPoints(tool) ? constrain(prev, onGrid(raw.p), track) : raw.p;
  }, [raw, snap, tool, prev, drafting]);
  const marker = useMemo(
    () => (snap && hover ? markerSegments(snap.point, snap.kind, hover.wpp) : null),
    [snap, hover],
  );

  return {
    points: state.points,
    /** See `cursor` above: snapped, on the grid or tracked. */
    cursor,
    /** The object snap under the cursor, for the status bar. */
    snap,
    /** Its marker, segments in drawing coordinates for the overlay. */
    marker,
    onMove,
    hint: state.hint,
    hintArgs: state.hintArgs,
    /** What `select` would measure now, or what `divide` and `measureAlong` will place points
     *  along, to draw highlighted. */
    highlighted: selectedTarget(state.selected) ?? state.asking?.target ?? null,
    /** `divide` or `measureAlong` waits for its number: which one. */
    asking: state.asking?.kind ?? null,
    /** `zoomWindow` has its first corner and waits for the other. */
    corner: state.corner,
    /** Hands the typed number of parts or step (drawing units) to `divide` or `measureAlong`. */
    answer: (value: number) => dispatch({ type: "answer", value }),
    onClick,
    /** A point typed on the command line, for `distance`, `area`, `idPoint` and the corners of
     *  `zoomWindow`: placed as typed, without snaps, grid, ortho or polar, and never half of a
     *  double click. */
    typePoint: (p: Point) =>
      dispatch({
        type: "click",
        world: p,
        screen: [Number.NaN, Number.NaN],
        time: performance.now(),
        picked: null,
        info: null,
        snapped: true,
        snapKind: null,
      }),
    finish: () => dispatch({ type: "finish" }),
    escape: () => dispatch({ type: "escape" }),
  };
}

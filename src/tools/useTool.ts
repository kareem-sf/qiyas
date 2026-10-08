/** The measure tools: which clicks become which take-off command. The rules live in a pure reducer;
 *  the hook does the async picking around it and hands each finished command to the app. */
import { useEffect, useReducer } from "react";
import type { Command, EntityInfo, Picked } from "../api";
import type { Target } from "../viewport/batch";
import type { Point } from "../viewport/camera";
import type { Tool } from "../viewport/Viewport";

/** Two clicks this close in time and on screen are a double click. */
const DOUBLE_CLICK_MS = 300;
const DOUBLE_CLICK_PX = 4;
/** Picking reaches this many CSS pixels around the cursor. */
const PICK_PX = 6;

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
    }
  | { type: "finish" }
  | { type: "escape" }
  | { type: "tool"; tool: Tool };

const PROMPT: Record<Tool, string> = {
  select: "measure.clickEntity",
  distance: "measure.needTwo",
  area: "measure.needThree",
  count: "measure.clickBlock",
  zoomWindow: "",
};

/** `select` starts quiet, so the status bar shows the file name. */
export const initialToolState = (tool: Tool): ToolState => ({
  tool,
  points: [],
  pendingCommand: null,
  hint: tool === "select" ? "" : PROMPT[tool],
  hintArgs: null,
  selected: null,
  lastClick: null,
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
  // A command is pending for one state only: the hook sees it once.
  const s = { ...state, pendingCommand: null };
  switch (event.type) {
    case "tool":
      return { ...initialToolState(event.tool), hint: PROMPT[event.tool] };
    case "escape":
      return initialToolState(s.tool);
    case "finish":
      return finish(s);
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
        case "area":
          return double
            ? { ...finish(s), lastClick: null }
            : { ...clicked, points: [...s.points, event.world] };
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
        default:
          return clicked;
      }
    }
  }
}

export type ToolApi = {
  pick(x: number, y: number, tol: number): Promise<Picked | null>;
  entityInfo(handle: number, chain?: number[]): Promise<EntityInfo>;
  /** Applies a finished command. */
  run(cmd: Command): void;
  onError(e: unknown): void;
};

/** Tool state for `tool`, following it as it changes. */
export function useTool(tool: Tool, api: ToolApi) {
  const [state, dispatch] = useReducer(toolReducer, tool, initialToolState);

  useEffect(() => {
    if (state.tool !== tool) dispatch({ type: "tool", tool });
  }, [tool]);

  useEffect(() => {
    if (state.pendingCommand) api.run(state.pendingCommand);
  }, [state.pendingCommand]);

  async function onClick(world: Point, screen: Point, worldPerPixel: number) {
    const time = performance.now();
    let picked: Picked | null = null;
    let info: EntityInfo | null = null;
    try {
      if (tool === "select" || tool === "count") {
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

  return {
    points: state.points,
    hint: state.hint,
    hintArgs: state.hintArgs,
    /** What `select` would measure now, to draw highlighted. */
    highlighted: selectedTarget(state.selected),
    onClick,
    finish: () => dispatch({ type: "finish" }),
    escape: () => dispatch({ type: "escape" }),
  };
}

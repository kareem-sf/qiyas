/** The measure tools: which clicks become which take-off command. The rules live in a pure reducer;
 *  the hook does the async picking around it and hands each finished command to the app. */
import { useEffect, useReducer } from "react";
import type { Command, EntityInfo } from "../api";
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
  /** Entity picked by `select`, waiting for a second click to be measured. */
  highlighted: number | null;
  lastClick: { screen: Point; time: number } | null;
};

export type ToolEvent =
  | {
      type: "click";
      world: Point;
      screen: Point;
      time: number;
      /** Entity under the click (`select` and `count` only), and its info (`count` only). */
      picked: number | null;
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
  highlighted: null,
  lastClick: null,
});

function finish(s: ToolState): ToolState {
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
        case "select":
          if (event.picked !== null && event.picked === s.highlighted) {
            const handle = event.picked;
            return { ...clicked, highlighted: null, pendingCommand: { type: "measure_entity", handle } };
          }
          return { ...clicked, highlighted: event.picked };
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
  pick(x: number, y: number, tol: number): Promise<number | null>;
  entityInfo(handle: number): Promise<EntityInfo>;
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
    let picked: number | null = null;
    let info: EntityInfo | null = null;
    try {
      if (tool === "select" || tool === "count") {
        picked = await api.pick(world[0], world[1], PICK_PX * worldPerPixel);
      }
      if (tool === "count" && picked !== null) info = await api.entityInfo(picked);
    } catch (e) {
      api.onError(e);
      return;
    }
    dispatch({ type: "click", world, screen, time, picked, info });
  }

  return {
    points: state.points,
    hint: state.hint,
    highlighted: state.highlighted,
    onClick,
    finish: () => dispatch({ type: "finish" }),
    escape: () => dispatch({ type: "escape" }),
  };
}

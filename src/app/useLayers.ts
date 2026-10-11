/** The layers panel and what goes with it: hiding and isolating layers, their properties, layer
 *  states and the translator, groups, and what new entities are drawn with. */
import { useCallback, useMemo, useRef, useState, type RefObject } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  drawingInfo,
  drawingLayers,
  runCommand,
  setLayerHidden,
  type Command,
  type Current,
  type DrawingInfo,
  type LayerProp,
} from "../api";
import { isStale } from "../shell/switcher";

type Deps = {
  switchGen: RefObject<number>;
  info: DrawingInfo | null;
  setInfo(info: DrawingInfo): void;
  setError(error: string | null): void;
  run(step: () => Promise<void>): Promise<void>;
  autosave(): Promise<void>;
  redraw(): Promise<void>;
  refreshTakeoffAndBoq(): Promise<void>;
  hidden: Set<string>;
  setHidden(hidden: Set<string>): void;
};

export function useLayers({
  switchGen,
  info,
  setInfo,
  setError,
  run,
  autosave,
  redraw,
  refreshTakeoffAndBoq,
  hidden,
  setHidden,
}: Deps) {
  /** LAYSTATE's and LAYTRANS's dialogs are open. */
  const [layerStatesOpen, setLayerStatesOpen] = useState(false);
  const [translatorOpen, setTranslatorOpen] = useState(false);
  /** GROUP's entities are picked: the command waiting for the group's name. */
  const [groupAsk, setGroupAsk] = useState<Extract<Command, { type: "create_group" }> | null>(null);
  /** Clicking a grouped entity selects its group (AutoCAD's PICKSTYLE); Ctrl picks one alone. */
  const [groupSelect, setGroupSelect] = useState(true);
  /** True while a layer change talks to the core; clicks meanwhile are dropped (the panel shows its
   *  buttons waiting, `layersBusy`), so they can't interleave. */
  const layerBusy = useRef(false);
  /** The same, for the layers panel to show: its buttons wait and the cursor says so. */
  const [layersBusy, setLayersBusy] = useState(false);

  /** Runs a layer, group or property command (its error goes to the caller), then reads the
   *  drawing again: the batch, the take-off, the layers, groups and hidden layers. */
  async function runDrawingCommand(cmd: Command) {
    const gen = switchGen.current;
    const outcome = await runCommand(cmd);
    await redraw();
    await refreshTakeoffAndBoq();
    const next = await drawingInfo();
    if (!isStale(gen, switchGen.current)) {
      setInfo(next);
      setHidden(new Set(next.hidden_layers));
    }
    await autosave();
    return outcome;
  }

  /** `runDrawingCommand` with its error in the status bar. */
  const runLayerCommand = (cmd: Command) =>
    run(async () => {
      await runDrawingCommand(cmd);
    });

  /** The group the top-level entity `handle` is in, while group selection is on. */
  function groupOf(handle: number) {
    if (!groupSelect) return null;
    return info?.groups?.find((g) => g.selectable && g.handles.includes(handle)) ?? null;
  }

  /** Asks for a drawing and reads its layers, for the layer translator's standard. */
  async function loadLayerStandard() {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return null;
    return { path, layers: await drawingLayers(path) };
  }

  /** Reads the drawing's info again (layers, groups, current state) and the hidden layers. */
  async function syncInfo() {
    const gen = switchGen.current;
    const next = await drawingInfo();
    if (isStale(gen, switchGen.current)) return;
    setInfo(next);
    setHidden(new Set(next.hidden_layers));
  }

  /** Moves the hidden set to `target`, one core call per changed layer. Whatever happens, the panel
   *  shows the calls that went through and the canvas is redrawn from the core. */
  async function applyHidden(target: Set<string>) {
    if (layerBusy.current) return;
    layerBusy.current = true;
    setLayersBusy(true);
    const done = new Set(hidden);
    let failure: string | null = null;
    try {
      for (const l of info?.layers ?? []) {
        const hide = target.has(l.name);
        if (done.has(l.name) === hide) continue;
        await setLayerHidden(l.name, hide);
        if (hide) done.add(l.name);
        else done.delete(l.name);
      }
    } catch (e) {
      failure = String(e);
    }
    setHidden(done);
    try {
      await redraw();
    } catch (e) {
      failure ??= String(e);
    }
    setError(failure);
    layerBusy.current = false;
    setLayersBusy(false);
  }

  // Stable between pointer moves, so the memoised layers panel doesn't re-render with the cursor.
  const toggleLayer = useCallback(
    (name: string) => {
      const next = new Set(hidden);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      applyHidden(next);
    },
    [hidden, info],
  );

  /** Shows only `name`: hides every other layer and unhides it. */
  const isolateLayer = useCallback(
    (name: string) =>
      applyHidden(new Set(info?.layers.map((l) => l.name).filter((n) => n !== name))),
    [hidden, info],
  );

  const showAll = useCallback(() => applyHidden(new Set()), [hidden, info]);

  /** One property of the drawing's layer `name` (one undo step). */
  const setLayerProp = useCallback(
    (name: string, prop: LayerProp, value: boolean | string | number) =>
      runLayerCommand({ type: "set_layer", name, prop, value }),
    [info],
  );

  // The layers panel's handlers, stable between pointer moves so the memoised panel doesn't
  // render again with the cursor.
  const renameLayer = useCallback(
    (from: string, to: string) => runLayerCommand({ type: "rename_layer", from, to }),
    [info],
  );
  const deleteLayer = useCallback((name: string) => runLayerCommand({ type: "delete_layer", name }), [info]);
  const newLayer = useCallback((name: string) => runLayerCommand({ type: "new_layer", name }), [info]);
  const openLayerStates = useCallback(() => setLayerStatesOpen(true), []);
  const openTranslator = useCallback(() => setTranslatorOpen(true), []);

  /** The viewport `viewport` freezes `layer` inside it, or thaws it. */
  const setViewportFrozen = useCallback(
    (viewport: number, layer: string, frozen: boolean) =>
      runLayerCommand({ type: "set_viewport_frozen", viewport: { handle: viewport, chain: [] }, layer, frozen }),
    [info],
  );

  /** What new entities are drawn with, from the palette (not an undo step). */
  const setCurrentState = useCallback(
    (current: Current) => {
      const gen = switchGen.current;
      run(async () => {
        await runCommand({ type: "set_current", current });
        const next = await drawingInfo();
        if (isStale(gen, switchGen.current)) return;
        setInfo(next);
        await autosave();
      });
    },
    [info],
  );

  /** The drawing's own layers (not its xrefs'), for the palette. */
  const ownLayers = useMemo(() => info?.layers.map((l) => l.name).filter((n) => !n.includes("|")) ?? [], [info]);

  /** Draws new entities on layer `name` from now on (saved with the project, not an undo step). */
  const makeCurrent = useCallback(
    (name: string) => {
      if (!info) return;
      const gen = switchGen.current;
      run(async () => {
        await runCommand({ type: "set_current", current: { ...info.current, layer: name } });
        const next = await drawingInfo();
        if (isStale(gen, switchGen.current)) return;
        setInfo(next);
        await autosave();
      });
    },
    [info],
  );

  return {
    layerStatesOpen,
    setLayerStatesOpen,
    translatorOpen,
    setTranslatorOpen,
    groupAsk,
    setGroupAsk,
    groupSelect,
    setGroupSelect,
    layersBusy,
    runDrawingCommand,
    runLayerCommand,
    groupOf,
    loadLayerStandard,
    syncInfo,
    applyHidden,
    toggleLayer,
    isolateLayer,
    showAll,
    setLayerProp,
    renameLayer,
    deleteLayer,
    newLayer,
    openLayerStates,
    openTranslator,
    setViewportFrozen,
    setCurrentState,
    ownLayers,
    makeCurrent,
  };
}

export type Layers = ReturnType<typeof useLayers>;

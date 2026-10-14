/** What the select tool picked: the entity the properties palette shows and edits, and the image
 *  whose brightness, contrast and fade it shows. */
import { useCallback, useEffect, useState } from "react";
import {
  entityInfo,
  imageDisplay,
  runCommand,
  type DrawingInfo,
  type EntityInfo,
  type EntityProp,
  type ImageDisplay,
} from "../api";
import type { Tool } from "../shell/Ribbon";
import type { Measure } from "./shared";
import type { Layers } from "./useLayers";
import type { Sheets } from "./useSheets";

type Deps = Pick<Sheets, "batch" | "redraw"> &
  Pick<Layers, "runLayerCommand"> & {
    info: DrawingInfo | null;
    tool: Tool;
    measure: Pick<Measure, "highlighted" | "picks">;
  };

export function useSelection({ info, tool, measure, batch, redraw, runLayerCommand }: Deps) {
  /** The image the select tool has chosen, and its brightness, contrast and fade. */
  const [imageChosen, setImageChosen] = useState<{ handle: number; display: ImageDisplay } | null>(
    null,
  );
  /** What the select tool picked, for the properties palette. */
  const [pickedInfo, setPickedInfo] = useState<EntityInfo | null>(null);
  /** Two or more entities picked together with the select tool (a Shift click, a box, a group),
   *  each one's info; null otherwise. */
  const [pickedMany, setPickedMany] = useState<EntityInfo[] | null>(null);

  /** The palette's edit of the picked entity (a drawing's own, at the top of the sheet). */
  const setPickedProperty = useCallback(
    (prop: EntityProp, value: boolean | string | number | [number, number]) => {
      if (!pickedInfo) return;
      runLayerCommand({ type: "set_property", target: { handle: pickedInfo.handle, chain: [] }, prop, value });
    },
    [pickedInfo, info],
  );

  /** Sets the chosen image's brightness, contrast and fade (one undo step), then draws it. */
  async function setChosenDisplay(handle: number, display: ImageDisplay) {
    await runCommand({ type: "set_image_display", handle, ...display });
    await redraw();
  }

  // An image chosen with the select tool shows its brightness, contrast and fade; read again when
  // the batch changes (a set, an undo or a redo).
  const chosen = measure.highlighted;
  const chosenHandle = chosen && chosen.chain.length === 0 ? chosen.handle : null;
  useEffect(() => {
    if (chosenHandle === null) {
      setImageChosen(null);
      return;
    }
    let gone = false;
    imageDisplay(chosenHandle)
      .then((display) => !gone && setImageChosen(display ? { handle: chosenHandle, display } : null))
      .catch(() => !gone && setImageChosen(null));
    return () => {
      gone = true;
    };
  }, [chosenHandle, batch]);

  // The select tool's pick, for the properties palette; read again when the batch changes (an
  // undo, a redo, another sheet).
  const pickedTarget = tool === "select" ? measure.highlighted : null;
  const pickedKey = pickedTarget ? `${pickedTarget.handle}:${pickedTarget.chain.join(",")}` : null;
  useEffect(() => {
    if (!pickedTarget) {
      setPickedInfo(null);
      return;
    }
    let gone = false;
    entityInfo(pickedTarget.handle, pickedTarget.chain)
      .then((found) => !gone && setPickedInfo(found))
      .catch(() => !gone && setPickedInfo(null));
    return () => {
      gone = true;
    };
  }, [pickedKey, batch]);

  // The whole selection set when it holds two or more: every entity is read, so the palette's
  // sums add up everything picked (one that cannot be read leaves none, never a short sum).
  const picks = measure.picks;
  const picksKey = picks.length >= 2 ? picks.join(",") : null;
  useEffect(() => {
    if (picksKey === null) {
      setPickedMany(null);
      return;
    }
    let gone = false;
    const read = async () => {
      const found: EntityInfo[] = [];
      for (let i = 0; i < picks.length; i += 64) {
        found.push(...(await Promise.all(picks.slice(i, i + 64).map((h) => entityInfo(h, [])))));
      }
      return found;
    };
    read()
      .then((found) => !gone && setPickedMany(found))
      .catch(() => !gone && setPickedMany(null));
    return () => {
      gone = true;
    };
  }, [picksKey, batch]);

  return { imageChosen, setImageChosen, pickedInfo, pickedMany, setPickedProperty, setChosenDisplay };
}

export type Selection = ReturnType<typeof useSelection>;

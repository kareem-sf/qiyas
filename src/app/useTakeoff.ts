/** The side panel's state: the take-off rows and totals, the BOQ, the conflicts, the inquiry
 *  info card, and what each highlights on the drawing. (The panel's handlers join this file as
 *  `useTakeoff`.) */
import { useState, type RefObject } from "react";
import {
  boq as readBoq,
  conflicts as readConflicts,
  takeoff,
  totals,
  type Boq,
  type Conflict,
  type Measurement,
  type Total,
} from "../api";
import type { Info } from "../shell/InfoCard";
import { isStale } from "../shell/switcher";

export function useTakeoffState(switchGen: RefObject<number>) {
  const [items, setItems] = useState<Measurement[]>([]);
  const [itemTotals, setItemTotals] = useState<Total[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** The drawing's imported BOQ. */
  const [bill, setBill] = useState<Boq | null>(null);
  /** The BOQ item whose linked measurements are shown. */
  const [boqCode, setBoqCode] = useState<string | null>(null);
  /** The last comparison's conflicts. */
  const [conflicts, setConflicts] = useState<Conflict[]>([]);
  /** A comparison ran for this drawing, so no conflicts means none were found. */
  const [compared, setCompared] = useState(false);
  /** The measurements a conflict's Show pointed at. */
  const [conflictIds, setConflictIds] = useState<string[]>([]);
  /** Entities the assistant pointed at (Show on a tool card, or a zoom it asked for). */
  const [aiHighlight, setAiHighlight] = useState<number[]>([]);
  /** What the last inquiry (geometry, ID point) found, shown until closed. */
  const [infoCard, setInfoCard] = useState<Info | null>(null);

  /** Re-reads the take-off, the BOQ and the conflicts, after anything that may have changed them. */
  async function refreshTakeoffAndBoq() {
    const gen = switchGen.current;
    const [nextItems, nextTotals, nextBoq, nextConflicts] = await Promise.all([
      takeoff(),
      totals(),
      readBoq(),
      readConflicts(),
    ]);
    if (isStale(gen, switchGen.current)) return;
    setItems(nextItems);
    setItemTotals(nextTotals);
    setBill(nextBoq);
    setConflicts(nextConflicts);
    // Conflicts kept in the project file come from an earlier comparison.
    if (nextConflicts.length > 0) setCompared(true);
  }

  /** Forgets the selection and highlights of the drawing on screen (another one is coming). */
  function reset() {
    setSelectedId(null);
    setBoqCode(null);
    setAiHighlight([]);
    setConflictIds([]);
    setCompared(false);
    setInfoCard(null);
  }

  /** After the last tab closed: nothing to show. */
  function clear() {
    setItems([]);
    setItemTotals([]);
    setBill(null);
    setConflicts([]);
  }

  return {
    items,
    itemTotals,
    selectedId,
    setSelectedId,
    bill,
    boqCode,
    setBoqCode,
    conflicts,
    setConflicts,
    compared,
    setCompared,
    conflictIds,
    setConflictIds,
    aiHighlight,
    setAiHighlight,
    infoCard,
    setInfoCard,
    refreshTakeoffAndBoq,
    reset,
    clear,
  };
}

export type TakeoffState = ReturnType<typeof useTakeoffState>;

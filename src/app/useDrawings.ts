/** Drawings as files: the open drawings' tabs, and the project autosave switch. (The open, save,
 *  close, recovery and sheet set flows join this file as `useDrawings`.) */
import { useRef, useState } from "react";
import { saveProject, tabs as readTabs, type TabInfo } from "../api";
import type { T } from "./shared";

/** The open drawings' file tabs, and the one on screen. */
export function useFileTabs() {
  const [fileTabs, setFileTabs] = useState<TabInfo[]>([]);
  const [activeTab, setActiveTab] = useState<number | null>(null);

  /** Reads the file tabs again (titles, unsaved dots, the active one). */
  async function refreshTabs() {
    try {
      const next = await readTabs();
      setFileTabs(next.tabs);
      setActiveTab(next.active);
    } catch {
      // The tabs keep what they showed; the next refresh tries again.
    }
  }

  return { fileTabs, setFileTabs, activeTab, setActiveTab, refreshTabs };
}

export type FileTabsState = ReturnType<typeof useFileTabs>;

/** Saving the take-off next to the drawing after every change. */
export function useAutosave(t: T, setError: (error: string | null) => void) {
  /** Set when autosave failed for this drawing, so the error shows once, not after every click. A new
   *  open or a successful save turns autosave back on. */
  const autosaveOff = useRef(false);

  /** Saves the take-off next to the drawing. A failure is shown once and turns autosave off, but the
   *  work goes on. */
  async function autosave() {
    if (autosaveOff.current) return;
    try {
      await saveProject();
    } catch (e) {
      autosaveOff.current = true;
      setError(t("project.autosaveOff", { error: String(e) }));
    }
  }

  return { autosaveOff, autosave };
}

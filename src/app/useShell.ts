/** The window's own chrome: theme, the status bar's error and notice, the settings and About
 *  dialogs' switches, the side panel's tab, the command line aliases, and the viewport refs. */
import { useEffect, useMemo, useRef, useState } from "react";
import { aliases as readAliases } from "../api";
import { blurAfterClick } from "../shell/blurAfterClick";
import { useAutoClear } from "../shell/useAutoClear";
import { mergeAliases } from "../tools/aliases";
import type { Theme } from "../viewport/gl";
import type { ViewportHandle } from "../viewport/Viewport";
import { initialTheme, THEME_KEY } from "./shared";

export type SideTab = "takeoff" | "boq" | "conflicts" | "assistant";

export function useShell() {
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  /** The user's command line aliases from the settings. */
  const [userAliases, setUserAliases] = useState<Record<string, string>>({});
  const commandAliases = useMemo(() => mergeAliases(userAliases), [userAliases]);
  const [error, setError] = useState<string | null>(null);
  /** A one-off status bar message, e.g. after an export; the next action clears it. */
  const [notice, setNotice] = useState<string | null>(null);
  /** The right panel's tab. */
  const [sideTab, setSideTab] = useState<SideTab>("takeoff");
  const viewport = useRef<ViewportHandle>(null);
  const viewportBox = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  // Space pans the drawing; a button the mouse just clicked must not take it as a press.
  useEffect(blurAfterClick, []);
  // A notice goes with the next command (`run`), or on its own after a few seconds.
  useAutoClear(notice, () => setNotice(null));

  /** Reads the user's command line aliases (at start and after the settings close). */
  const loadAliases = () =>
    readAliases()
      .then(setUserAliases)
      .catch(() => {});
  useEffect(() => {
    loadAliases();
  }, []);

  function chooseTheme(next: Theme) {
    setTheme(next);
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch {
      // Storage blocked: the choice lasts for this session only.
    }
  }

  /** Runs an IPC step, showing its error in the status bar. */
  async function run(step: () => Promise<void>) {
    setNotice(null);
    try {
      await step();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  // Hand the keyboard back to the drawing, so Space pans instead of pressing a button.
  const focusDrawing = () => viewportBox.current?.focus();

  return {
    theme,
    chooseTheme,
    aboutOpen,
    setAboutOpen,
    settingsOpen,
    setSettingsOpen,
    commandAliases,
    loadAliases,
    error,
    setError,
    notice,
    setNotice,
    sideTab,
    setSideTab,
    viewport,
    viewportBox,
    run,
    focusDrawing,
  };
}

export type Shell = ReturnType<typeof useShell>;

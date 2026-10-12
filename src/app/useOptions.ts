/** The Manage tab's dialogs (Options, Customize, Aliases, Purge, Audit) and the window-wide
 *  options they keep in `settings.json`: decimals with no drawing open, the snap aperture and pick
 *  box, the keyboard shortcuts and the ribbon layout. Read at start and after a dialog saves. */
import { useEffect, useMemo, useState } from "react";
import { getSettings, type RibbonLayout, type Settings } from "../api";
import { SHORTCUT_COMMANDS } from "../shell/OptionsDialog";

const KNOWN = new Set(SHORTCUT_COMMANDS.map(([c]) => c));

export function useOptions() {
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [customizeOpen, setCustomizeOpen] = useState(false);
  const [aliasesOpen, setAliasesOpen] = useState(false);
  const [purgeOpen, setPurgeOpen] = useState(false);
  const [auditOpen, setAuditOpen] = useState(false);
  /** Decimals where no drawing is open; a drawing's drafting precision wins while it is. */
  const [displayPrecision, setDisplayPrecision] = useState(3);
  /** CSS pixels around the cursor snaps and picks reach. */
  const [px, setPx] = useState({ snap: 10, pick: 6 });
  const [userShortcuts, setUserShortcuts] = useState<Record<string, string>>({});
  /** The user's ribbon; null draws the default. */
  const [ribbon, setRibbon] = useState<RibbonLayout | null>(null);
  /** The shortcuts the keyboard handler can run (a hand-edited file may name others). */
  const shortcuts = useMemo(
    () => Object.fromEntries(Object.entries(userShortcuts).filter(([c]) => KNOWN.has(c))),
    [userShortcuts],
  );

  /** Takes the options from the settings (as read, or as a dialog saved them). */
  function takeSettings(s: Settings) {
    setDisplayPrecision(s.display_precision ?? 3);
    setPx({ snap: s.snap_aperture_px ?? 10, pick: s.pick_box_px ?? 6 });
    setUserShortcuts(s.shortcuts ?? {});
    setRibbon(s.ribbon ?? null);
  }

  useEffect(() => {
    getSettings()
      .then(takeSettings)
      // No settings to read: the defaults stay.
      .catch(() => {});
  }, []);

  return {
    optionsOpen,
    setOptionsOpen,
    customizeOpen,
    setCustomizeOpen,
    aliasesOpen,
    setAliasesOpen,
    purgeOpen,
    setPurgeOpen,
    auditOpen,
    setAuditOpen,
    displayPrecision,
    px,
    shortcuts,
    ribbon,
    setRibbon,
    takeSettings,
  };
}

export type Options = ReturnType<typeof useOptions>;

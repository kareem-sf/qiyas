# Fix round 2: N1 and two minors

Base fcc2619. Commits: the ribbon fix, the 4xx log fix, and a docs commit (CLAUDE.md + this report); see `git log`.

## 1. N1 (tabs row vs prefs overlay)
- `theme.css`: `:root { --prefs-width: 340px }` (210px under 1420px). `.prefs` has `min-width: var(--prefs-width)`;
  `.ribbon-tabs` has `overflow-x: auto`, thin scrollbar, `padding-inline-end: calc(var(--prefs-width) + 10px)`,
  children `flex-shrink: 0`; quick-access buttons `white-space: nowrap`. Breakpoints 1100 to 1420 and 700 to 1020
  (prefs on their own row, row padding back to 10px). Logical sides only, so Arabic mirrors.
- Side effect fixed: an `overflow` row would clip the Open recents menu, so `RecentsMenu` is placed `fixed` under
  its button like `ViewsMenu` (RTL aware).
- Tests (`Ribbon.test.tsx`, `RecentsMenu.test.tsx`): nowrap, overflow, padding variable, min-width, both new
  breakpoints and the old ones gone, no physical left/right; menu is `position: fixed`.
- Not seen by eye (no dev server). The 340/210 widths are estimates; adjust `--prefs-width` if the overlay is wider.

## 2. 4xx request log
- `request_for_log(body, secret)` scrubs first (pictures, tools, system prompt, then the key), then
  `log::clip_tail`: last 4,000 characters after a leading `…` when longer.
- Tests: tail kept and starts with `…`, last message present, head gone; key straddling the cut is gone
  (fails with cut-then-scrub); `clip_tail` unit test.

## 3. Quick-access roving
- `useToolbarRoving` in `useRoving.ts` (shares `nextIndex`, RTL mirrored): enabled buttons outside menus are
  stops, one tab stop (last focused, else first), arrows/Home/End, disabled skipped. Used on the toolbar in `Ribbon.tsx`.
- Tests: one tab stop and movement incl. Open's recents arrow, End/Home/wrap, stop stays on last focused;
  disabled skipped and arrows mirror in RTL. Both fail without the change.

## Check
`scripts/check.sh` exit 0 (first run stopped at rustfmt on my new test, fixed and amended; the run counted is the
second, whole): Rust 827 passed, 0 failed (was 824); vitest 80 files, 567 tests passed.

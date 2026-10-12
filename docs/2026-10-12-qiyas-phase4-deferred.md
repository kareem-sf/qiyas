# Qiyas Phase 4: rulings and items carried forward

Date: 2026-10-12. Source: the build ledger of the Phase 4 plan (`docs/superpowers/plans/2026-10-08-qiyas-phase4-output.md`). Check results: `docs/2026-10-12-qiyas-phase4-check.md`. Carried in: `docs/2026-10-09-qiyas-phase3-deferred.md`.

The founder approved the plan with "Proceed" on 2026-10-08. Every decision below was taken on his behalf while building; each line says what it costs if wrong, so he can undo any of them.

## Rulings made before the build (preflight)

- Task 0 was added: `src/App.tsx` (3,400 lines) was split into `src/app/*` hooks and `Header`, `Dialogs`, `Panels` with no behaviour change (a 28-step IPC and DOM comparison against the old window was identical), and every Phase 4 window change waited for it. Cost if wrong: none seen; the hooks' order is pinned in CLAUDE.md and `App.test.tsx`.
- The plot reads the renderer's batch (lines, fills, glyphs, pictures) rather than drawing the document again: text is real PDF text with embedded Noto subsets when every glyph maps back to a character, else outlines; fills are paths; pictures are embedded. Cost: a second drawing path would have drifted from the screen.
- Plot styles (`ctb.rs`) live with the plot crate; the editor is a window dialog.
- Scripts have their own grammar in Rust (`script.rs`), the headless truth: a documented subset of AutoCAD words with typed points, plus any command in its wire form as a JSON line. The window's TypeScript command parser is unchanged. Cost: two grammars; the shared words are listed in CLAUDE.md.
- PURGE reuses the save-time purge; AUDIT is new and fixes only the safe items.
- Parametric constraints are `QIYAS-PARAM` xdata on the entities, with an own 150-line expression parser (no new licence). nalgebra (Apache-2.0) is the only new library; its licence text is in `LICENSES/`.
- The updater's feed address and public key are placeholders that make every check refuse until the founder sets them; signing keys are never committed.

## Rulings made during the build

Plot and print
- The plot snapshot is gathered under the session lock and the PDF written outside it, on a blocking thread; curves are flattened again for the paper at 0.01 mm. Cost if wrong: a slow plot on a huge drawing holds the snapshot only, never the window.
- CTB and STB fixtures are files Qiyas's own writer makes after parsing real ones, never vendor files. Windows printing passes the file and printer through environment variables into a fixed PowerShell script: no name ever inside a command string. Defpoints never plots. Arabic stored visually that Qiyas did not write is plotted as outlines, never as text.
- A plot by command stays inside the drawing's folder; the Save dialog's path goes through one small `plot_pdf_to` IPC and nowhere else. Page setups are kept per drawing path and visible layout name in the window's local storage (the project file has no place yet). Quick print and Print all stop when another drawing comes on screen (generation guard, one at a time). Cost: same-name layouts share a setup.

Scripts and the command line tool
- The headless runner never touches the app's `.sv$` autosaves or `.qlock` files; `convert` writes no project file and never rewrites the source's; output collisions are refused before anything runs (case-insensitive on every system); the default export name is `{name}.{ext}.xlsx`. Exit codes 0 done, 1 error, 2 warnings only. SAVE on a drawing the failsafe reader repaired is refused unless `--allow-repaired-save`. An unreadable project file is a warning (exit 2) and the drawing opens with an empty take-off.
- In the window, a script's OPEN runs the recovery check and refuses a drawing with an autosave to decide about, a crash or a lock elsewhere; DELAY is capped at 10 s; a Stop button; the ribbon and command line wait; QUIT ends the script only; refused during a block edit. Cost: a script cannot decide about autosaves (by design).

Audit, purge, options
- Quoted hex strings in the codec's debug text count as handles, and a drawing holding data the codec does not read purges layers only, with a reason in the report. Cost: a few unused entries survive a purge (safe side).
- The ribbon is data pinned to one JSON on both sides (`src/shell/ribbon.default.json`, compared by a Rust and a TypeScript test); a saved layout may only use the default's buttons. Drafting precision wins while a drawing is open; `display_precision` applies with none open. Background and highlight colours are drawn; the crosshair colour is kept but not drawn yet (no crosshair). User shortcuts call `preventDefault` so F5 never reloads the window.

Parametric
- Constraints as xdata on entities; deletes remove them through the follow hooks; placements (move, rotate, scale, stretch, align, a mirrored line) re-solve in the same undo step; vertex-changing edits (trim, break, reverse, extend, join, a mirrored arc) drop the entity's constraints with a count in the outcome rather than re-solving against wrong points. Cost if wrong: a trimmed constrained line loses its constraints (it says so).
- A parameter whose expression cannot be worked out is marked with an error and skipped, never stopping every solve; Arabic digits are stored as ASCII; AutoConstrain is capped at 2,000 constraints and runs one staged kernel call (6 s became 0.06 s); the solver works about the sketch's middle at unit scale (site grids). The 50 ms end-to-end guard runs in release only.

Updates, launch, installers
- A deb or rpm install waits for the password while the window stays usable; a drawing changed meanwhile keeps Qiyas running ("Installed. Restart when ready"). The download stalls out after 30 s and stops on Close. `requireSignedVersion` stays off until the Tauri CLI writes the version into the signature.
- Qiyas never takes `.dwg`/`.dxf` away from AutoCAD: macOS ranks them Alternate, Linux adds "Open with", Windows keeps `.qiyas` only. The recovery queue asks about a drawing once (dedupes by path), so a double-clicked drawing opens after the launch question.

Recovery and locks
- A dead lock (pid gone, or a different start time) marks the drawing crashed and its autosave is offered whatever its age; a live lock elsewhere is reported, never enforced (no read-only open), and Discard is refused while it holds. Liveness is the pid plus its start time within 2 s; the watch checks it at most every 10 s and never under the app's locks.
- An autosave never writes over another Qiyas's: under a foreign lock it goes to `<file>.sv$.<pid>`. Crash copies (`<file>.sv$.crash-<time>`, 3 at most) are offered once as the crash's autosave, then listed under the drawing as older copies with their kind (crash or instance); ended instances' autosaves are listed too, a running one never. Clean drawings are not asked about at a crash.

## Rulings from the final review and its fix waves (2026-10-12)
The final review found no Critical, 5 Important and 8 Minor items; every Review Focus of the plan is proven by a real test (Focus 2 on CTB and STB files Qiyas's own writer made, Focus 4 timed in the solver alone, in release). The founder, testing the Linux AppImage meanwhile, raised two more: the assistant's 400 after a tool call and the ribbon's jumbled rows.
- Arabic copied or searched in a plotted PDF came back with the wrong letters (Noto Sans Arabic draws letters that differ only by their dots as one shape; the text map kept one letter per shape). Fix: an ActualText span per letter, only where the text map would be wrong (one span per text reversed the words in pdftotext). Cost if wrong: a viewer that ignores ActualText still copies wrong letters; slightly larger PDFs.
- A window script keeps to the tab it started on (OPEN moves it) and stops when another tab comes on screen between lines. Cost: a tab switch kills a run; the tab bar is not made inert while a script runs (left open).
- The command line tool refuses to write over a drawing whose lock belongs to a running Qiyas or another computer (SAVE, SAVEAS, convert's output; the lock is read, never written; a dead lock is fine). Cost: a stale lock from another computer blocks a batch until its `.qlock` is deleted by hand.
- The installers ship `LICENSES/` (with a README naming the MPL-2.0 sources and the font licences) and a generated `THIRD-PARTY.md` (names, versions, licences; `scripts/third-party.sh` regenerates it). The full copyright notices of every MIT and BSD library are not copied in yet: owed before the first installer leaves the company.
- Audit's fix waits for a block edit in place (the dry run still runs); a script may OPEN a drawing that only has older copies; the print spool is a private per-user folder and the PDF goes after `lp`/`lpr` (else after a day); Windows printing runs PowerShell hidden.
- The assistant sends an empty string, not null, as the content of a tool-calling turn, and echoes Gemini's thought signatures back only when the service sent them; a 4xx answer logs the request sent (pictures as `<image>`, tools as names, the system prompt as its length, the last 4,000 characters), never the key. Not yet proven against the live Gemini service: the founder's next try shows the refused request in the log if it still fails.
- The ribbon became AutoCAD-style: labelled groups in three-row grids, a panel that scrolls sideways instead of wrapping, and a quick-access bar (Open with recents, Save, Save as, Undo, Redo) after the app name, mirrored in Arabic. Take-off's buttons (Export take-off, Import BOQ, Compare, Export conflicts) moved to a Take-off group on the Measure tab; Recover sits under Manage, Drawing, beside Audit and Purge. A saved custom layout that still holds a moved button falls back to the default once. Cost if wrong: a button to move; one custom layout lost.
- The tabs row no longer hides under the preferences overlay at laptop widths (end padding, side scroll, breakpoints raised), and the quick-access bar takes arrow keys.
- The Thmanyah font files are built into the app by Vite and so into every installer; whether the licence allows that is the founder's decision before an installer leaves the company (code unchanged).

## Left open (minor)
- Task 0: the comparison script's gaps were covered by reading; `useDrawings` is 602 lines; some refs are written during render.
- Plot: pictures with missing definitions are dropped silently; picture memory is not bounded beyond 8192 px a side; `plot_rotation` margins; `plotToPdf` has no generation guard; a narrow switch window after the last check.
- Scripts and CLI: a DWG and a DXF twin share one `.qiyas`; a JSON `write_block` path resolves from the script's folder; `convert` does not rewrite picture links to the new folder; opening a broken project writes `.qiyas.bak` (over a good one); a Stop clicked before the flag clears is lost; shortcuts and the tab bar are not inert while a script runs.
- Purge and audit: hex-looking words keep extra entries (safe side); `*U0` dimensions keep all `*D` blocks on save; the core still accepts Audit fix during a block edit from MCP or a script (the window refuses it).
- Options: a bound F5 during a drawing switch can still reload.
- Parametric: staged inference can miss a parallel or perpendicular pair near the axis tolerance; a mirrored arc drops constraints silently; stretching part of a constrained entity is refused as a conflict.
- Updates and launch: two drawings double-clicked in one launch question (the second never opens); Close cannot cancel in the millisecond between download end and install; Recover then Later opens the original too.
- Recovery: the crash-copy rename races a running autosave; a wall-clock step over 2 s misjudges liveness; a drawing not on disk waits 10 s for its lock; two crash copies in one second collide; instance liveness is judged against this computer's pids (a shared folder); a stderr line in the watch thread; per-second disk I/O.

- From the final review and its re-review: `convert` writes without flushing and duplicates a core helper; script logs show Purge, Audit and constraint results as one raw debug line; the plot-style fixtures are Qiyas-written, not vendor files; a headless script's JSON `write_block` does not check the lock; the ActualText test checks one span when pdftotext is missing; the script-tab test sleeps 200 ms; synthesised `call_N` ids could collide across turns; Home may scroll at 1440 px and the panel height is tight; the Views menu is placed once at open; unused i18n keys (`ribbon.file`, `ribbon.history`, `text.group`); old saved layouts are dropped whole rather than repaired.

## Carried to the next phase
- Full copyright notices of every library in `THIRD-PARTY.md`; the assistant against Gemini through Runware, proven live.
- The Thmanyah font files are built into the app by Vite and so into every installer: whether the licence allows that is the founder's call before an installer leaves the company (Noto Sans Arabic is already there for drawings).
- Phase 3 carries that still stand: whole-sheet redraw per change and the click index on every layer toggle; no draw order (wipeouts hide later lines; a click inside a solid fill picks nothing); grips; the split pane is view only; dimensions not re-measured after a trim or stretch; fields not evaluated on open; drag-to-insert from the block palette; `earcutr` for the triangulator.
- Page setups in the project file (they are in local storage now); viewport pictures and twisted views on screen; the crosshair.
- The assistant and MCP still get no edit, draw, dimension, plot or parametric tools.

## Founder checks (need a real window, a printer or AutoCAD)
- Install the deb (`sudo apt install ./Qiyas_0.1.0_amd64.deb`) or run the AppImage: does Qiyas start with its icon and Arabic name, and does a `.dwg` offer Qiyas under "Open with"?
- Open the two office drawings, plot one layout to PDF at 1:100 on A1 and measure a known line on the page against AutoCAD's plot; print it; copy Arabic from the PDF.
- Run a script from the Manage tab; run `qiyas-cli batch` on a folder.
- Kill Qiyas mid-edit and relaunch: the recovery dialog, Recover, Save As.
- About box: "Updates are not set up in this build yet" until the feed and key are set.
- Clipboard on Wayland; Arabic text and dimensions; the Parametric tab on a real plan (auto constrain, change a width); Options colours and a custom shortcut.
- The Thmanyah licence question above.

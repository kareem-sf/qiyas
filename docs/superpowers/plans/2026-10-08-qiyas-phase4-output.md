# Qiyas Phase 4: Output, Automation, Parametric and Shipping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qiyas prints and plots, runs scripts and headless jobs, has parametric constraints, and ships as an installable app with updates and crash recovery: the spec's Phase 4 (features 13, 69 to 71, 79, 80, 83, 84, 88).

**Architecture:** Plotting renders the same batch (lines, fills, glyph quads, images) to PDF through `printpdf` with plot styles applied as a colour and weight map; printing hands that PDF to the OS. Scripts are the command line's grammar read from a file; the headless CLI is a second binary over `qiyas-core`. Parametric constraints are a point-based solver over entity vertices (Gauss-Newton on constraint residuals) with inference from `opencadkernel::constraint_inference`. Plugins are MCP servers or command scripts; no native plugin ABI in this phase (decision recorded below).

**Tech Stack:** printpdf 0.12, the OS print path (Linux `lp`/`xdg-open`, Windows `ShellExecute print`, macOS `lpr`), tauri-plugin-updater and tauri-plugin-single-instance, nalgebra (BSD) for the solver, clap for the CLI.

**Spec:** `docs/superpowers/specs/2026-10-08-qiyas-design.md` ("Phase 4").

## Global Constraints

- Everything from the earlier plans.
- Plot output must match the screen: same colours (plot style applied), same line weights at the plot scale, text as real PDF text where the font allows (searchable), fills as vector fills.
- Headless CLI never opens a window and exits non-zero on any error with a one-line message.
- Updates: manual "Check for updates" plus a check at launch at most once a day; the feed URL and public key live in `tauri.conf.json`; nothing is auto-installed without a click.
- Decision: native plugins (feature 83) are replaced by MCP servers and command scripts. A plugin is "anything that can call the command bus"; this keeps the app small and safe. Recorded for the founder.

## Review Focus

1. Plot a layout with a paper-space viewport at 1:100 on A1: a 10 m line must measure 100 mm on the printed page (checked by reading the PDF's path coordinates in the test). Task 1.
2. A CTB where colour 1 plots black at 0.5 mm and colour 3 is "no plot": the PDF must contain the red entities in black at 0.5 mm and nothing from the green ones. Task 2.
3. A script that opens a drawing, counts three blocks, exports the take-off and quits, run headless on a folder of 50 drawings: every file produces its Excel, one corrupt file is reported and skipped. Task 4.
4. A rectangle with four coincident constraints, two horizontal, two vertical and one dimensional width constraint: changing the width keeps it a rectangle and the solver converges in under 50 ms. Task 6.
5. Crash recovery: kill the process mid-edit; on relaunch the recovery dialog offers the autosave and opening it restores the last autosaved state with its take-off. Task 8.

---

### Task 1: Plot to PDF

**Files:** `crates/qiyas-plot/{Cargo.toml, src/lib.rs, src/pdf.rs, src/page.rs}` (`PlotJob { sheet, area: Extents | Window(bounds) | Layout, paper: Paper { name, w_mm, h_mm }, orientation, scale: Fit | Ratio(1, 100), centre: bool, style: Option<PlotStyle>, lineweights: bool, output: PathBuf }`; `plot_pdf(session, &job) -> Result<()>`: builds the batch for the sheet, maps world → page, writes lines with weights, fills, glyphs as text runs (embedding Noto subsets) or outlines when a glyph has no text mapping, images; paper-space layouts render each viewport's model view clipped), `src/shell/PlotDialog.tsx` (paper list from `paper_catalog` built in, preview thumbnail via `render_png`, page setup saved per layout in the project file), ribbon Output tab.
- Tests: Review Focus 1 (parse the written PDF with `lopdf` and find the path); fit-to-paper centres extents; fills present.

---

### Task 2: Plot styles (CTB/STB), print to printer, quick print, print all

**Files:** `crates/qiyas-plot/src/ctb.rs` (parse CTB and STB: they are zlib-compressed text; map colour → pen colour, screening, lineweight, "no plot"), `PlotStyleEditor.tsx` (edit and save CTB), print: write the PDF to a temp file and hand it to the OS print dialog (Linux: `xdg-open` the PDF with a note, or `lp -d <printer>` when a printer is chosen from `lpstat -a`; Windows: `rundll32`-free approach via the default PDF handler's print verb; macOS: `lpr`), Quick print (current view, default printer), Print all layouts.
- Tests: Review Focus 2; CTB round trip after an edit.

---

### Task 3: Command scripts (79) and headless conversion (80)

**Files:** `crates/qiyas-core/src/script.rs` (runs the command-line grammar from a `.scr` file: one command per line, point and option answers on the following lines or the same line, `;` comments, `DELAY`, `SAVEAS`, `QUIT`), `crates/qiyas-cli/{Cargo.toml, src/main.rs}` (`qiyas-cli convert <in> <out>` (dwg↔dxf, version flag), `qiyas-cli run <script.scr> <drawing>`, `qiyas-cli takeoff <drawing> --count DOOR --export out.xlsx`, `qiyas-cli batch <folder> --script x.scr` with a per-file report), ribbon Manage → Run script.
- Tests: Review Focus 3 with a temp folder of generated drawings and one junk file.

---

### Task 4: Audit, purge, options, customise the interface (84), aliases editor

**Files:** `Audit` (reports dangling handles, empty blocks, unreferenced layers; fixes with confirmation), `Purge` (unused layers, blocks, styles, linetypes; dry run first), `OptionsDialog.tsx` (units display, autosave interval, snap aperture, pick box, colours of background/crosshair/highlight, keyboard shortcuts), `CustomizeUi.tsx` (ribbon tab/group/button layout stored as JSON in settings, reset to default), `AliasEditor.tsx` (command aliases as a table in settings, used by the command line).
- Tests: purge dry run lists exactly the unused layer; audit finds an entity on a missing layer; options round trip.

---

### Task 5: Parametric constraints, part 1: geometric (69)

**Files:** `crates/qiyas-param/{Cargo.toml, src/lib.rs, src/model.rs, src/solver.rs}` (a constraint graph over "handles" of entity vertices and curves: `Coincident`, `Collinear`, `Concentric`, `Fixed`, `Parallel`, `Perpendicular`, `Horizontal`, `Vertical`, `Tangent`, `Smooth`, `Symmetric`, `Equal`; the solver treats every constrained vertex as variables and minimises residuals with Gauss-Newton + damping (nalgebra), under 50 ms for 200 constraints; constraints are stored in the drawing as xdata on the entities (`QIYAS-PARAM`) and shown as glyphs in a constraint bar), `AutoConstrain{handles, settings}` using `constraint_inference::infer_constraints`, `src/tools/constraintTools.ts`, `src/shell/ConstraintBar.tsx`.
- Tests: Review Focus 4 (geometric part); auto-constrain on the sample rectangle infers 4 coincident + 2 horizontal + 2 vertical; deleting a constrained entity removes its constraints.

---

### Task 6: Parametric constraints, part 2: dimensional (70) and the parameters manager (71)

**Files:** `DimConstraint::{Linear, Aligned, Angular, Radius, Diameter}` with an expression (`d1 = 2*d2 + 1.5`) evaluated by a small expression parser (`evalexpr`-style, own 150-line parser to keep licences simple), `ParametersManager.tsx` (name, expression, value; editing re-solves and updates the geometry), dimensional constraints render like dimensions with a lock glyph.
- Tests: Review Focus 4 (width change), expression dependency (change d2 → d1 updates), circular expressions rejected.

---

### Task 7: Updates, single instance, file associations, installers (88 and shipping)

**Files:** `tauri-plugin-updater` wiring with a static feed JSON on the company site (the founder chooses the URL; placeholder until then), signing keys generated by the founder (`tauri signer generate`, never committed), "Check for updates" in the About box and a once-a-day check; `tauri-plugin-single-instance` (a second launch with a file path opens it in the running app); file associations `.dwg`/`.dxf`/`.qiyas` in `tauri.conf.json` bundle config (Windows MSI and NSIS, Linux AppImage and deb, macOS dmg); DWG thumbnails are out of scope (OS-specific shells).
- Tests: the updater feed parser against a sample JSON; single-instance argument forwarding tested manually; bundles built with `pnpm tauri build` on this machine (Linux) and documented for the other two.

---

### Task 8: Crash recovery and autosave hardening

**Files:** `crates/qiyas-core/src/autosave.rs` (every N minutes and after every 20 document commands, write `<name>.sv$` (DWG) and `<name>.sv$.qiyas` atomically; a lock file `<name>.qlock` with the PID while open; on launch, lock files whose PID is dead mean a crash → recovery dialog listing `.sv$` files newer than the drawing; "Recover" opens the autosave and offers Save As), panic hook that flushes an autosave before exit (best effort, with a timeout), `RecoveryDialog.tsx`.
- Tests: Review Focus 5 (simulate by writing the autosave then killing a child process in the test harness); stale lock cleanup.

---

### Task 9: Check and release candidate

- [ ] `scripts/check.sh`; build the Linux bundle; plot the founder's drawing to PDF and compare with a print from another CAD; run the headless batch on his folder (copies); write `docs/…-phase4-check.md`; no push, no release, until the founder says yes.

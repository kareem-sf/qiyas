# Qiyas Phase 1b (drawing I/O): final check

Date: 2026-10-09. Qiyas commit `4c6651f` (branch main, not pushed). Same laptop as the Phase 1 check.

## In short

- All local checks pass: 352 Rust tests and 204 front-end tests, no warnings.
- **Phase 1's known gap is closed.** Clicking a wall inside a block now gives its length, not a count of the block. Checked on both real drawings.
- **Arabic text in your drawings shows joined and right to left.** The big drawing has 2,442 Arabic texts. All were stored the normal way, so none needed the "reversed" switch.
- **PDF underlays and calibration work on a real AutoCAD plot.** On the Fire Station ground floor plan, calibrating on the 44650 overall dimension made grid A to K measure 44.450 m, which is right.
- **One real problem, now fixed: the big drawing had got slower.** During the check every layer click and first draw took about 1.6 s, up from about 0.45 s in Phase 1. The final review found the cause: the render crate grew, and the compiler split the click-index code across many units. Building that one crate as a single unit brought the first draw back to about 0.6 to 0.7 s (see "What broke or looked wrong", 1, and the fix wave below).
- Neither real drawing has images, PDF underlays or xrefs, so those were tried on files added for the test, not on your DWGs.

## What Phase 1b added

- Picking inside blocks: the first click picks the line inside the block, a slow second click picks the whole block, and a double click (or Enter) measures it.
- Drawing text as real letters, Latin and Arabic. Before, text showed as boxes.
- Snaps, ortho, polar and a dot grid.
- Xrefs (read only), with a dialog to reload them or find a missing file.
- PDF pages brought in as lines and text, or attached as underlays. Raster images too. Both can be calibrated.
- Inquiry tools: Geometry, Divide, Measure along, ID point and Extract data.
- A command line with AutoCAD-style aliases, and a properties palette.
- Several drawings open at once as tabs, named views, and recent files.
- Smaller polish: smooth wheel zoom, keyboard tabs, and a problems list.

## Check script

`scripts/check.sh` on a clean tree: exit 0. Rust: 352 passed, 0 failed, 0 ignored (pdfium is installed on this machine, so the PDF tests ran). Front end: 204 tests in 43 files. fmt, clippy and tsc were clean.

## Numbers per drawing

From `bench_open` (release build), three runs each, plus a temporary test example (now deleted) that timed the parts separately. "Phase 1" is from the foundation check. "Phase 1 code today" is the Phase 1 commit rebuilt and run on the same laptop today, so you can compare like with like. "After the fix" is Qiyas commit `f65c79d` and later (the click index fix, see below), three to five runs on copies of the drawings; `bench_open` now times the click index on its own.

**office drawing A 17.09.2026.dwg** (1.61 MB, office floor plans)

| What | Phase 1 | Before the fix | After the fix |
|---|---|---|---|
| Open | 27–31 ms | 27–32 ms | 25–29 ms |
| First draw (batch) | 75–79 ms | 302–312 ms | **101–125 ms** |
| of which: lines and text / click index | – | 45 ms / 250 ms | about 45 ms / **60–79 ms** |
| Layer click (rebuild) | – | about 0.29 s | not timed (the same rebuild as the first draw) |
| Batch sent to the screen | 9.1 MB | 9.1 MB, plus one 4 MB letter page sent once | same |
| Line segments | 322,697 | 321,239 | 321,239 |
| Texts drawn | boxes | 98 texts, 1,248 letters | same |
| Problems | 0 | 0 | 0 |

**drawing B HO - Option B Design Package.dwg** (2.65 MB, about 40 sheets in model space)

| What | Phase 1 | Phase 1 code today | Before the fix | After the fix |
|---|---|---|---|---|
| Open | 72–80 ms | 76–78 ms | 73–76 ms | 73–80 ms |
| First draw (batch) | 418–438 ms | 461–463 ms | **1,460–1,760 ms** | **600–720 ms** |
| of which: lines / text / click index | – | 101 ms / – / 327 ms | 340 ms / about 60 ms / **1,240 ms** | about 290 ms (lines and text) / **335–400 ms** |
| Layer click (rebuild) | about 0.45 s | – | **1.65–1.72 s** | not timed (the same rebuild as the first draw) |
| Zoom change (letters redrawn) | – | – | 48 ms | not timed |
| Batch sent to the screen | 39.7 MB | – | **49.7 MB** (lines 40.3 MB plus 9.4 MB of letters) | same |
| Line segments | 1,389,985 | – | 1,388,323 | 1,388,323 |
| Texts drawn | boxes | – | 13,462 texts, 187,724 letters, 1 letter page | same |
| Problems | 0 | – | 0 | 0 |

How the time splits:

- **Text** costs about 60 ms on the big drawing. All 13,462 texts fit on one 2048 × 2048 letter page.
- **Lines:** the extra cost (101 to 340 ms) comes from picking inside blocks. The drawing now keeps 390,000 small pieces, one per line inside a block, against 47,000 before.
- **The click index** went from 330 ms to 1,240 ms, with the same index code as in Phase 1. To check the drawing data was not the cause, the Phase 1 build and today's build were each given the other's line data. Each kept its own speed, whichever data it got. So the slowdown came from the build itself, not the drawing.
- **Cause, found after the check:** as `qiyas-render` grew, the compiler split it into more pieces ("codegen units", 16 in release, 256 in dev builds). The click index's bulk load (from the `rstar` library) was split with it and ran about 4 times slower. The fix builds `qiyas-render` as one piece (`codegen-units = 1` in `Cargo.toml`, for dev and release builds). The click index is back to 335–400 ms on the big drawing (60–79 ms on drawing A) and the first draw to 0.6–0.7 s. In a dev build (`pnpm tauri dev`) the big drawing now draws in about 0.8 s, of which the index is about 0.47 s. A release test in `scripts/check.sh` (1 million segments must index in under 0.6 s) keeps it from coming back: without the setting it takes 1.1 s and fails.

## Real-file checks (through the core, no window)

**Wall inside a block (the Phase 1 gap).** In the big drawing, the whole plan sits inside bound-xref blocks such as `BASE-B`. Clicking a wall line there now gives:

| Drawing | What was clicked | Phase 1 answer | Now |
|---|---|---|---|
| drawing B | wall polyline inside `BASE-B` | "Insert on A-WALL-EXST", 1 pcs | **5.632 m**, length |
| drawing B | wall line inside `BASE-B` | "Insert on A-WALL-EXST", 1 pcs | **0.622 m**, length |
| drawing A | wall line inside an unnamed block | "Insert on WALL", 1 pcs | **0.393 m**, length |

The longest wall polyline inside `BASE-B` reads 95.484 m. The description still carries the raw bound-xref layer name, for example "LwPolyline on 02-First floor (arch&furn)$0$02-FIRST FLOOR (ARCH&FURN)$0$A-WALL". That is too long for a QS (see below).

**Arabic text (big drawing).** 14,092 texts, of which 2,442 are Arabic. None use the old "presentation form" letters, and none were detected as stored backwards. Examples: "المصمم / المقاول", "عنوان اللوحة", "رقم اللوحة". On screen they are joined and right to left, see `p1b-11` and `p1b-12`. The small drawing has no Arabic.

**Images, underlays and xrefs in your DWGs.** Neither drawing has a raster image, a PDF underlay or an xref; the "xrefs" in the big one are already bound, as `$0$` names. So xref loading and stored images could not be tried on your files.

## PDF and image (copies, headless)

**PDF:** `9-Fire Station ARCH DWG 1.pdf` from your Zamanan tender folder (copied). It is an AutoCAD 2022 plot, A3, 11 pages, vector. Page 1 is the ground floor plan, "SCALE 1:150 @ A3". It was attached to a copy of the drawing A drawing.

| Step | Result |
|---|---|
| Attach page 1 as underlay at 1:100 | 16 ms. It lands 42.02 × 29.70 m, which is A3 at 1:100. |
| Picture of the page | 0.5 s per zoom step, up to 2048 × 1448 px |
| Import page 1 at 1:100 | 275 ms: 61,776 entities (49,615 lines, 11,860 polylines, 301 texts) on 25 colour layers |
| Overall dimension "44650" at 1:100 | 30.163 m between its extension lines |
| Calibrate those two points to 44.65 m | factor 1.4803. The sheet is really about 1:148, not 1:150. |
| Grid A to K after calibration | **44.450 m** (the drawing gives 44650 − 100 − 100 = 44450) |
| Import straight at 1:150, as the title says | the 44650 dimension reads 45.244 m (1.3 % too long) |
| Save as DWG and reopen | underlay still there, 0 problems |

Two things this real sheet shows:

- **Calibrate before you measure, even when the sheet states a scale.** The title says 1:150, but the plot is about 1:148. The graphic scale bar on the same sheet says 1:100, and it is wrong for this plot.
- **Most of the sheet's text came in as lines, not text.** It was plotted from SHX fonts. Only 301 real texts came in, and the dimension numbers are lines.

**Image:** the WhatsApp photo in your drawing B folder (899 × 1599 px, a hand sketch). It is under the 200 KB the brief asked for, but it is the only real image on this machine. The other large images are screenshots.

- Attach: 4 ms. Picture drawn in 30 ms.
- Calibrated the bottom edge from 10 m to 5 m. It measured 5.000 m, and the height followed (8.893 m, shape kept).
- Brightness, contrast and fade set and read back.
- The photo has no EXIF turn flag and no real dimension, so the calibration was checked only as maths, not against a real size.

## UI run (Vite page, real core)

The app page ran in the built-in browser (1400 × 900). A temporary test served all the real app commands over local HTTP, with pdfium loaded. It is deleted, and nothing of it is committed. Screenshots are in the session scratchpad, `t12/screens/p1b-*.jpg`.

1. **Command line:** typed `DI`, then `0,0`, `12,0` and Enter. The take-off got "Distance 12.000 m" (`p1b-1`).
2. **Snap:** with the Distance tool, hovering near a wall corner showed the End marker. The status bar said "End" (`p1b-2`).
3. **Properties palette on a door:**
   - First click: "Arc, A-DOOR, length 1.414 m, radius 0.900 m, inside DOOR".
   - Second click: the whole DOOR reference (`p1b-3`, `p1b-4`).
4. **Tabs:**
   - Opened drawing A, then the big drawing, as second and third tabs.
   - Switching tabs took about 60 ms in the core during the check. After the fix wave a tab in the back drops its drawn lines and click index to save memory, so switching back to the big drawing now takes about 0.6 s.
   - Each tab keeps its own take-off (`p1b-5`, `p1b-6`).
5. **Attach PDF:**
   - The dialog showed all 11 page thumbnails in 2.9 s and detected A3.
   - Attached at 1:100. The underlay drew, a PDF-UNDERLAY layer appeared, the tab got its "unsaved" dot, and Calibrate turned on (`p1b-7`, `p1b-8`).
6. **Arabic:**
   - The interface mirrors right to left (`p1b-9`, `p1b-10`).
   - The big drawing's Arabic title-block text shows joined (`p1b-11`, and English dark `p1b-12`).

## Real app window

`pnpm tauri dev --config crates/qiyas-desktop/tauri.conf.json`: the build took 45 s, and a window titled "Qiyas" appeared after 51 s. After the app started, its log had nothing in it.

Earlier lines in the log are errors from browser tabs I had left on the dev page with the test bridge removed, not from the app window. Then only that run's own processes were stopped. No agent could see inside the window, so nothing was clicked in it.

## What broke or looked wrong

Nothing crashed. No code was changed in this check.

1. **The big drawing was about 3.5 times slower to redraw** (first draw and every layer click: about 1.6 s, was about 0.45 s). Fixed in the fix wave.
   - Most of it was the click index (1.24 s), 3.5 times more than the same code at the Phase 1 commit, on the same data.
   - Cause: the render crate grew to eight modules, so the compiler split the index code across many codegen units and lost the inlining its hot loop needs. The crate is now built as one unit in both profiles; the index takes 0.34 s again and a release test guards it.
   - Picking inside blocks still adds about 240 ms to building the lines.
   - Building the index in the background after the first draw is left for Phase 3.
   - The batch also grew from 39.7 to 49.7 MB, because of the letters.
2. **Measurement names carry raw bound-xref layer names**, for example "…$0$02-FIRST FLOOR (ARCH&FURN)$0$A-WALL". The same problem as the count names in Phase 1: it should show "A-WALL".
3. **Layer names are jumbled in Arabic mode.** Latin names are drawn right to left: "01-IVORY…$0$A-FURN" shows as "…)$0$A-FURN-01", with the start cut off.
4. **A new underlay can land off screen.** After Attach PDF the view did not move to show it, so you have to press Zoom extents to find it.
5. **"Z W" in the command line wants a mouse drag.** Typed corners are refused with "Unknown command".
6. **A door reference shows "Length 2.313 m" in the palette.** That is the sum of the lines inside the block. For a block, a count is what matters, so the length is confusing.
7. **Small things:**
   - In Arabic, the Measure tab is "قياس", the same word as the app's name.
   - Command line history stays in the language it was typed in.
   - The test plan's circle looks slightly faceted when the whole drawing is in view.

## Fix wave after the final review

The final whole-branch review (read-only, with its own timings) rated the phase "ready with fixes". One fix wave followed, all on main, check script green (358 Rust tests, 209 frontend tests):

- Click index 4 times faster: the render crate builds as one codegen unit; a release test fails if 1 M segments take over 1 s.
- Measurement names, info cards, inquiry and the palette show a bound-xref layer by its own name ("A-WALL"); the full name stays in the layer field.
- Latin names read in their own direction in Arabic mode (layers, take-off, tabs, recents, views, sheets, info card).
- The view fits a newly attached PDF underlay or image.
- Status bar cursor in metres with an "m" suffix (one unit everywhere).
- A block reference shows count and name, no length or area.
- Extract data writes numbers as numeric Excel cells.
- Images over 20,000 px a side or 512 MB decoded are refused before decoding.
- One work budget per sheet (5 M paths), so a big block inserted many times stops with "Drawing too complex".
- Tabs in the back drop their drawn lines and click index (memory), rebuilt on switch.
- `Z W` takes typed corners; "Yes" at the unsaved prompt still warns about a recovered drawing; a fast tab switch drops stale zoom work; the Arabic Measure tab says "القياسات".

Rulings and items carried forward: `docs/2026-10-09-qiyas-phase1b-deferred.md`.

## Phase 1b limits (left for later)

From the build log's "minor (deferred)" lines, in plain words. None blocks Phase 1b. Three of them were meant for a final clean-up round that has not happened yet: the status bar showing metres with an "m", Extract data writing numbers as numbers, and the unsaved-drawing prompt items.

- **Text:**
  - Zooming far in or out sends the whole drawing again, not just the letters.
  - A new letter re-sends a whole 4 MB letter page.
  - Pictures the AI looks at still show text as boxes.
  - Very small text can shimmer.
  - Some rare Arabic cases: marks on reversed text, joiner characters, and mixed old and new letter forms in one text.
  - The properties palette's text height ignores the block's scale.
- **Xrefs:**
  - Clipped xrefs are not clipped, so they can over-count on real sheet sets (Phase 3).
  - The AI working by handle alone can confuse an xref entity with a drawing entity.
  - An attribute inside an xref does not hide with it.
  - Blocks inside an xref measure as a count.
  - An xref inside a drawing block is drawn but not counted or searched.
  - Reloading an xref holds the app for a moment.
- **PDF and images:**
  - Underlay clip frames are ignored.
  - Underlays and images inside blocks are not drawn.
  - Polygonal and inverted image clips show the whole image.
  - Transparent PNGs lose their transparency.
  - Only an image's frame can be clicked, not its picture.
  - The brightness and contrast curve is a guess.
  - Plain Save rewrites some relative links.
  - "Yes" at "Save changes?" skips the "opened with problems" warning.
- **Snaps:**
  - On very dense drawings a true End can be missed (8 candidates are looked at).
  - Perpendicular needs the cursor near the foot.
  - Node snaps only find points at the top level.
  - A self-crossing line gives no Intersection.
  - The marker lingers one move after leaving a snap.
  - Arcs in stretched blocks lose Centre and Mid.
- **Tabs:** each tab's zoom and position are not remembered (switching back fits the sheet). A rare timing case remains when switching tabs fast while zooming.
- **Inquiry:**
  - Extract data stops at 5,000 rows without saying so.
  - The step has no unit in its description.
  - Measure along on mirrored arcs is unverified.
  - Every drawing undo step copies the drawing's tables.
- **Screen:**
  - The status bar shows drawing units, not metres. It was ruled to show metres with an "m".
  - Extract data writes numbers to Excel as text.
  - After COUNT or AI, Enter repeats nothing.
  - Aliases can only be changed in `settings.json`.
  - Holding Space on a tab repeats.
  - Clicking a button drops keyboard focus.

## What you should check yourself

**In AutoCAD (Qiyas follows AutoCAD's documented rules, but no agent could compare):**

1. **PDF underlay size.** In AutoCAD, attach a PDF at scale 1 to a DWG and save. Open it in Qiyas: same size and place? Then the reverse: attach in Qiyas, save, open in AutoCAD.
2. **Image turn and clip.** Attach a phone photo taken upright (with an EXIF turn flag) in Qiyas, save, open in AutoCAD. Is it turned the same way? Then clip an image with a rectangle in AutoCAD and open it in Qiyas: is the same part kept?
3. **Xref layers.** Open a real drawing with live (unbound) xrefs. Do the xref layers list, hide and colour as in AutoCAD? Does Locate find a moved xref?

**Your own drawings:**

4. Open drawings with Arabic text from your projects, especially old ones made with Arabic SHX fonts or from other Arabic CAD tools. Does any Arabic look backwards or broken? If so, try View, "Arabic text is reversed".
5. Open your biggest drawing. Is about 0.6 s per layer click acceptable for now?

**Screen and window:**

6. Try a high-DPI or external 4K screen: are text and lines sharp, and does moving the window between screens redraw correctly?
7. **Do a full drive-through in the real Tauri window** (no agent could see it):
   - Open two drawings as tabs.
   - Type `DI` and two points.
   - Snap to an end.
   - Click a door and read the palette.
   - Attach a PDF and calibrate it on a known dimension.
   - Attach a photo.
   - Save as DWG and reopen.
   - Switch to Arabic.

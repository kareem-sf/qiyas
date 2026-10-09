# Qiyas Phase 3 (edit and annotate): final check

Date: 2026-10-09. Qiyas commit `48deb76` (branch main, not pushed). Same laptop as the earlier checks.

## In short

- All local checks pass: 611 Rust tests and 399 front-end tests, no warnings. The PDF tests ran (pdfium is installed).
- **The spec's Phase 3 check works inside Qiyas.** On the test plan and on copies of both your drawings, Qiyas redrew a wall, added a dimension and a revision cloud, saved as DWG and DXF and reopened them. Every entity came back, and every take-off row still points at its entity. Whether AutoCAD shows the same thing is still for you to check: this laptop has no other CAD program.
- **One real problem: the big drawing is now about 6 times slower to redraw.** Every drawing change, undo and layer click on the ALJ drawing takes about 3.6 to 3.9 s, up from about 0.65 s at the end of Phase 1b. On KMQ it is about 0.7 s, up from about 0.1 s. I traced it to one commit, `502c371` (fills, hatch patterns, rays). The time goes on drawing what is inside block references. Not fixed in this check.
- **One wrong number: a hatch with islands measures too big.** Measuring a hatch adds the islands' area instead of taking it away. On an ALJ room it gave 10.392 m² for a region of 7.577 m² (37 % too much).
- Hatch by clicking inside a room works, but follows the drawn line pieces. Next to a circle the area was about 3 % off. On real office plans, a click finds the floor between furniture, not the whole room.
- Nothing crashed. Undo, redo, explode, find and replace, layer states, copy and paste between two drawings, autosave and recovery all did what they should.

## What Phase 3 added

- **Undo for drawing changes.** One step per command, at most 200.
- **Drawing tools:** line, polyline, rectangle, polygon, circle, arc, ellipse, spline, point, donut, ray, construction line, revision cloud, wipeout, hatch (click inside or pick shapes), 2D solid, trace, multiline, sketch.
- **Modify tools:** move, copy, rotate, scale, mirror, stretch, align, arrays, explode, erase. Also trim, extend, break, join, lengthen, reverse, offset, fillet, chamfer, blend, polyline edit, overkill and flatten. A box selection picks many at once.
- **The take-off follows edits.** Rows on a moved or trimmed entity are measured again. Rows on an erased or exploded one keep their value but lose their link.
- **Text:** single-line text, multiline text, text edit, find and replace.
- **Dimensions:** all the usual kinds, leaders, tolerances, dimension styles and an annotation scale.
- **Tables and fields:** area, length, count, date and file name.
- **Blocks:** create, insert, write to a file, edit in place, attributes, a block palette, Design Center and a tool palette.
- **Layers:** properties, layer states, a layer translator, freezing a layer in one viewport, groups, match properties, and an editable properties palette.
- **Clipboard:** Ctrl+C and Ctrl+V between drawings and other apps.
- **Layouts and views:** paper-space viewports, a split view, UCS and sheet sets.
- **Autosave every 5 minutes**, and a recovery dialog after a crash. Also Recover for damaged files.

## Check script

`scripts/check.sh` in the repo (not a copy), with `CARGO_BUILD_JOBS=2`: exit 0.

- Rust: 611 passed, 0 failed, 0 ignored. The PDF tests ran: 5 in `qiyas-pdf` and 9 in `qiyas-core`.
- The release speed test passed: 1 million segments indexed in under 0.6 s.
- Front end: 399 tests in 62 files.
- fmt, clippy and tsc were clean.

The check ran again after adding `examples/phase3_walkthrough.rs`, with the same result.

## Numbers per drawing

All timings are from release builds, on copies of the drawings. `bench_open` was run three times. The new walkthrough (`cargo run --release -p qiyas-core --example phase3_walkthrough`) was run on each drawing. "Phase 1b" is the end of the last check. "Phase 3 start" is commit `3f7a890`, rebuilt and run today.

**KMQ X GAC 17.09.2026.dwg** (1.61 MB, office floor plans, metres)

| What | Phase 1b | Phase 3 start | Now |
|---|---|---|---|
| Open | 25–29 ms | – | 28–32 ms |
| First draw (and every redraw) | 101–125 ms | 101–112 ms | **690–730 ms** |
| of which: click index | 60–79 ms | 62–63 ms | 78–79 ms |
| Line segments | 321,239 | – | 347,843 |
| Batch sent to the screen | 9.1 MB | – | 11.1 MB |
| Problems | 0 | – | 0 |

**ALJ HO - Option B Design Package.dwg** (2.65 MB, about 40 sheets in model space, metres)

| What | Phase 1b | Phase 3 start | Now |
|---|---|---|---|
| Open | 73–80 ms | – | 78–86 ms |
| First draw (and every redraw) | 600–720 ms | 656–664 ms | **3,440–4,040 ms** |
| of which: click index | 335–400 ms | 340–356 ms | 369–393 ms |
| Line segments | 1,388,323 | 1,388,323 | 1,485,161 |
| Batch sent to the screen | 49.7 MB | – | 59.3 MB |
| Memory, one drawing open and drawn | – | – | about 630 MB |
| Problems | 0 | – | 0 |

**Each step on the real drawings** (the walkthrough). "Redraw" is building and sending the sheet again. The window does this after every change.

| Step | KMQ | ALJ |
|---|---|---|
| Draw a wall (polyline) | 5 ms + 0.68 s redraw | 23 ms + 3.78 s redraw |
| Undo it | 6 ms + 0.68 s redraw | 23 ms + 3.57 s redraw |
| Layer click (hide, then show) | 0.68 s, 0.70 s | 3.44 s, 3.76 s |
| Linear dimension | 7 ms + 0.69 s redraw | 28 ms + 3.53 s redraw |
| Revision cloud | 6 ms | 25 ms |
| Explode the biggest block | "Man", 2,153 entities: 3 ms + 0.69 s | "BASE-B", 2,876 entities: 6 ms + 3.86 s |
| Undo the explode | 8 ms + 0.69 s | 27 ms + 4.04 s |
| Window over the whole sheet | 26 ms, 6,182 entities | 134 ms, 47,137 entities |
| Crossing over the whole sheet | 30 ms | 167 ms |
| Hatch by a click in a room | 41 ms | 39 ms + 28 ms |
| Find, then replace, whole drawing | 1 ms, 7 ms | 15 ms, 42 ms |
| Autosave | 20 ms (1.29 MB) | 70 ms (2.69 MB) |
| Recover from the autosave | 23 ms | 95 ms |
| Save as DWG | 25 ms (1.30 MB) | 74 ms (2.71 MB) |
| Save as DXF | 39 ms (7.70 MB) | 111 ms (17.73 MB) |
| Reopen the DWG, the DXF | 22 ms, 68 ms | 76 ms, 196 ms |

The commands themselves are fast: under 30 ms even on the big drawing. Almost all the waiting is the redraw.

## Real-file checks (through the core, no window)

The new example `examples/phase3_walkthrough.rs` does the spec's check and more. It copies the drawing to a temp folder and never writes to the one you give it. Then it does these steps:

1. Draws a wall on A-WALL, then undoes and redoes it.
2. Hides and shows a layer.
3. Adds a linear dimension and a revision cloud.
4. Trims a line and moves a block.
5. Makes a block with an attribute and inserts it.
6. Edits a text and hatches by a click.
7. Runs find and replace, and saves and restores a layer state.
8. Adds a table, then explodes the biggest block and undoes it.
9. Copies and pastes with a second drawing, both ways.
10. Autosaves and recovers.
11. Saves as DWG and DXF, then reopens both.

It ran on `samples/test-plan.dwg` and on copies of your two drawings. The copies are deleted.

**Save and reopen.** Everything came back, in both DWG and DXF, with 0 problems.

| Drawing | Entities on the sheet after the edits | Back after DWG / DXF | Take-off rows still linked |
|---|---|---|---|
| Test plan | 38 (9 kinds) | 38 / 38, same kinds | 6 of 6 |
| KMQ | 6,231 (13 kinds) | 6,231 / 6,231, same kinds | 6 of 6 |
| ALJ | 47,256 (13 kinds) | 47,256 / 47,256, same kinds | 5 of 5 |

In every reopened file, three things read back: the attribute value "W1", a table cell and the edited text. The new wall measured the same before and after: 8.680 m on the test plan, 48.352 m on KMQ and 203.837 m on ALJ.

**Exact numbers on the test plan** (drawn in metres; the working unit was 0.62 m):

| What | Expected | Qiyas |
|---|---|---|
| The new wall (8 + 6 units) | 8.680 m | 8.680 m |
| Its dimension (8 units) | 4.960 m | 4.960 m |
| A 1.86 m line trimmed in the middle | 0.930 m | 0.930 m |
| Living room found by a click | 58.290 m², less 4 column corners of 0.0025 m² | 58.280 m² |
| The fountain, an island in it | 7.069 m² | 6.797 m² (drawn as straight pieces, 3.8 % small) |
| Measuring that hatch | 58.280 − 6.797 = 51.483 m² | **65.077 m²** (the island added, not taken away) |
| A rectangle cut by the fountain | 1.899 m² | 1.952 m² (2.8 % big, same reason) |

**Other checks, all as expected, on all three drawings:**

- Undo removed the new wall. Redo brought it back under the same handle.
- After the layer was turned off, restoring the layer state turned it back on.
- Exploding the biggest block, then undoing it, left the take-off rows as they were.
- Find and replace changed exactly the one edited text.
- Copy and paste:
  - Seven entities went from the test plan to a second drawing, and all seven arrived: a dimension, 2 hatches, a block, 2 polylines and a table. From ALJ it was six, because one hatch was refused (see below).
  - Paste as block worked.
  - A DOOR block copied back from the second drawing pasted fine.
- Autosave and recovery:
  - The autosave was offered for recovery and opened as "walk (recovered).dwg", unsaved.
  - All entities and rows were there.

**Hatch by a click on your drawings:**

- On KMQ, a click near "MAIN MEETING ROOM" found a 17.009 m² region with 76 islands (chairs and similar).
- On ALJ, a click near the same label on another floor found 8.985 m² with 21 islands.
- On these office plans, the furniture lines cut the room into pieces. So a click finds the floor between the furniture, not the whole room. A click right on the label found the 1.8 m² meeting table.
- A 44 × 36 m rectangle drawn in the middle of the ALJ sheet was refused as "too dense, zoom in". That is by design: there were more than 50,000 line pieces in the search box.
- The first try, on a "Chair Ergonomic Office" label on KMQ, also gave "zoom in", not "no closed region". The search grows until it hits the limit.

## What broke or looked wrong

Nothing crashed. No product code was changed in this check.

1. **The redraw is about 6 times slower on drawings full of blocks.** This is the biggest problem from this phase.
   - On ALJ, every drawing change, undo, redo and layer click now waits about 3.6 to 3.9 s. It was 0.65 s at the start of Phase 3. On KMQ it is 0.7 s, up from 0.1 s.
   - I rebuilt and timed the commits one by one. The jump comes from `502c371` "fill triangles, hatch pattern lines, rays and construction lines": KMQ went from 101–106 ms to 664–669 ms, and ALJ from 642–655 ms to 3,877–3,943 ms.
   - The click index is not the cause: it stayed at 0.35–0.39 s on ALJ. The cost is in drawing what sits inside block references. On KMQ, deleting the 705 block references on the sheet cut the draw from 618 ms to 13 ms. Deleting all loose hatches, lines or dimensions changed nothing.
   - I did not find the exact line. KMQ's blocks hold only 79 hatches, so it may not be the hatches alone.
   - Every change still redraws the whole sheet. Building the click index in the background, carried from Phase 1b, was not done.
2. **Measuring a hatch with islands gives too much.** The islands' area is added instead of taken away.
   - Test plan: 65.077 m², should be 51.483 m².
   - KMQ: 17.244 m², should be 16.774 m².
   - ALJ: 10.392 m², should be 7.577 m².
   - This matters for your own drawings too: any hatch with islands that a QS measures will be over-measured.
3. **Hatch by a click follows straight line pieces, not true arcs.** The region is found on the lines as drawn on screen. Next to the test plan's round fountain, the area was 2.8 % too big. The fountain island came out 3.8 % too small.
4. **On real office plans, a click finds the space between furniture.** The QS has to hide the furniture layers first, or use the Area tool. A point with no closed shape nearby says "zoom in" instead of "no closed region".
5. **The check script still has a gap.** If the shared build folder is used without pdfium, the PDF tests are skipped silently. A guard that fails in that case was planned for this task. It is not added yet (this check changed no code).

## Phase 3 limits (left for later)

From the build log's "minor (deferred)" lines and the rulings, in plain words. None of them blocks Phase 3, but items 1 and 2 above should be fixed before you rely on edits in the big drawings.

- **Not done this phase, by ruling:**
  - Draw, modify and dimension commands are not AI tools yet. The assistant cannot draw.
  - No grips.
  - The split view only shows. Tools work in the main view.
  - Two drawings side by side wait for a multi-drawing window.
  - Lock files, the crash hook, AUDIT and printing a sheet set are Phase 4.
- **Drawing:**
  - No draw order: a wipeout hides every line under it, even lines drawn later.
  - Hatch "too dense" counts text boxes and pattern lines too, so it says "zoom in" more often than needed.
  - Rays run past the sheet's edges.
  - A zoom window drops a polyline that is half drawn.
- **Modify:**
  - Fillet and chamfer work on two lines, or two straight sides of one polyline, only.
  - Dimensions and tables can only be moved and copied.
  - Images and PDF underlays move but are not copied.
  - Erasing a dimension or table leaves its hidden block in the file.
  - Exploding a block loses constant attribute values.
  - Moving or erasing many entities while many rows follow them is slow (the follow-up scan is quadratic).
  - The same handle twice in one selection is not cleaned up.
- **Text and dimensions:**
  - Find can count more matches than Replace changes, because of skipped texts.
  - Dimension breaks and jogs are lost when the dimension is rebuilt.
  - Dimensions are not measured again after a trim or stretch.
  - Some continued and baseline dimensions from an aligned base slant.
  - Fields are not worked out when a drawing opens.
  - No annotative scale lists. Qiyas writes leaders, not AutoCAD's multileaders.
- **Blocks:**
  - Edit in place refuses arrayed or unevenly scaled references, and blocks holding dimensions or tables.
  - Saving an unchanged edit still clears the undo history.
- **Layers:**
  - Locked layers block a little more than AutoCAD does: they also refuse copy, array, blend and explode sources.
  - Find and replace, update fields and attribute sync ignore the lock.
- **Viewports, clipboard and recovery:**
  - Viewports do not draw pictures or twisted views.
  - Quitting with Don't Save at the moment of an autosave can leave an autosave behind.
  - A clean tab's old autosave is not deleted.
  - The kept clipboard is not reset if the display connection drops.
  - `LICENSES/` has no Zlib or BSL-1.0 licence texts yet.

## What you should check yourself

No agent could see the app window or open the files in another CAD program. To make the test file:

`cargo run --release -p qiyas-core --example phase3_walkthrough -- --keep ~/qiyas-phase3-check`

This writes `check.dwg` and `check-dxf.dxf` there from the test plan. They hold a dimension, two hatches (one with an island), a table, a block with an attribute (MARK = W1) and a revision cloud. Put a drawing path after `--` to run it on a copy of one of your drawings.

**In AutoCAD or the free ODA viewer:**

1. Open `check.dwg`, then `check-dxf.dxf`. Do the dimension (text and arrows), both hatches, the table, the block with its attribute value and the revision cloud look as they do in Qiyas? Is anything missing, or is there an error on open?
2. Run AUDIT in AutoCAD on `check.dwg`. Does it report errors?

**In the real Qiyas window:**

3. **Clipboard on Wayland:**
   - Copy entities in Qiyas and paste them into a second Qiyas drawing.
   - Copy them again and paste into a text editor. You should see DXF text.
   - Copy a line of text from another app and paste it into Qiyas. It should become a text.
4. **A drive-through of each ribbon tab:**
   - Home: draw, modify, text, layers, clipboard.
   - Annotate: dimensions, leaders, table, field.
   - Insert: create, insert and edit a block in place.
   - View: viewports, split view, UCS, sheet set.
   - Measure.
   - Watch for anything that does not respond, and time a few changes on the ALJ drawing (expect about 4 s, see above).
5. **Recovery:**
   - Edit a drawing, wait for the autosave (set 1 minute in Settings), then kill the app (`pkill qiyas`).
   - Start it again. Does the recovery dialog appear?
   - Does Recover open the drawing with your edits and your take-off?
6. **Arabic:**
   - Type Arabic in a single-line text, a multiline text and a dimension override, in both English and Arabic mode.
   - Is it joined and right to left? Does it stay right after save and reopen, and in AutoCAD?
7. **The split pane:** does it follow edits in the main view and keep its own zoom?
8. **The layer translator against a real standard:**
   - Use your office's layer standard drawing (or a client's) as the source on one of your drawings.
   - Are the renames, merges and colours what you expect?
9. **Speed:** is about 0.7 s per change on a KMQ-sized drawing acceptable for now, before the redraw fix?

# Qiyas Phase 3 (edit and annotate): final check

Date: 2026-10-09. Qiyas commit `48deb76` (branch main, not pushed). Same laptop as the earlier checks.

Checked again on 2026-10-12, after the final fix wave (`8342692` to `61f8eee`) and four more commits from this re-check (`99ed496`, `c3078c8`, `4977add`, `1f0981a`). The speed numbers, the hatch areas and the walkthrough were run again on the test plan and on copies of your two drawings (copies deleted after). Nothing was seen in a window, and no other CAD program opened the files.

## In short

- **After the fix wave, the redraw is much faster again.** On ALJ the first draw (and every redraw) took about 1.0 to 1.5 s, down from 3.4 to 4.0 s. On KMQ it took about 0.3 to 0.4 s, down from 0.7 s. The laptop was busy while these ran (other agents building, load average 6 to 10, swap in use), so these are upper bounds; the first check ran on a quiet laptop. Both are still above Phase 1b (0.6 to 0.72 s on ALJ, 0.1 s on KMQ): part of the gap is the load, part is what Phase 3 draws more (fills, pattern lines; the batch grew from 49.7 to 59.3 MB on ALJ and 9.1 to 11.1 MB on KMQ). A run on a quiet laptop would tell how much.
- **Hatch areas are right now.** A hatch takes its islands away. On the test plan, the living room hatched round the fountain measures 51.211 m² (58.280 − 7.069), where it gave 65.077 m² before. On copies of your drawings, the room hatches measure their region less their islands (KMQ 6.363 m², ALJ 7.584 m²).
- **Hatch by a click follows true circles and arcs.** A click inside the fountain gives 7.069 m², exactly 2.25π. Before, it followed the 5 cm pieces drawn on screen and was 3 to 4 % off.
- **This re-check found two more hatch problems, both fixed the same day:**
  - A hatch by a click with islands, saved as DWG, came back broken. On ALJ, reopening the saved file crashed Qiyas. Saving as DXF was fine.
  - A click region with an island touching its wall measured the island in instead of out: 6.932 m² on a KMQ room of 6.363 m².
- **The spec's Phase 3 check still works inside Qiyas.** On the test plan and on copies of both your drawings, Qiyas redrew a wall, added a dimension and a revision cloud, hatched a room by a click, saved as DWG and DXF and reopened them. Every entity came back, and every take-off row still points at its entity. Whether AutoCAD shows the same thing is still for you to check: this laptop has no other CAD program.
- On real office plans, a click still finds the floor between furniture, not the whole room. That is by design.

## What Phase 3 added

- **Undo for drawing changes.** One step per command, at most 200.
- **Drawing tools:** line, polyline, rectangle, polygon, circle, arc, ellipse, spline, point, donut, ray, construction line, revision cloud, wipeout, hatch (click inside or pick shapes), 2D solid, trace, multiline, sketch.
- **Modify tools:** move, copy, rotate, scale, mirror, stretch, align, arrays, explode, erase. Also trim, extend, break, join, lengthen, reverse, offset, fillet, chamfer, blend, polyline edit, overkill and flatten. A box selection picks many at once.
- **The take-off follows edits.** Rows on a moved or trimmed entity are measured again. Rows on an erased or exploded one are measured again without it, or keep their value and lose their link when nothing of theirs is left.
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

All timings are from release builds, on copies of the drawings. `bench_open` was run three times. The new walkthrough (`cargo run --release -p qiyas-core --example phase3_walkthrough`) was run on each drawing. "Phase 1b" is the end of the last check. "Phase 3 start" is commit `3f7a890`, rebuilt and run on 2026-10-09. "Before the fix wave" is the first check (`48deb76`). "After the fix wave" is the re-check on 2026-10-12. The re-check ran while other agents were building on the same laptop (load average 6 to 10, swap in use): its times are upper bounds. `bench_open` ran eight times per drawing; the ALJ range leaves out four runs taken while the laptop was swapping (1.9 to 3.7 s).

**KMQ X GAC 17.09.2026.dwg** (1.61 MB, office floor plans, metres)

| What | Phase 1b | Phase 3 start | Before the fix wave | After the fix wave |
|---|---|---|---|---|
| Open | 25–29 ms | – | 28–32 ms | 33–51 ms |
| First draw (and every redraw) | 101–125 ms | 101–112 ms | 690–730 ms | **259–389 ms** |
| of which: click index | 60–79 ms | 62–63 ms | 78–79 ms | 89–108 ms |
| Line segments | 321,239 | – | 347,843 | 347,843 |
| Batch sent to the screen | 9.1 MB | – | 11.1 MB | 11.1 MB |
| Problems | 0 | – | 0 | 0 |

**ALJ HO - Option B Design Package.dwg** (2.65 MB, about 40 sheets in model space, metres)

| What | Phase 1b | Phase 3 start | Before the fix wave | After the fix wave |
|---|---|---|---|---|
| Open | 73–80 ms | – | 78–86 ms | 99–118 ms |
| First draw (and every redraw) | 600–720 ms | 656–664 ms | 3,440–4,040 ms | **1,052–1,223 ms** |
| of which: click index | 335–400 ms | 340–356 ms | 369–393 ms | 453–990 ms |
| Line segments | 1,388,323 | 1,388,323 | 1,485,161 | 1,485,161 |
| Batch sent to the screen | 49.7 MB | – | 59.3 MB | 59.3 MB |
| Memory, one drawing open and drawn | – | – | about 630 MB | not measured |
| Problems | 0 | – | 0 | 0 |

**Each step on the real drawings** (the walkthrough). "Redraw" is building and sending the sheet again. The window does this after every change.

| Step | KMQ, before | KMQ, after the fix wave | ALJ, before | ALJ, after the fix wave |
|---|---|---|---|---|
| Draw a wall (polyline) | 5 ms + 0.68 s redraw | 6 ms + 0.29 s redraw | 23 ms + 3.78 s redraw | 35 ms + 1.23 s redraw |
| Undo it | 6 ms + 0.68 s redraw | 6 ms + 0.39 s redraw | 23 ms + 3.57 s redraw | 36 ms + 1.31 s redraw |
| Layer click (hide, then show) | 0.68 s, 0.70 s | 0.30 s, 0.33 s | 3.44 s, 3.76 s | 1.20 s, 1.40 s |
| Linear dimension | 7 ms + 0.69 s redraw | 11 ms + 0.40 s redraw | 28 ms + 3.53 s redraw | 39 ms + 1.21 s redraw |
| Revision cloud | 6 ms | 9 ms | 25 ms | 40 ms |
| Explode the biggest block | "Man", 2,153 entities: 3 ms + 0.69 s | "Man": 8 ms + 0.36 s | "BASE-B", 2,876 entities: 6 ms + 3.86 s | "BASE-B": 12 ms + 1.48 s |
| Undo the explode | 8 ms + 0.69 s | 9 ms + 0.28 s | 27 ms + 4.04 s | 41 ms + 1.41 s |
| Window over the whole sheet | 26 ms, 6,182 entities | 38 ms, 6,182 entities | 134 ms, 47,137 entities | 158 ms, 47,137 entities |
| Crossing over the whole sheet | 30 ms | 54 ms | 167 ms | 200 ms |
| Hatch by a click in a room | 41 ms | 64 ms (another room, 20 islands) | 39 ms + 28 ms | 116 ms (another room, 140 islands) |
| Find, then replace, whole drawing | 1 ms, 7 ms | 2 ms, 12 ms | 15 ms, 42 ms | 29 ms, 68 ms |
| Autosave | 20 ms (1.29 MB) | 31 ms (1.36 MB) | 70 ms (2.69 MB) | 88 ms (2.73 MB) |
| Recover from the autosave | 23 ms | 33 ms | 95 ms | 124 ms |
| Save as DWG | 25 ms (1.30 MB) | 40 ms (1.39 MB) | 74 ms (2.71 MB) | 119 ms (2.73 MB) |
| Save as DXF | 39 ms (7.70 MB) | 65 ms (8.08 MB) | 111 ms (17.73 MB) | 134 ms (17.72 MB) |
| Reopen the DWG, the DXF | 22 ms, 68 ms | 24 ms, 91 ms | 76 ms, 196 ms | 120 ms, 226 ms |

The drawing commands themselves are fast: under 50 ms even on the big drawing. Almost all the waiting is still the redraw.

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

It ran on `samples/test-plan.dwg` and on copies of your two drawings. The copies are deleted. In the re-check, the room hatch on your drawings was clicked at a point given with `--room` (KMQ `40.890,54.329`, beside the first "MAIN MEETING ROOM" label; ALJ `-126.610,102.729`, beside its label): the first check's points were not written down, and the first room label the walkthrough finds on its own ("Chair Ergonomic Office") gives "zoom in".

**Save and reopen.** Everything came back, in both DWG and DXF, with 0 problems, before and after the fix wave.

| Drawing | Entities on the sheet after the edits | Back after DWG / DXF | Take-off rows still linked |
|---|---|---|---|
| Test plan | 38 (9 kinds) | 38 / 38, same kinds | 6 of 6 |
| KMQ | 6,231 (13 kinds) | 6,231 / 6,231, same kinds | 6 of 6 |
| ALJ | 47,256 (13 kinds) | 47,256 / 47,256, same kinds | 5 of 5 |

In every reopened file, three things read back: the attribute value "W1", a table cell and the edited text. The new wall measured the same before and after: 8.680 m on the test plan, 48.352 m on KMQ and 203.837 m on ALJ.

The walkthrough counts entities by kind; it did not compare a hatch's paths. That is how the broken DWG islands (below) got past the first check: the hatch came back as a hatch, with garbage paths. A test now compares them.

**Exact numbers on the test plan** (drawn in metres; the working unit was 0.62 m):

| What | Expected | Before the fix wave | After the fix wave |
|---|---|---|---|
| The new wall (8 + 6 units) | 8.680 m | 8.680 m | 8.680 m |
| Its dimension (8 units) | 4.960 m | 4.960 m | 4.960 m |
| A 1.86 m line trimmed in the middle | 0.930 m | 0.930 m | 0.930 m |
| Living room found by a click | 58.290 m², less 4 column corners of 0.0025 m² | 58.280 m² | 58.280 m² |
| The fountain, an island in it | 7.069 m² | 6.797 m² (straight pieces, 3.8 % small) | 7.069 m² (an arc) |
| Measuring that hatch | 58.280 − 7.069 = 51.211 m² | **65.077 m²** (the island added) | 51.211 m² |
| A click inside the fountain | 7.069 m² | – | 7.069 m² |
| A rectangle cut by the fountain | 1.898 m² (1.86 × 1.55 less the fountain's part) | 1.952 m² (2.8 % big) | 1.898 m² |

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

**Hatch by a click on your drawings** (after the fix wave):

- On KMQ, the click beside "MAIN MEETING ROOM" found a 7.785 m² region with 20 islands (chairs and similar, 1.422 m²). The hatch measures 6.363 m²: the region less its islands.
- On ALJ, the click beside its "MAIN MEETING ROOM" found 7.819 m² with 140 islands (0.235 m²). The hatch measures 7.584 m².
- In the first check, other clicks gave KMQ 17.244 m² for 16.774 m² and ALJ 10.392 m² for 7.577 m²: the islands added. Those points were not written down, so they were not repeated.
- On these office plans, the furniture lines cut the room into pieces. So a click finds the floor between the furniture, not the whole room. A click right on the KMQ label found the 1.818 m² meeting table.
- A 44 × 36 m rectangle drawn in the middle of the ALJ sheet was refused as "too dense, zoom in". That is by design: there were more than 50,000 line pieces in the search box.
- The first room label the walkthrough finds on both drawings, "Chair Ergonomic Office", also gives "zoom in", not "no closed region". The search grows until it hits the limit.

## What broke or looked wrong

Items 1, 2, 3 and 5 below are fixed. Items 6 and 7 were found in the re-check and fixed. Item 4 is how the tool works.

1. **Fixed: the redraw was about 6 times slower on drawings full of blocks** (`8342692`).
   - On ALJ, every drawing change, undo, redo and layer click waited about 3.6 to 3.9 s, up from 0.65 s at the start of Phase 3. On KMQ it was 0.7 s, up from 0.1 s. The jump came from `502c371` "fill triangles, hatch pattern lines, rays and construction lines".
   - The cause: a fill (a solid hatch, a 2D solid or a trace) inside a block was cut into triangles again for every reference to that block, not once per fill. Cutting a fill into triangles costs more the more corners it has (with the square of them), so a block with a big fill, placed hundreds of times, did that work hundreds of times on every redraw.
   - Now each fill is cut once per redraw and the triangles are placed for every reference. A fill of more than 2,000 corners draws its outline only. A release-only test in `scripts/check.sh` guards it.
   - After the fix: ALJ about 1.0 to 1.5 s, KMQ about 0.3 to 0.4 s per redraw (see the tables).
   - Every change still redraws the whole sheet. Building the click index in the background, carried from Phase 1b, was not done.
2. **Fixed: measuring a hatch with islands gave too much** (`5de3ab5`, and `1f0981a` from the re-check).
   - The cause: the areas of all the hatch's boundary paths were added, so the islands were added instead of taken away. Now a hatch measures even-odd, as it is filled: a path inside an odd number of others is taken away.
   - Before: test plan 65.077 m², KMQ 17.244 m² for 16.774 m², ALJ 10.392 m² for 7.577 m².
   - After: the test plan's living room measures 51.211 m², exactly 58.280 − 7.069. The room hatches on your drawings measure their region less their islands (see above).
3. **Fixed: hatch by a click followed straight line pieces, not true arcs** (`1829c6d`, `78c7065`, `61f8eee`).
   - The cause: the region was found on the lines as drawn on screen, which cut curves into pieces up to 5 cm off (0.05 drawing units), not on the drawing.
   - Now the entities near the click are cut again as finely as Qiyas measures (1 mm in a metre drawing). An edge along a drawn circle or arc becomes that arc in the hatch, so it measures exactly. Multilines give their drawn faces, rays and construction lines close a region, and blocks and array cells count as drawn.
   - After: a click inside the fountain gives 7.069 m², exactly 2.25π (the island was 6.797 m²). The rectangle cut by the fountain gives 1.898 m² (was 1.952 m²).
4. **On real office plans, a click finds the space between furniture.** The QS has to hide the furniture layers first, or use the Area tool. A point with no closed shape nearby says "zoom in" instead of "no closed region".
5. **Fixed: the check script had a gap** (`b6d5618`). Where pdfium is installed, `scripts/check.sh` now fails if the PDF tests are skipped.
6. **Found in the re-check, fixed (`4977add`): a hatch by a click with islands came back broken from a saved DWG.**
   - The islands' paths lost their "polyline" mark when they were made, and the DWG writer then wrote each one in a form the reader can't read back. The hatch reopened with garbage paths.
   - On ALJ, the hatch with 140 islands, saved as DWG and reopened, crashed Qiyas while drawing. On the test plan and KMQ the file opened, but the hatch's islands were wrong. DXF was not affected.
   - Now the islands keep the mark. A test saves a hatch with an island as DWG and as DXF and checks its paths and its area come back the same.
7. **Found in the re-check, fixed (`1f0981a`): a click region with an island touching its wall measured the island in.**
   - When furniture touches a wall, the region found by a click runs round it and touches itself there. Such a path was measured as the sum of its loops, so the piece of furniture was added.
   - KMQ: 6.932 m² for a region of 6.363 m². Now each loop goes into the even-odd count with the hatch's other paths: 6.363 m².

## Phase 3 limits (left for later)

From the build log's "minor (deferred)" lines and the rulings, in plain words. None of them blocks Phase 3.

- **Not done this phase, by ruling:**
  - Draw, modify and dimension commands are not AI tools yet. The assistant cannot draw.
  - No grips.
  - The split view only shows. Tools work in the main view.
  - Two drawings side by side wait for a multi-drawing window.
  - Lock files, the crash hook, AUDIT and printing a sheet set are Phase 4.
- **Drawing:**
  - No draw order: a wipeout hides every line under it, even lines drawn later.
  - Rays run past the sheet's edges.
  - A zoom window drops a polyline that is half drawn.
- **Modify:**
  - Fillet and chamfer work on two lines, or two straight sides of one polyline, only.
  - Dimensions and tables can only be moved and copied.
  - Images and PDF underlays move but are not copied.
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
  - Find and replace and attribute sync leave locked entities as they are and say how many. Fields on a locked layer still update when what they measure changes, as in AutoCAD (ruling of 2026-10-12): a field is a worked-out value, and the lock stops the user, not the field.
- **Viewports, clipboard and recovery:**
  - Viewports do not draw pictures or twisted views.
  - Quitting with Don't Save at the moment of an autosave can leave an autosave behind.
  - A clean tab's old autosave is not deleted.
  - The kept clipboard is not reset if the display connection drops.
- **Fixed in the fix wave** (were on this list): hatch "too dense" no longer counts text boxes and pattern lines; a saved file leaves out the hidden blocks of erased dimensions and tables; `LICENSES/` has the Zlib and BSL-1.0 texts.

## What you should check yourself

No agent could see the app window or open the files in another CAD program. To make the test file:

`cargo run --release -p qiyas-core --example phase3_walkthrough -- --keep ~/qiyas-phase3-check`

This writes `check.dwg` and `check-dxf.dxf` there from the test plan. They hold a dimension, two hatches (one with an island), a table, a block with an attribute (MARK = W1) and a revision cloud. Put a drawing path after `--` to run it on a copy of one of your drawings.

**In AutoCAD or the free ODA viewer:**

1. Open `check.dwg`, then `check-dxf.dxf`. Do the dimension (text and arrows), both hatches (the fountain must stay empty), the table, the block with its attribute value and the revision cloud look as they do in Qiyas? Is anything missing, or is there an error on open?
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
   - Watch for anything that does not respond, and time a few changes on the ALJ drawing (expect about 1.0 to 1.5 s after the fix wave, see above).
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
9. **Speed:** is about 0.3 to 0.4 s per change on a KMQ-sized drawing, and about 1.0 to 1.5 s on ALJ, fast enough for now?

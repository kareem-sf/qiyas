# Qiyas Phase 3: rulings and items carried forward

Date: 2026-10-09. Source: the build ledger of the Phase 3 plan (`docs/superpowers/plans/2026-10-08-qiyas-phase3-edit-annotate.md`). Check results: `docs/2026-10-09-qiyas-phase3-check.md`.

The founder approved the plan with "Proceed" on 2026-10-08. Every decision below was taken on his behalf while building; each line says what it costs if wrong, so he can undo any of them.

## Rulings made during the build

Undo and commands
- Every step keeps its `Source`, so the assistant's card Undo still undoes only its own step. Phase 1b's whole-drawing snapshots became before-image steps; table-only edits keep a tables snapshot. Cost if wrong: a few extra snapshot steps.
- Every document command bumps the document revision; dirty stays "revision since last save". Cost: none.
- Every command that targets existing entities takes handle plus chain (the pick convention); block or xref content is refused, all or nothing. Why: the AI and MCP wire shape is hard to change later. Cost: a slightly wider API.
- `SetCurrent` (current layer, colour, linetype, lineweight) is view state, not undoable; saved in the project file.
- Edit in place closes by snapshot, never by undo. Save writes the sheet's new entities into the block and clears the whole undo history (like AutoCAD's REFCLOSE); Discard drops them and clears history unless nothing was done. During an edit only CreateBlock, WBLOCK, a nested edit, inserting the block into itself and redefining it are refused. Cost: the close itself cannot be undone.
- Locked layers are enforced: move, erase, property and match-property changes refuse entities on a locked layer. Why: a QS locks the architect's layers to protect them. Cost: an extra refusal path.
- Undo also restores the session's current layer, hidden layers and layer-state hidden lists when the step changed them.

Drawing and rendering
- Fills travel in batch section 12 (triangles) and 13 (fill ranges); pattern hatches are ordinary lines clipped to the boundary. Cost: one section layout to revise.
- Wipeouts draw after all lines so they mask line work; a proper draw order is still missing, so entities drawn later than a wipeout are hidden too.
- Default hatch pattern scale is 0.025 ÷ metres per unit (25 in millimetre drawings, 0.025 in metre drawings), with scale and angle options. Islands inside the picked ring stay empty.
- Multilines measure along their centre line; their element lines are paint only.
- A dimension draws its own `*D` block when it has one, else Qiyas generates the geometry; picking returns the dimension; measuring one gives its measured distance. Editing a dimension or its text override regenerates the block. Editing a dimension style rebuilds every dimension of that style whose block Qiyas made; AutoCAD-made blocks stay untouched.
- An annotation scale other than 1:1 replaces the style's DIMSCALE (never multiplies it); at 1:1 the style's scale applies. Cost if wrong: dimension size on mixed drawings.
- A table draws its grid and cell text generated from its rows, columns and cells. Fields are kept as `QIYAS_FIELD` xdata with the evaluated text visible; they update automatically after any edit of a referenced entity. Area and length fields read in metres, like the take-off, not drawing units. Cost: differs from AutoCAD's own fields.
- Paper-space viewports draw the model batch per viewport with its own camera and scissor; picking inside a viewport returns the viewport. The split view is two panes on the active drawing only and is view only this phase; two drawings side by side wait for a multi-session refactor.
- The UCS is view state written to the header on save; the clipboard is a small DXF text with a Qiyas marker.

Modify tools and the take-off
- Move and Copy translate dimensions and tables with their blocks; other transforms refuse them, and box selections drop them with an "N skipped" notice. Cost: rework if Phase 4 wants more.
- The take-off follows every edit in the same undo step: moves re-measure linked rows; trims and breaks link the new pieces to the row; joins repoint rows; erase and explode drop the link but keep the value; a row that can no longer be measured is unlinked and the outcome says so. Never a silently stale value. Cost: extra work per edit.
- Trim and break keep the original handle on the largest remaining piece; join keeps the first entity's handle; offset keeps exact arcs; chamfer is two trims and a line.
- Explode turns visible attributes into text (BURST style) so door tags survive; it honours the block base point.

Text, blocks, layers, files
- New text is stored in reading order with a marker, so it renders right whatever the drawing's "Arabic text is reversed" setting says. Find and replace covers text, multiline text, attributes, dimension overrides and table cells; it leaves fields alone and counts them as skipped.
- CreateBlock refuses existing or invalid names; WBLOCK writes only inside the drawing's folder or through the dialog and never over the open drawing; the Design Center and layer translator open other drawings read only.
- Layer states are AutoCAD layer states (visible in its Layer States Manager); Qiyas's own hidden set is a view setting saved in the project file. Deleting layer 0, Defpoints, the current layer or a layer in use is refused. Groups are real DWG groups.
- Autosave every 5 minutes (setting) while dirty to `<file>.sv$` beside the drawing, atomically, never during a block edit; Save, a "Don't save" close and Discard delete it; a recovery dialog offers newer autosaves on launch. Recover reports what the failsafe reader dropped. Audit, lock files and the panic hook are Phase 4.
- Zlib and BSL-1.0 licences are allowed (both permissive); the clipboard uses the Wayland feature because the founder's desktop is Wayland.
- The assistant and MCP do not get edit commands this phase: a destructive edit from the model needs a confirmation surface the panel does not have. When they come, text, markup and dimensions first, under the destructive-tool cap, never delete, explode or block edits without confirmation.
- Splitting `src/App.tsx` (about 3,400 lines) is the first task of Phase 4, not part of this phase's fix wave.

## Rulings from the final review and its fix wave (2026-10-12)
- Fills inside block references are triangulated once per fill entity and mapped through every reference; a fill of more than 2,000 corners draws its outline only until the triangulator is replaced (earcutr, ISC). Hatch areas are even-odd (islands taken away). A hatch by click re-flattens the nearby entities exactly (arcs as arcs), never off the screen flattening. Cost if wrong: none seen; the bench shows about 1 s per change on the 1.4 M-segment drawing on a busy laptop (was 3.5 s), 0.3 s on the 0.4 M one.
- Fields are derived values: their values update on locked layers too; the lock blocks only the user's own edits, find and replace and attribute sync.
- A partly erased take-off row is measured again (a count counts one less), never kept stale. WBLOCK keeps a `.bak` and writes atomically. Pasted and imported input is bounded (32 MB, 64 KB plain text, block depth 32, the usual entity cap). Paste scales by the units ratio. Imported entities drop the handles of the drawing they came from. Saved files leave out orphan anonymous blocks and gone group members; memory keeps them for undo.
- The assistant and MCP get no edit commands this phase. Splitting the window code (`src/App.tsx`) is Phase 4's first task.

## Left open by the last re-review (minor)
- The worst-case cost of a hatch click that ends in "zoom in" on the big drawing is not benchmarked; pattern spans are computed and thrown away there.
- Circles and arcs inside arrayed references come back as chords for the click region.
- Check-doc wording slips (run counts, a stale "exit 0" heading, "under 50 ms" against a 116 ms click).
- A full `scripts/check.sh` on a quiet machine is still owed (other agents' files blocked the formatting step on the shared tree).

## Carried to Phase 4
- Redraw still rebuilds the whole sheet per change and the click index on every layer toggle (Phase 1b carry; the spec wants dirty-handle redraw and a background index).
- No draw order yet: wipeouts hide lines drawn after them. A click inside a solid fill picks nothing (the index holds lines only).
- Grips; the split pane is view only; no two drawings side by side.
- Dimensions are not re-measured after a trim or stretch of what they measure; DimBreak and DimJogLine are lost whenever the dimension is rebuilt; Baseline/Continue on an aligned base; no DIMATFIT; fixed-height text styles ignored.
- Fields are not evaluated on open; counts change on UpdateFields only.
- Drag-to-insert from the block palette; renaming attribute tags in BATTMAN; dynamic `*U` block links are lost when blocks are created, imported or edited in place; more than 200 steps inside one in-place edit makes Discard look quiet.
- Take-off follow scans are quadratic on very large selections; the Design Center reopens the other drawing for each insert.
- The UCS also applies on layouts; the viewport "off" flag is read from the wrong bit (codec); rotated AutoCAD tables may rotate twice (needs a fixture); the table editor lacks spreadsheet keys; editing a multiline text drops its formatting; undoing a layer-state restore does not re-hide layers; a zoom window drops a polyline under way; `.qss` path checks are Unix-shaped.
- Replace the kernel's ear-clipping triangulator with earcutr (ISC) and raise the fill-corner cap back; measure `codegen-units = 1` for opencadkernel.
- The AutoCAD checks below.

## Founder checks (need AutoCAD or a real window)
- Open a Qiyas-saved DWG with a dimension, a hatch with an island, a table, a block with attributes, a revision cloud and a WBLOCK file in AutoCAD or ODA viewer; run AUDIT.
- Copy and paste into another app on Wayland; paste from a millimetre drawing into a metre one.
- Drive through every ribbon tab in the real window; the split pane; the viewports on a layout; the recovery dialog after killing the app.
- Arabic text and dimension input; the layer translator against a real office standard; a hatch with an island measured against a hand number.

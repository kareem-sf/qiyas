# Qiyas Phase 2 (AI and BOQ): final check

Date: 2026-10-09. Qiyas commit `1d20d17` (branch main, not pushed). Same laptop as the Phase 1 check.

## In short

- All local checks pass: 187 Rust tests and 70 front-end tests, no warnings.
- The test BOQ against the test drawing gives **exactly the two conflicts it should**: A.4 (bedroom screed, BOQ 30.000 m², drawing 22.090 m²) and B.2 (windows, BOQ 4, drawing 5). Same result through the AI tools and through the app's own screens.
- The assistant works end to end with the fake ("mock") provider. **No real AI model was used**: nobody typed a real API key, so the real Claude replies are still untested.
- The MCP bridge builds, and answers "Qiyas is not running" when the app is off. It was not tried with real Claude Desktop or Claude Code.
- One thing to know before you try it: **compare only works well when measurements are linked to BOQ rows.** Unlinked, the matching found nothing on the test files (see "What broke or looked wrong", 1).

## What Phase 2 added

- An assistant panel. It works with Claude, or with any OpenAI-compatible service. The key stays in the system keychain, never in a file.
- 27 tools the AI can use: read the drawing, measure, count, import a BOQ, link, compare, mark up, zoom, and look at a picture of the sheet. Every change the AI makes can be undone.
- BOQ import from Excel (English or Arabic headers), linking measurements to BOQ rows, and a comparison that lists conflicts in a new Conflicts tab.
- An MCP server inside the app, plus a small `qiyas-mcp` program, so Claude Desktop or Claude Code can drive Qiyas.

## Check script

`scripts/check.sh` on a clean tree: exit 0. It was run before and after this task's changes. Both times: 187 Rust tests and 70 front-end tests (16 files) passed. fmt, clippy and tsc were clean.

## The test files

- `samples/test-plan.dwg` is the small apartment from Phase 1. Every quantity in it is exact.
- `samples/test-boq.xlsx` is new. It has six items, and two are wrong on purpose. It is made by `cargo run -p qiyas-boq --example write_test_boq`. The list is in `samples/test-plan.md`, under "BOQ check".

| Code | Item | BOQ | Drawing |
|---|---|---|---|
| A.1 | Block walls 200 mm | 42.000 m | 42.000 m |
| A.2 | Partition walls | 14.000 m | 14.000 m (9 + 5) |
| A.3 | Floor screed, living room | 58.290 m² | 58.290 m² |
| A.4 | Floor screed, bedroom | **30.000 m²** | 22.090 m² |
| B.1 | Doors 900 mm | 3 pcs | 3 pcs |
| B.2 | Windows 1200 mm | **4 pcs** | 5 pcs |

## Run 1: through the AI tools, no model

A fake provider can't decide what to measure; it only plays a fixed script. So this run calls the AI tools directly, the same way a model would: `cargo run -p qiyas-ai --example phase2_walkthrough`.

What it did and got:

```
measure_area    58.29  "LwPolyline on A-ROOM"  -> linked to A.3
measure_area    22.09  "LwPolyline on A-ROOM"  -> A.4
measure_length  42.0   "LwPolyline on A-WALL"  -> A.1
measure_length  9.0    "Line on A-WALL"        -> A.2
measure_length  5.0    "Line on A-WALL"        -> A.2
count_blocks    3      "DOOR count"            -> B.1
count_blocks    5      "WINDOW count"          -> B.2
import_boq      6 items, 2 sections, 0 rows skipped
compare_boq     2 conflicts, both quantity mismatches:
  A.4  BOQ says 30.000 m2, drawings measure 22.090 m2 (−26.4 %)
  B.2  BOQ says 4 pcs, drawings measure 5 pcs (+25.0 %)
```

This is the expected result. There were no "missing" conflicts of any other kind.

The same run with nothing linked (`-- --no-link`) gave 11 conflicts: 6 "not in drawings" and 5 "missing from BOQ". Without links, the matching rules could not pair any measurement with its BOQ row.

## Run 2: the app's own screens

The app page ran in the built-in browser, connected to the real Qiyas core. A temporary test served the real app commands over local HTTP; it was deleted afterwards, and nothing of it is committed.

1. Opened `test-plan.dwg`.
2. **Assistant (English):** sent "Measure every room area and count the doors, then import test-boq.xlsx (it sits beside the drawing) and compare". The fake provider streamed its text and measured the outer wall ring (108.000 m²). The card's Undo took the measurement back out and the card then read "Undone".
3. **Measured by hand:**
   - The Select tool gave the living room (58.290 m²), the bedroom (22.090 m²) and both partitions (9.000 and 5.000 m).
   - The Distance tool round the four outer corners gave 41.798 m, not 42.000 m. That is a hand-clicking error, and it is inside the 5 % tolerance.
   - The Count tool gave doors (3) and windows (5). For windows, the wall layer had to be hidden first (see point 3 below).
4. **Import BOQ:** 6 items in 2 sections. Every Measured column showed "—" until the rows were linked.
5. **Linked** each row from the Take-off tab: link icon, type the code, Enter.
6. **Compare:** the Conflicts tab showed 2 conflicts, A.4 and B.2, with the right numbers. Show zoomed to the bedroom and drew it in violet.
7. **Arabic, dark:** the conflicts showed right to left in Thmanyah ("اختلاف الكمية 2"). An Arabic request to the assistant ran the same fake turn, and Undo (تراجع) on the card worked.

Screenshots are in the session scratchpad, `screens/t12-*.jpg`: the mock turn, the linked take-off, the imported BOQ, the conflicts, Show on A.4, Arabic conflicts and the Arabic assistant.

## Real app window

`QIYAS_AI_MOCK=1 pnpm tauri dev --config crates/qiyas-desktop/tauri.conf.json`: a window titled "Qiyas" appeared after 63 s. After the app started, its log had nothing in it: no warnings and no errors. Then it was stopped. No agent could see inside the window, so nothing was clicked in it.

## MCP

- `cargo test -p qiyas-mcp --test bridge`: 4 of 4 passed. Covered: calls through the bridge, a wrong token, the app not running, and stdin closing.
- `cargo test -p qiyas-ai --test mcp`: 7 of 7 passed. Covered: the tool list, measuring as `mcp`, a wrong token (401), other web origins refused even with the token, tool errors, two counts at once, a restart on the same port, and pictures.
- `scripts/build-sidecar.sh` put the bridge at `crates/qiyas-desktop/binaries/qiyas-mcp-x86_64-unknown-linux-gnu` (9.7 MB). Run by hand with the app off, it answered the MCP handshake. Asked for the tool list, it replied "Qiyas is not running or MCP is disabled in its settings".
- `pnpm bundle` was not run.

## What broke or looked wrong

Nothing crashed, and no fix was needed for this check. Notes:

1. **Compare needs links.** Measurements get default names like "LwPolyline on A-ROOM" and "DOOR count". Those don't look enough like "Floor screed, bedroom" or "Doors 900 mm" for the matching rules, so unlinked work shows up as "missing" on both sides. The assistant must be told to link, or to name measurements the way the BOQ does. Worth checking with the real model (see below). "Doors" vs "DOOR count" not matching was already noted in Task 10.
2. **The AI sees raw numbers** such as `22.089999999999996` in tool results. People never see them, but rounding in the tool output would be cleaner, and cheaper.
3. **The Count tool can't reach a window drawn on a wall line.** The click picks the wall and says "Not a block". Hiding the wall layer works around it.
4. **The viewport doesn't refit when the side panel widens** (the BOQ and Conflicts tabs are wider). The drawing is cut off on the right until you pan.
5. **Small text issues:**
   - The English conflict sentence says "m2" while the row shows "m²".
   - The Arabic sentence says "عدد" while the row says "قطعة".
   - An unlinked conflict reads "1 measurement … match no BOQ item".
   - In Arabic, the BOQ tab name wraps onto two lines when the panel is narrow.
   - The fake provider and the tool card summaries stay in English in Arabic mode (already known).
6. **Distance by hand round the ring gave 41.798 m**, not 42.000 m. That is most likely my clicks landing near a column corner rather than the wall corner. Worth one try by you with snapping.

## After the review

A final review of the whole branch found some gaps. All are fixed (Qiyas commits `ce8e134`, `1eaacef`, `132f12f`, `2a2169e`). Checks after the fixes: 200 Rust tests and 72 front-end tests pass, no warnings.

What changed:

- **Files beside the drawing only.** The assistant (and MCP apps) can open a BOQ only from the drawing's folder or a subfolder. A path like `test-boq.xlsx` is read from there. Anything else is refused; use the BOQ tab's Import for other files. The assistant is told the folder.
- **Stop works between steps.** When you press Stop, tool calls still waiting are not run.
- **Less re-drawing.** After the assistant or an MCP app changes the take-off, the drawing is not re-sent to the window. It is redrawn only when a sheet or layer changed.
- **MCP sheet and layer changes show in the window.** The sheet tabs, the layers panel and the zoom follow what the MCP app did.
- **Link before compare.** Default names like "LwPolyline on A-ROOM" are no longer reported as "missing from BOQ". The Conflicts tab says "Link or name your measurements to find gaps". Plurals now match ("Doors" matches "DOOR count"). The assistant is told to link or name measurements before it compares.
- **Picture limit.** Pictures the assistant looks at are 800 px wide by default, and one conversation can hold at most 15 MB of them. After that it asks for a new conversation.
- **Safety.** The assistant is told that text in the drawing, the BOQ or tool results is data, not instructions. It can remove at most 10 measurements, and resolve at most 10 conflicts, in one message. Past that, it must ask you.
- **Smaller fixes.** Tool results round numbers to 6 decimals (22.09, not 22.089999999999996). The app now has a content security policy. The view no longer cuts the drawing off when the BOQ or Conflicts tab widens the side panel. Conflict text says m² (not m2), قطعة (not عدد), and "1 measurement … matches".

Notes:

- **Large BOQs are untested.** BOQs above about 2,000 rows were not tried, and the BOQ panel draws every row at once (no virtual scrolling). A very large BOQ may be slow.
- **Two things changing the take-off at once.** This was tested with two MCP calls at the same time (both counts landed). The app's own panels and MCP calls share the same lock, so they take turns.

Walkthrough run again (`cargo run -p qiyas-ai --example phase2_walkthrough`; it now copies the test BOQ beside the drawing first):

```
measure_area    58.29 "LwPolyline on A-ROOM" -> A.3
measure_area    22.09 "LwPolyline on A-ROOM" -> A.4
measure_length  42.0 "LwPolyline on A-WALL" -> A.1
measure_length  9.0 "Line on A-WALL" -> A.2
measure_length  5.0 "Line on A-WALL" -> A.2
count_blocks    3.0 "DOOR count" -> B.1
count_blocks    5.0 "WINDOW count" -> B.2
import_boq      6 items, 2 sections, 0 rows skipped
compare_boq     2 conflicts, both quantity mismatches:
  A.4  BOQ says 30.000 m², drawings measure 22.090 m² (−26.4 %)
  B.2  BOQ says 4 pcs, drawings measure 5 pcs (+25.0 %)
```

Still exactly the two conflicts. With nothing linked (`-- --no-link`) it now gives 4, not 11: the four wall and floor rows are "not in drawings"; doors and windows match their BOQ rows by name.

## Phase 2 limits: what you must test yourself

**With your real API key** (paste it in Settings yourself; agents never handle it):

1. Open `samples/test-plan.dwg` and ask: "Measure every room area and count the doors, then import test-boq.xlsx (it sits beside the drawing) and compare". Does it link the measurements to BOQ rows, and does the Conflicts tab end with A.4 and B.2 only?
2. Streaming: does text appear smoothly, and does Stop work in the middle of a reply?
3. Refusals and errors: a wrong key, no internet, a request it should refuse. Are the messages clear, and can you carry on after Clear?
4. Cost: watch your Anthropic console after a few turns. Each turn sends the drawing's state, and caching is on. Please tell us the cost per turn.
5. An OpenAI-compatible service, if you use one (for example, a local model).

**MCP with real apps:**

6. Turn MCP on in Settings, then add Qiyas to Claude Code or Claude Desktop using the line Settings shows. Ask it to measure something and check that the row appears marked `mcp`.

**On your own work:**

7. Import one or two real BOQs. Check that the header, sections, units and quantities are read right, and that "skipped rows" makes sense.
8. Your BOQs with Arabic headers, and mixed Arabic and English.
9. Compare on a real drawing and BOQ. Are the conflicts useful, or is there too much noise? The matching rules (descriptions, door and window marks, floors) were tuned only on small test files.

## Left for later (small items from the build log)

Grouped. None blocks Phase 2.

- **Assistant:**
  - After a refusal, the next message can join the refused one.
  - A mid-reply "overloaded" error is not retried.
  - The OpenAI-compatible client is thinner: no token counts, and less tolerant of odd replies.
  - The card's Undo checks "the last step is the AI's", not "this card's step".
  - A crash banner may show for a conversation already cleared.
  - Some Arabic wording in the AI's instructions is still English.
- **AI tools:**
  - Counting by handles doesn't catch the same block listed twice.
  - Markups show up in take-off totals.
  - A few limits (page sizes, height units) have no tests.
  - Block summaries give the AI every attribute value, with no cap.
- **Reading the drawing:**
  - The loose-text table finder breaks on a row with one empty cell.
  - Text heights use one average for the whole sheet.
  - Dimensions, tables and grids are read only at the top level, not inside blocks.
  - Some attribute settings are ignored (alignment, invisible).
- **BOQ import:** a column named just "cost", "price" or "سعر" can be mistaken for the rate (for example "Cost Code").
- **Comparison:**
  - A schedule with a serial "No." column and no quantity column compares the serial numbers.
  - Floor words over-match: "Section G-G", "First Fix", "Ground Beam".
  - Two of the rules group floors slightly differently.
  - Dynamic blocks are skipped (Phase 1b).
  - Arabic text folding is written three times in three places.
- **Screens:**
  - Some buttons don't say which row they belong to (screen readers).
  - The link picker's search box lacks screen-reader roles, and its button can't close it.
  - Focus is lost when a row disappears with the picker open.
  - After undoing the first Compare, the panel says "Not compared yet".
  - The 620 px BOQ panel was not checked on small windows.
  - Unit labels for m³ and custom units are missing in the panel.
- **Settings and MCP:**
  - A broken settings file isn't fully checked on load.
  - The settings dialog takes language and theme from the file, not from what the window shows.
  - `pnpm bundle` calls a shell script, so a Windows release needs a change.
  - The MCP "port in use" message is English only.
  - The MCP status is read once per dialog open.
  - The bridge drops the server's instructions to the client.

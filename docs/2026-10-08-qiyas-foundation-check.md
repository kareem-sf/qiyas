# Qiyas foundation: final check on real drawings

Date: 2026-10-08. Qiyas commit `c7b7642` (branch main, not pushed), updated after the review fixes up to `dbae6be`.
Machine: Intel i5-1235U laptop, 8 GB RAM, Intel Iris Xe graphics, Linux (Hyprland).

## In short

- All local checks pass.
- Both real drawings open with no problems. Open plus first draw takes under 0.1 s on the small one and about 0.5 s on the big one (release build).
- Panning stays at 60 fps on both, but the big drawing only just makes it (worst frame 16.4 ms against a 16.7 ms limit).
- A wall length, a room area and a block count all came back exactly after saving as DWG and reopening, even with the project file removed.
- `pnpm tauri dev` used to take about 20 seconds to draw the big drawing. After the fix it takes about 1.7 s (release build: 0.4 s), and the window no longer freezes while it works.
- Saving over a drawing now keeps the old file as a `.bak` next to it, for example `villa.dwg.bak`.

## What was available

This machine has only two real drawings, both AutoCAD 2018 format (AC1032). There is no villa set and nothing over 20 MB, so the three-drawing plan became two:

- **Small building, many blocks:** two office floor plans with furniture (705 block references, 63 block types).
- **Largest:** a design package with about 40 sheets laid out side by side in model space.

Both were copied to a scratch folder before testing, and the copies were deleted afterwards. The originals were not touched.

## Check script

`scripts/check.sh` on a clean tree: exit 0. After the review fixes it runs 66 Rust tests and 24 front-end tests (8 files), with no warnings.

## Numbers per drawing

Open and first-draw times are from `cargo run --release -p qiyas-core --example bench_open`, three runs each. The `tauri dev` (debug) rows are from the same example without `--release`, after the dev-build fix (three runs each). Other work was running on the laptop at the same time. One run taken while the load was high was 3 to 4 times slower, and it is left out.

Pan times come from the Vite page in the built-in browser (Chromium, real Iris Xe GPU) at 1360 × 948 px, 1× pixel density. They cover 60 frames of continuous middle-drag panning and time each frame until the GPU has finished drawing it. Faked IPC served the real core.

**KMQ X GAC 17.09.2026.dwg** (1.61 MB, AC1032)

| What | Result |
|---|---|
| Open (release) | 27–31 ms |
| First batch (release) | 75–79 ms |
| Open / first batch in `tauri dev` (debug) | 29–33 ms / 311–343 ms (was 121 ms / 3,812 ms) |
| Line segments | 322,697 (6,003 entities drawn) |
| Batch sent to the screen | 9.1 MB |
| Problems | 0 |
| Layers, sheets | 52 layers, 3 sheets, units metres |
| Pan at full extents, average / worst frame | 6.0–6.9 ms / 7.6–14.3 ms: **60 fps, yes** |
| Open button to drawing on screen (in the page) | about 0.24 s |

**ALJ HO - Option B Design Package.dwg** (2.65 MB, AC1032)

| What | Result |
|---|---|
| Open (release) | 72–80 ms |
| First batch (release) | 418–438 ms |
| Open / first batch in `tauri dev` (debug) | 84–100 ms / 1,735–1,753 ms (was 314 ms / 19,837 ms) |
| Line segments | 1,389,985 (46,953 entities drawn) |
| Batch sent to the screen | **39.7 MB** |
| Problems | 0 |
| Layers, sheets | 142 layers, 3 sheets, units metres |
| Pan at full extents, average / worst frame | 11.8–14.9 ms / 13.0–16.4 ms: **60 fps, only just** |
| Pan zoomed into one sheet, average / worst frame | 6.7 ms / 14.9 ms |
| Open button to drawing on screen (in the page) | about 0.7 s |

Picking an entity takes about 0.002 ms on both drawings. Copying the whole drawing in memory, which every save now does so a failed save changes nothing, takes 3 ms and 8 ms.

## Measurement round trip (KMQ drawing)

These steps were done in the real app page, driven through the real core:

| Measurement | How | Value |
|---|---|---|
| Wall length | Select tool, clicked a wall line | 6.413 m (layer WALL) |
| Room area | Area tool, 4 corner clicks, Enter | 9.600 m² |
| Block count | Count tool, clicked one task chair | 220 pcs |

- **Save as DWG:** took 0.2 s. The file went from 1.61 MB to 1.27 MB.
- **Reopen with the project file:** the same three rows came back.
- **Reopen with the project file removed** (only what is stored inside the DWG): the same three values came back exactly (6.412828… m, 9.599703… m², 220), with the same descriptions. The layers QIYAS-LENGTH, QIYAS-AREA and QIYAS-COUNT are there. Problems: 0. The drawing has 5 more segments (the measured line and the area outline). Nothing else Qiyas draws changed.
- **Big drawing saved as DWG** with no measurements: its size stayed the same (+204 bytes), the segment count was identical, and 3 empty QIYAS layers were added.

## Real app window

`pnpm tauri dev` built in 49 s. A window titled "Qiyas" appeared after 55 s. The log was clean: only two notes from Tauri, no warnings or errors. Then it was stopped. No agent could see inside the window, so nothing was clicked in it.

After the review fixes, the first `pnpm tauri dev` build takes about 2 minutes (it now optimises the libraries once); later starts take under a second to build. The window titled "Qiyas" appeared and the log was clean again.

## What broke or looked wrong

1. **The dev build was very slow on big drawings (fixed).** In `pnpm tauri dev`, drawing the big file for the first time took about 20 s, and the small one about 4 s. Dev builds now optimise the libraries and the two Qiyas parts that draw: the big file takes about 1.7 s and the small one 0.3 s (release: 0.4 s and 0.08 s). Every command also runs off the window's main thread now, so the window stays responsive while a drawing opens or draws. The cost: the first dev build takes about 2 minutes instead of 49 s.
2. **The big drawing has no speed margin when panning.** Every frame redraws all 2.8 million line ends. On this laptop the worst frame was 16.4 ms against a 16.7 ms limit. A bigger drawing, a 4K or high-DPI screen, or the slower WebGL in the Linux app window will drop below 60 fps. Only drawing what is on screen, or simplifying when zoomed out, would fix it later.
3. **The big drawing sends a 40 MB batch.** That happens on open, every layer toggle, every sheet change and every save, and it took about 0.5 s locally. Expect around 1 s per layer click on this drawing. It was not measured inside the real window.
4. **Count names are raw block names.** For example "furniture plan-all al-omairi$0$Verus Task Chair - Gray-2145241-05 - Second". The `$0$` part comes from a bound xref. It should be cleaned up for a QS. (Counts used to show as "220.000 pcs"; they now show as "220 pcs" in the panel and in Excel.)
5. **Text is drawn as outline boxes** (Phase 1a design). Title blocks and notes look like solid black slabs when zoomed out.
6. **Saving the small drawing made it 22% smaller** (1.61 → 1.27 MB), the same as in Task 11. The big drawing kept its size. Qiyas sees nothing missing, but only AutoCAD can say whether anything visible was lost.
7. A save with no measurements still adds 3 empty QIYAS layers (minor).

## Known limits

- **Measuring inside blocks:** clicking an entity that lives inside a block (including bound xrefs, where the whole plan is one block) gives a count of the block, not the length or area of the clicked line. Use the Distance and Area tools inside blocks for now; Phase 1b adds picking into blocks.

## Not checked

- A villa set, a drawing over 20 MB, real DXF files, and Arabic text inside drawings. None were on this machine.
- Speed inside the real Tauri window (WebKitGTK), as opposed to Chromium.

## What you should check yourself

1. Open a drawing you re-saved from Qiyas in AutoCAD or DWG TrueView. Confirm the QIYAS layers show and nothing visible is lost, especially on the small drawing that shrank by 22%.
2. Open the big drawing and pan it, in `pnpm tauri dev` or a release build (`pnpm tauri build`). Does it feel smooth? How long does a layer click take?
3. Open a drawing that has Arabic text and see how it looks (today text is boxes only).
4. Try it on a high-DPI or external 4K screen.
5. Do a full drive-through in the real window: open, measure a wall, a room and a count, save as DWG, reopen, export to Excel, and switch to Arabic. No agent could see that window.
6. If you have a villa set or a drawing over 20 MB, send it over and we will run the same numbers.
7. Press Save (Ctrl+S) on a drawing. Check that a `.bak` file with the old drawing appears next to it (for example `villa.dwg.bak`) and that it still opens.
8. Open a drawing that shows problems in the status bar, then press Save. Qiyas should ask first, because saving rewrites the drawing with only what it could read. Say no and check nothing changed.
9. With an Arabic keyboard layout, try Ctrl+Z and the tool letters D, A, C, S.

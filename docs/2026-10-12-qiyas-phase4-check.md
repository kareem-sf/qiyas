# Qiyas Phase 4 (output, automation, parametric, shipping): final check

Date: 2026-10-12. Qiyas commit `b2cd83a` (branch main, not pushed), plus two commits from this check: `e1ff888` (a one-line build fix, below) and this document with its example. Same laptop as the earlier checks.

Everything below ran without a window. The plots, the batch and the constraint ran through the Rust crates and the command line tool, on copies of your two drawings (copies deleted after). No other CAD program is on this laptop, so no plot was compared with an AutoCAD print. The installers were built but not installed or started.

The laptop was busy during the check (other agents building and testing, load average 10 to 20 at times, swap in use). Times are upper bounds.

## In short

- **The check script passes**: 804 Rust tests, 557 front end tests, 0 failed. The PDF tests ran. It needed one small fix first (see What broke).
- **The Linux installers build**: deb 26.1 MB, rpm 26.1 MB, AppImage 118.8 MB. Each holds the app, the `qiyas-mcp` bridge and pdfium. Nobody has installed or opened them yet.
- **Plots of your drawings come out**: right page size, lines and real text. KMQ on A1 takes about 1.2 s and gives a 2.7 MB PDF; ALJ takes about 5 s and gives 8.7 MB.
- **Arabic in plotted PDFs looks right but copies wrong.** On the page, ALJ's title block reads correctly. Copied or searched in a PDF viewer, many words come out with the wrong letters: "جميل" comes out as "حميل", "الجيزة" as "الحيرة", "التصميم" as "اليصميم". Letters that share a shape and differ only by their dots get mixed up. Not fixed: it is product code.
- **The headless batch works**: 5 drawings (your two, the test plan twice, one damaged file) in 0.14 s. 4 workbooks were written; the damaged file was reported and skipped. Exit code 1, as designed.
- **The parametric rectangle works on your drawings**: a 4 × 3 rectangle drawn on each, auto-constrained (8 constraints), width set from 4 to 6. It stayed a rectangle, in 1 to 5 ms.
- **The Thmanyah font is inside the installers.** The window's Arabic font is built into the app. Shipping it to others needs Thmanyah's licence to allow it. Please confirm before anyone outside the company gets an installer.

## What Phase 4 added

- **Plot to PDF**: any sheet, extents, a window or the layout; a paper size; fit or a scale such as 1:100. Lines keep their weights, text is real, searchable PDF text, and fills are filled.
- **Plot styles** (CTB and STB): read, edit and save. Print to a printer, Quick print, Print all layouts.
- **Command scripts** (`.scr`): run from the Manage tab, or without a window through `qiyas-cli`.
- **`qiyas-cli`**: `convert`, `run`, `takeoff` and `batch` over a folder or a sheet set.
- **Audit and Purge**, an **Options** dialog (colours, pick box, snap size, shortcuts), **Customize** the ribbon, and an **Aliases** editor.
- **Parametric constraints**: geometric ones (coincident, parallel, horizontal...), auto constrain, dimensional ones with formulas (`d1 = 2*d2 + 1.5`), and a parameters manager.
- **Updates**: Check for updates in the About box, and a check once a day. It says "not set up" until you set the feed address and signing key.
- **One Qiyas at a time**: a second launch hands its files to the running one. Double-clicking `.dwg`, `.dxf` and `.qiyas` files opens them in Qiyas.
- **Installers** for Linux (deb, rpm, AppImage); the same command makes Windows and macOS ones on those computers.
- **Stronger crash recovery**: lock files, crash copies, and an autosave if Qiyas panics.

## Check script

`scripts/check.sh` in the repo (not a copy), with `CARGO_BUILD_JOBS=2`.

| Run | Result | Time |
|---|---|---|
| 1 (cold) | Stopped at its pdfium guard: the 5 PDF tests were ignored | 47 min (busy laptop) |
| 2 | Lost to a session restart while compiling | – |
| 3 | Same stop as run 1 | 6 min |
| 4, after `e1ff888` | **Exit 0** | 6.5 min (warm build) |

Run 4:

- Rust: **804 passed, 0 failed, 1 ignored** (`write_fixtures`, which only writes the plot style test files, ignored on purpose).
- The PDF tests ran: 5 in `qiyas-pdf`, 9 in `qiyas-core`.
- The three release speed guards passed: 1 million segments indexed under 0.6 s, a fill in a block placed 500 times, 200 constraints solved under 50 ms.
- Front end: **557 tests in 80 files**, all passed.
- fmt, clippy and tsc were clean.

The Phase 4 suites in that run:

| Suite | Tests |
|---|---|
| Plot (`qiyas-plot`: plot, ctb, print) | 18 + 11 + 6 |
| Scripts (`qiyas-core` script) and CLI | 16 + 16 |
| Audit, purge | 5 + 11 |
| Parametric (`qiyas-core` param) | 13 |
| Autosave and recovery | 6 + 14 |

## Numbers per drawing

Release builds, on copies. `bench_open` ran 3 times per drawing. "Phase 3" is the last check's "after the fix wave" column.

| What | Test plan | KMQ X GAC (1.61 MB) | ALJ HO Option B (2.65 MB) |
|---|---|---|---|
| Open | 2–3 ms | 30–33 ms | 78–88 ms |
| First draw (and every redraw) | 1 ms | **214–237 ms** (Phase 3: 259–389) | **894–951 ms** (Phase 3: 1,052–1,223) |
| of which: click index | 0 ms | 81–83 ms | 401–409 ms |
| Line segments | 96 | 347,843 | 1,485,161 |
| Batch sent to the screen | 5 KB | 11.1 MB | 59.3 MB |
| Problems reading it | 0 | 0 | 0 |
| Sheets | Model, Layout1 | Model, Layout1, Layout2 | Model, Layout1, Layout2 |
| Model extents | 12.4 × 12.5 m | 69.1 × 80.6 m | 1,329.9 × 291.2 m |

The redraw is a little faster than at the end of Phase 3; the laptop was quieter for this part (load average about 3.5).

**Plot to PDF** (`cargo run --release -p qiyas-plot --example phase4_check -- <drawing> <out>`). "Read" is gathering the drawing under the lock; "write" is drawing the PDF without it, which the window does off its main thread.

| Plot | Test plan | KMQ | ALJ |
|---|---|---|---|
| Model, A1 landscape, fit: read + write | 0 + 1 ms | 267 + 886 ms | 1,364 + 3,726 ms |
| PDF size | 7.8 KB | 2.65 MB | 8.69 MB |
| Text characters `pdftotext` finds | 58 | 2,473 | 52,779 |
| Scale fit gave (about) | 1:21 | 1:138 | 1:1,600 |
| Model, A1, 1:100, centred: read + write | 0 + 1 ms | 268 + 901 ms | 1,329 + 4,512 ms |
| PDF size | 6.2 KB | 2.77 MB | 14.1 MB |
| Layout1, A1, fit | refused: "Nothing to plot" | blank page, 569 bytes | blank page, 569 bytes |

- `pdfinfo` on every PDF: 1 page, A1 2383.94 × 1683.78 pt (A4 841.89 × 595.28 pt for the label plots), producer Qiyas.
- `pdffonts`: Noto Sans (and Noto Sans Arabic where there is Arabic) embedded as subsets, with a ToUnicode map.
- At 1:100 your drawings are bigger than A1 (KMQ 690 × 806 mm, ALJ 13.3 m wide). The middle is plotted and the rest is cut off at the page edge, as expected.
- The layouts in both your drawings are empty: everything is in model space. The test plan's empty layout says "Nothing to plot", yours give a blank page. Two answers to the same case; small.

**Arabic in the PDF.** An Arabic label, "غرفة المعيشة ٥٨٫٢٩ م²", was added in memory to each drawing and plotted on A4. On the test plan and KMQ, `pdftotext` gave it back exactly. ALJ's own Arabic (its title blocks) has the problem in In short: drawn right (checked in a 1440 dpi render of the title block), extracted wrong.

| In the drawing (`convert` to DXF) | Copied from the PDF |
|---|---|
| التصميم الهندسي | اليصميم المندبي |
| المقر الرئيسي لعبد اللطيف جميل | المقر الربيشي لعند اللطيف حميل |
| للمراجعة والتعليق | للمراحعة واليعليق |
| السادس من أكتوبر، الجيزة | الشادش من أكيوبر، الحيرة |

The likely cause: Noto Sans Arabic draws ت, ب, ن, ي (and ج/ح, س/ش...) in the middle of a word as one dotless shape plus separate dots. The PDF's text map gives each shape one letter, the first one it met. The fix belongs in `qiyas-plot`'s ToUnicode map (a shape used for different letters must be written with its text, for example as an ActualText span, or drawn as outlines). Searching the PDF, or copying from it into a BOQ, gives wrong words until then.

**Purge (dry run) and audit** (`fix: false`), on the whole drawing:

| | Test plan | KMQ | ALJ |
|---|---|---|---|
| Purge dry run | 0 ms, nothing | 24 ms: 3 (1 layer, 2 blocks) | 57 ms: 99 (10 layers, 44 blocks, 45 linetypes) |
| Audit | 0 ms, nothing | 9 ms: 1 empty block, 1 unused layer | 29 ms: 1 empty block, 10 unused layers |

No missing layers, styles or blocks in either of your drawings. Nothing was purged or fixed.

**A constraint on a drawn rectangle.** A 4 × 3 rectangle drawn beside each drawing's extents, then Auto constrain, a linear width constraint (d1 = 4), then d1 = 6:

| | Test plan | KMQ | ALJ |
|---|---|---|---|
| Auto constrain | 8 constraints, 0 ms | 8, 1 ms | 8, 3 ms |
| Width constraint | 0 ms | 1 ms | 5 ms |
| d1 = 6 | 0 ms | 1 ms | 4 ms |
| Corners after | (0,0) (6,0) (6,3) (0,3) | same | same |

## Real-file checks (no window)

**The headless batch.** A folder of 5 copies: ALJ, KMQ, the test plan twice (one renamed), and KMQ cut off after 800 KB (damaged). The script:

```
COUNT DOOR
COUNT WINDOW
COUNT COLUMN
EXPORT {name}.xlsx
```

`qiyas-cli batch <folder> --script count.scr --out <out>`:

| Drawing | Result | DOOR | WINDOW | COLUMN |
|---|---|---|---|---|
| ALJ HO - Option B Design Package.dwg | ok | 7 | 0 | 0 |
| corrupt-cut.dwg | error: "This drawing can't be opened: no usable drawing data found" | – | – | – |
| KMQ X GAC 17.09.2026.dwg | ok | 7 | 0 | 0 |
| test-plan-copy.dwg | ok | 3 | 5 | 6 |
| test-plan.dwg | ok | 3 | 5 | 6 |

- Summary line: "5 drawings: 4 ok, 0 with warnings, 1 failed". Exit code 1 with "1 of 5 drawings failed". The whole run took **0.14 s**; with `--jobs 2 --json`, 0.15 s and the same answers.
- 4 workbooks, one per good drawing (6.5 KB each). Each has a row per count with its quantity and "pcs". The test plan's 3 / 5 / 6 match `samples/test-plan.md`. The counts on your drawings were not checked against AutoCAD.
- The workbook's Sheet column says `*Model_Space`, not "Model". Small.

**Convert:**

| From | To | Time | Reopened |
|---|---|---|---|
| ALJ (DWG 2018) | DXF, 17.3 MB | 233 ms | – |
| ALJ | DWG 2004, 2.59 MB | 197 ms | 1,485,161 segments, 0 problems |
| KMQ | DXF, 7.4 MB | 81 ms | 347,843 segments, 0 problems |
| KMQ DXF | DWG 2013, 1.14 MB | 118 ms | 347,843 segments, 0 problems |
| Test plan | DXF | 5 ms | – |
| The damaged KMQ | DXF | refused, exit 1 | – |

**Autosave and recovery**: the Rust tests only, as asked (6 autosave and 14 recovery tests, all passed). One of them starts a child process, kills it mid-edit and checks its autosave is offered with its take-off. The window's recovery dialog was not seen.

## Installers

`CARGO_BUILD_JOBS=2 pnpm bundle`: exit 0 in 15 min (most of it the release build of the app).

| File | Size |
|---|---|
| `target/release/bundle/deb/Qiyas_0.1.0_amd64.deb` | 26.08 MiB |
| `target/release/bundle/rpm/Qiyas-0.1.0-1.x86_64.rpm` | 26.07 MiB |
| `target/release/bundle/appimage/Qiyas_0.1.0_amd64.AppImage` | 118.78 MiB |

What is inside (deb unpacked with `bsdtar`, rpm listed, AppImage with `--appimage-extract`, all in a temp folder):

- `usr/bin/qiyas-desktop` (66 MB) and `usr/bin/qiyas-mcp` (9.7 MB), in all three.
- `usr/lib/Qiyas/resources/pdfium/libpdfium.so` with `VERSION` and its 17 licence files, in all three.
- Desktop entry `Qiyas.desktop`: `Exec=qiyas-desktop %F`, `Name[ar]=قياس`, `MimeType=image/vnd.dwg;image/vnd.dxf;application/x-qiyas`.
- `usr/share/mime/packages/com.qsmind.qiyas.xml` (the `.qiyas` type, with an Arabic comment) in the deb and the rpm. The AppImage has none, as documented.
- Icons at 32, 128 and 256 px.
- The AppImage also carries WebKitGTK and its libraries (366 MB unpacked), which is why it is 4.5 times the deb.

Checked without installing:

- The bundled `qiyas-mcp` answers an MCP `initialize`, and with no Qiyas running says "Qiyas is not running or MCP is disabled in its settings".
- The app holds the `REPLACE-ME` feed address and key, so Check for updates will say updates are not set up.
- `ldd` finds every library the app needs on this computer.

Small things in the packages:

- The deb's description is "(none)", and its maintainer is "qsmind".
- The desktop entry's `Categories=` is empty, so some menus put Qiyas under "Other".
- `MimeType=` has no `;` at the end, which `desktop-file-validate` warns about (not installed here to confirm).
- The licence texts in `LICENSES/` (opencadcodec and opencadkernel MPL-2.0, nalgebra and others) are not in the packages; only pdfium's are. MPL-2.0 asks that people who get the program are told where its source is. Worth adding before installers go out.
- Vite warns that the app's script is 702 KB (over its 500 KB hint). Harmless for a desktop app.

## What broke or looked wrong

1. **Fixed (`e1ff888`): the check script failed, because the PDF tests were skipped.**
   - `crates/qiyas-pdf/build.rs` looked for pdfium through `env!("CARGO_MANIFEST_DIR")`, which is fixed when the build script is compiled. A copy of the repo built earlier in the same `target/` folder (`scratchpad/t3idx`) left a compiled build script looking in that copy, where pdfium is not. So the tests were compiled as "no pdfium" and ignored, and the guard stopped the check.
   - It now reads the folder when it runs. One line; the check then passed.
2. **Not fixed: Arabic copied from a plotted PDF has wrong letters** (see Numbers per drawing). The page looks right. Search, copy and accessibility read the wrong words. This touches every Arabic drawing that goes to a client as a PDF. It is the most important thing to fix before release.
3. **To decide: the Thmanyah font ships inside the app.** Vite puts `thmanyahsans-Regular/Medium/Bold.otf` (about 240 KB each) into the app, so every installer carries them. The repo rules keep the files out of git, but an installer handed to anyone is distribution. If Thmanyah's licence does not allow embedding in an app, the window needs another Arabic font in the installers (Noto Sans Arabic is already there for drawings).
4. **The packages lack our `LICENSES/` texts** (above). Small to fix: add the folder as a bundle resource.
5. **An empty layout gives a blank page on your drawings, "Nothing to plot" on the test plan.** Small.
6. **Plots of big drawings are slow-ish and large**: ALJ fitted on A1 takes about 5 s and 8.7 MB. The window does this off its main thread, so it does not freeze. Whether 5 s is fine for you is your call.

No crash, no wrong count on the test plan, no wrong constraint.

## Phase 4 limits (left for later)

From the build log's "minor (deferred)" lines and the rulings. None blocks Phase 4.

- **Plot:**
  - Not compared with an AutoCAD print on this laptop (no other CAD).
  - Linetypes plot continuous, as the screen draws them.
  - Page setups are kept in the window's own storage, not in the drawing or the project file. Two layouts with the same name share one.
  - A picture whose definition is missing is left out without a word. Big pictures use a lot of memory. Margins for a rotated plot are not read.
  - Arabic text extraction (above).
- **Scripts and CLI:**
  - Opening a drawing whose `.qiyas` project file is broken writes a `.qiyas.bak`, even in a batch, and can write over a good one.
  - Two drawings of the same name (`plan.dwg`, `plan.dxf`) share one project file.
  - In the window, a Stop clicked at the very end of a step can be lost; shortcuts and the tab bar still work while a script runs.
  - A COUNT of a block the drawing does not have gives 0, not a warning, so a typo reads as "none".
- **Audit, purge, options:**
  - From MCP or a script, Audit with fix still runs during an edit in place (the window refuses).
  - The crosshair colour is saved but not drawn (no crosshair yet).
  - F5 bound to a command can still reload the window during a drawing switch.
- **Parametric:**
  - Auto constrain can miss a parallel or perpendicular pair near the angle tolerance.
  - Mirroring an arc drops its constraints without saying so.
  - Stretching part of a constrained entity is refused as a conflict.
  - Constraints show in one bar in a corner, not beside each entity. Not AI tools yet.
- **Updates and installers:**
  - Updates are off until you create the signing key and set the feed address.
  - The installers are unsigned. Windows and macOS will warn on first open; buying certificates is your decision.
  - Two drawings double-clicked during the launch question: the second never opens.
  - Windows and macOS installers were not built (no such computers here).
- **Recovery:**
  - Whether a lock's process is alive is judged on this computer only (a shared folder from another computer counts as "elsewhere").
  - Two crash copies in the same second, or a crash copy renamed while an autosave writes, can collide.

## What you should check yourself

No agent saw the window. To get the files:

- The installers are in `target/release/bundle/` (paths above).
- `cargo run --release -p qiyas-plot --example phase4_check -- "<drawing>" <out folder>` plots a drawing to the PDFs used above, reads the drawing only and writes only into the out folder.

1. **Install and start.** `sudo apt install ./Qiyas_0.1.0_amd64.deb` on a Debian or Ubuntu computer, or on this laptop just run the AppImage (`chmod +x`, then start it). Does Qiyas start, with its icon and its Arabic name in the menu? Does double-clicking a `.dwg` offer Qiyas under "Open with" (deb only)?
2. **Open your drawings.** ALJ and KMQ: do they look as before? A redraw should take about 0.2 s on KMQ and about 1 s on ALJ.
3. **Plot a layout or the model to PDF** (Output tab, Plot…). Open it in another viewer (Firefox, Okular, Acrobat) and put it next to an AutoCAD print of the same sheet: line weights, text, hatches, colours, what is missing.
4. **Copy Arabic from that PDF** into a text editor. Expect wrong letters on ALJ (problem 2).
5. **Run a script from the Manage tab** (Run script…), e.g. the four lines above. Does the dialog list each line with a mark and the time?
6. **Kill the app mid-edit**: change a drawing, wait for the autosave (set 1 minute in Settings), then `pkill qiyas`. Start again: does the recovery dialog say Qiyas closed unexpectedly, and does Recover bring your edits and take-off back?
7. **About box, Check for updates**: it should say updates are not set up in this build yet.
8. **Copy and paste on Wayland** between two drawings, and from another app into Qiyas.
9. **Arabic text and dimensions**: type Arabic in a text and a dimension, in English and Arabic mode. Joined, right to left, and right after save and reopen?
10. **Parametric tab on a simple rectangle**: Auto constrain, a Linear constraint on one side, then change it in the Parameters manager. Does it stay a rectangle, and does the bar show the constraints?
11. **Options dialog colours**: change the background and highlight colours. Do both viewports follow?
12. **The Thmanyah licence** (problem 3): does it allow embedding the font in an app given to others?

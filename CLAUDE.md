# Qiyas

Qiyas (قياس) is a desktop measurement CAD for quantity surveyors, part of QS Mind. It opens DWG and DXF drawings, lets the QS measure and edit, and uses AI to build and check BOQs against the drawing. Private repo (`kareem-sf/qiyas`). Stack: Tauri 2, React, TypeScript, Vite, Rust.

## Layout

```
qiyas/
  crates/
    qiyas-core      the document: opencadcodec CadDocument in memory, layers, blocks,
                    layouts, xrefs, selection, snaps, units, undo and redo, command bus
    qiyas-geom      2D geometry on top of opencadkernel: offset, trim, fillet, area,
                    length, closed regions, intersections
    qiyas-render    turns the document into GPU buffers: lines, fills, text glyphs,
                    hatches, images; spatial index; dirty tracking per handle
    qiyas-takeoff   measurements, schedules, BOQ model, links, the comparison engine
    qiyas-boq       Excel in and out (calamine to read, rust_xlsxwriter to write)
    qiyas-pdf       PDF import as vectors and text, PDF underlay pages, PDF plot
    qiyas-ai        tool catalogue over the command bus, MCP server, Claude client
    qiyas-mcp       the stdio bridge Claude Desktop and Claude Code start (bundled beside the app)
    qiyas-desktop   the Tauri app: IPC, file dialogs, printing, keychain, updates
  src/              React + TypeScript + Vite: viewport, ribbon, command line, panels
  scripts/check.sh  cargo fmt, clippy, test, tsc, vitest, in one run
```

The crates that exist now: `qiyas-core`, `qiyas-geom`, `qiyas-render`, `qiyas-takeoff`, `qiyas-boq`, `qiyas-ai`, `qiyas-mcp` and `qiyas-desktop`. `qiyas-pdf` arrives in a later phase. The take-off, BOQ, schedule and conflict types live in `qiyas-core`; `qiyas-takeoff` holds the comparison engine.

## Global constraints

- Desktop only. No web build, no CI workflows. Local check is `scripts/check.sh`.
- Window title and product name: `Qiyas`. Arabic name: `قياس`. Bundle identifier: `com.qsmind.qiyas`.
- This repo is its own git repo. Commit inside it. Never push or release without the founder's yes.
- Only MPL-2.0, Apache-2.0, MIT, BSD or OFL dependencies. No code copied from Open CAD Studio (GPL).
- Thmanyah font files are never committed. `scripts/fonts.sh` copies them from `~/Downloads/Thmanyah-Font-Family/thmanyah typeface` into the git-ignored `src/fonts/thmanyah/`. Instrument Sans (OFL) is committed.
- English text says "Qiyas" and "QS Mind"; Arabic text says "قياس" and "كيو إس مايند". Arabic uses Thmanyah Sans; headings use `font-feature-settings: "swsh"`.
- Measured values are stored in drawing units and displayed in metres, square metres and pieces. Unit factor comes from the drawing header (`insertion_units`): 1 = inches, 2 = feet, 4 = mm, 5 = cm, 6 = m, 0 or unknown = treat as mm and show a warning in the status bar.
- Every mutating command is undoable. Undo and redo stacks live in `qiyas-core`.
- Brand colours: Ink `#0E0F12`, Stone `#F2EEE7`, Paper `#F9F8F4`, Slate `#6E6A64`, Qiyas face anodised titanium `#C9A6E0` to `#6E4A9E`.

## Working here

- Use pnpm, not npm.
- Run `scripts/check.sh` before every commit.
- Never push or release without the founder's yes.
- Run the app: `pnpm tauri dev --config crates/qiyas-desktop/tauri.conf.json`.
- The `.qiyas` project file is saved next to the drawing after every command.
- Save keeps a `.bak` of the original drawing (e.g. `villa.dwg.bak`) before writing over it.

## MCP

Outside AI apps can drive Qiyas through its MCP server (the same tools as the in-app assistant;
their measurements are marked `mcp`).

- Turn it on in Settings, MCP server. It listens on `127.0.0.1:48123/mcp` (port in Settings) and
  needs `Authorization: Bearer <token>`; the token is in Settings and in `settings.json` (mode
  0600). The dialog shows whether it is listening.
- Settings gives both setups, with the bridge's path filled in:
  - Claude Code: `claude mcp add qiyas -e QIYAS_MCP_TOKEN=<token> -- <app folder>/qiyas-mcp`
  - Claude Desktop (`claude_desktop_config.json`):
    `{"mcpServers": {"qiyas": {"command": "<app folder>/qiyas-mcp", "env": {"QIYAS_MCP_TOKEN": "<token>"}}}}`
- `qiyas-mcp` speaks MCP on stdio and forwards to the app over HTTP. `QIYAS_MCP_URL` overrides
  the address (default `http://127.0.0.1:48123/mcp`). When the app is not running it answers
  "Qiyas is not running or MCP is disabled in its settings".
- Dev: `cargo build -p qiyas-mcp` puts the bridge in `target/debug`, the folder Settings shows
  under `pnpm tauri dev`.
- Bundling: `scripts/build-sidecar.sh` builds it in release into
  `crates/qiyas-desktop/binaries/qiyas-mcp-<target triple>` (git-ignored), then
  `pnpm tauri build --config crates/qiyas-desktop/tauri.bundle.conf.json` ships it beside the
  app. The external binary lives in that extra config, not `tauri.conf.json`, so plain builds
  and tests don't need the sidecar built first.

## Where the plans live

In the workspace repo, `docs/`:

- Spec: `docs/superpowers/specs/2026-10-08-qiyas-design.md`
- Plan (Phase 1a): `docs/superpowers/plans/2026-10-08-qiyas-foundation.md`

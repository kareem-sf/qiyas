# Qiyas (قياس)

Desktop measurement CAD for quantity surveyors, by [QS Mind](https://qsmind.app). Open DWG and
DXF drawings, measure and edit them, plot to PDF or a printer, run scripts headless, and let the
AI assistant (Mizan) build and check bills of quantities against the drawing. English and Arabic.

Tauri 2, React, TypeScript and Rust. Apache-2.0 (`LICENSE`); the libraries we ship and their
licences are listed in `LICENSES/THIRD-PARTY.md`. The DWG/DXF reader and the geometry kernel
([opencadcodec](https://github.com/opencadstudio/opencadcodec),
[opencadkernel](https://github.com/opencadstudio/opencadkernel)) are MPL-2.0.

## Install

Download the newest release from [Releases](https://github.com/kareem-sf/qiyas/releases):
AppImage, deb or rpm on Linux. Installed copies check that page for updates (About, Check for
updates). Windows and macOS installers are built on those systems with `pnpm bundle`.

## Build

```bash
pnpm install
scripts/pdfium.sh                                             # PDF support (once per machine)
scripts/check.sh                                              # fmt, clippy, tests, tsc, vitest
pnpm tauri dev --config crates/qiyas-desktop/tauri.conf.json  # run the app
pnpm bundle                                                   # installers in target/release/bundle
```

Needs Rust stable, Node 20+ and pnpm. The window's Arabic font is Thmanyah Sans, a licensed
font that is not in the repo: `scripts/fonts.sh` copies it from the company's copy, and
without it the system's Arabic font is used. Drawing text uses Noto Sans and Noto Sans Arabic
(OFL, included).

`CLAUDE.md` describes every part of the app and how we work here; `docs/` holds the specs,
plans, check reports and the decisions carried forward.

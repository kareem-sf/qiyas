# Changelog

All notable changes to Qiyas are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). A release's notes on GitHub are its
section below, so add the section before pushing the tag.

## [Unreleased]

## [0.0.1] - 2026-10-12

First public release.

### Added

- Opens DWG and DXF drawings (AutoCAD 2000 to 2018) with xrefs, images and PDF underlays, and
  saves them back with a `.bak`.
- Take-off: lengths, areas and counts; a BOQ imported from Excel, compared with the take-off, and
  exported to Excel.
- Drawing and editing: lines, polylines, arcs, hatches, text, dimensions, tables, blocks, layers
  and groups, with undo for everything; parametric constraints and a parameters manager.
- Plot a sheet to PDF or a printer with CTB/STB plot styles, or every layout at once.
- Command scripts in the window or headless with `qiyas-cli` (convert, run, take-off, batch).
- Audit, purge, options, a customisable ribbon and command aliases.
- Autosave, lock files and crash recovery; automatic updates from GitHub releases.
- Mizan, the built-in AI assistant (Claude or any OpenAI-compatible service), and an MCP server
  for Claude Desktop and Claude Code.
- English and Arabic.
- The layers and the take-off panels fold away behind a thin rail on their inner edge, to give the
  drawing the whole width; the choice is remembered.
- Small dot-grid loading indicators (our own design) replace the spinner where the window waits:
  opening, comparing with the BOQ, drawing a PDF page, plotting, scripts and updates. Mizan works
  under the Qiyas mark itself, turning slowly with its face breathing.
- The ribbon is an icon ribbon, measure first: Measure, Draw and Annotate on the tab row, the rest
  under More. Every button is an icon with a tooltip (its name, what it does, its command line
  alias); a group shows its main buttons and puts the others behind a "⋯" button as the window
  narrows, so the panel does not scroll. The header's buttons share one size scale.
- Installers for Linux (AppImage, deb, rpm), Windows (setup.exe, MSI) and macOS (Apple silicon
  and Intel), each with a signature for the updater.
- A quieter, thinner interface on headless libraries (Base UI, TanStack Table and Virtual, Headless
  Tree, cmdk, Sonner; all MIT, styled in our own CSS):
  - The take-off, BOQ and conflicts tables are one data grid: columns sort and resize (and keep
    their widths), the header stays on top, long lists scroll without lag, numbers line up at the
    end in tabular figures, the description takes the room that is left and shows its full text in
    a tooltip, and rows are reached with the arrow keys. A conflict row shows itself on the drawing
    when clicked.
  - The layers panel is a tree with keyboard movement; a long list is grouped by A-, S-, E- prefix
    and by xref. Isolate, make current and properties show on hover or focus, so long layer names
    are no longer cut.
  - The command line lists matching commands above it with each shortcut as a small key.
  - The ribbon's and the side panel's tabs have a sliding indicator; the side panel's tabs never
    wrap (the last fold into a menu when the panel is narrow).
  - Messages such as "Saved to …", what a modify left out and errors are quiet toasts at the bottom
    end of the window (the bottom left in Arabic); the status bar keeps to status. After a purge,
    closing its dialog offers Undo for a few seconds.
  - Right-click the drawing for Move, Copy, Erase, Properties and Zoom extents.
  - OSNAP, ORTHO, POLAR and GRID are one segmented control.
  - Mizan's conversation keeps to the newest message and offers "Jump to latest" when you have
    scrolled up. A PDF page being drawn and an update downloading show a labelled progress bar.

Rebuilt on 2026-10-13 under the same version: the Windows and macOS installers were added, the
language and theme buttons no longer cover the ribbon's last tabs in a narrow window, the
installers' metadata says Apache-2.0 (it said "UNLICENSED"), and the header shows the Qiyas mark
with the name Qiyas only.

[Unreleased]: https://github.com/qs-mind/qiyas/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/qs-mind/qiyas/releases/tag/v0.0.1

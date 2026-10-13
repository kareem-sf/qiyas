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
- Installers for Linux (AppImage, deb, rpm), Windows (setup.exe, MSI) and macOS (Apple silicon
  and Intel), each with a signature for the updater.

Rebuilt on 2026-10-13 under the same version: the Windows and macOS installers were added, the
language and theme buttons no longer cover the ribbon's last tabs in a narrow window, and the
installers' metadata says Apache-2.0 (it said "UNLICENSED").

[Unreleased]: https://github.com/qs-mind/qiyas/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/qs-mind/qiyas/releases/tag/v0.0.1

# Changelog

All notable changes to Qiyas are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). A release's notes on GitHub are its
section below, so add the section before pushing the tag.

## [Unreleased]

### Added

- Continuous integration on GitHub: `scripts/check.sh` on Linux and a release build on Linux,
  Windows and macOS for every pull request and push to `main`.
- Releases from a pushed tag: installers for Linux (AppImage, deb, rpm), Windows (NSIS, MSI) and
  macOS (Apple silicon and Intel), signed for the in-app updater, in a draft release with
  `latest.json`.
- Contributing guide, code of conduct, security policy, issue and pull request templates.

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
- Linux installers: AppImage, deb and rpm, each with a signature for the updater.

[Unreleased]: https://github.com/kareem-sf/qiyas/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/kareem-sf/qiyas/releases/tag/v0.0.1

# Contributing to Qiyas

Thanks for helping. Bug reports with a drawing attached, fixes and features are all welcome. For
anything bigger than a fix, open an issue or a discussion first so we agree on the approach.

## Build and run

You need Rust stable (`rust-toolchain.toml` picks it), Node 20 or newer and pnpm. On Linux, the
Tauri system packages too (`libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`,
`librsvg2-dev`, `libxdo-dev`, `libssl-dev`, `patchelf`; see `.github/workflows/ci.yml`).

```bash
pnpm install
scripts/pdfium.sh                                             # PDF support (once per machine)
pnpm tauri dev --config crates/qiyas-desktop/tauri.conf.json  # run the app
```

`CLAUDE.md` describes every part of the app; read the section you are changing first.

## Check before you commit

```bash
scripts/check.sh
```

It runs `cargo fmt --check`, clippy with warnings as errors, the Rust tests (the PDF tests too,
once `scripts/pdfium.sh` has run), three release-mode speed guards, `tsc` and vitest, and stops
at the first failure. CI runs the same script on every pull request, plus a release build on
Linux, Windows and macOS; the `check` job must pass before a pull request is merged.

New behaviour comes with a test. Keep changes to what the pull request is about.

## Commits

Conventional commit subjects in the imperative, with the area in brackets, as in `git log`:

```
feat(ribbon): AutoCAD-style groups and a quick-access bar
fix(plot): Windows print runs PowerShell hidden, without a console
docs: Phase 4 rulings and carried items
build(updater): release-feed.sh writes latest.json from the signed bundles
```

Sign off every commit (`git commit -s`): the `Signed-off-by:` line certifies the
[Developer Certificate of Origin](https://developercertificate.org/), that you wrote the change
or have the right to submit it under the project's licence.

## Licence

Qiyas is Apache-2.0 (`LICENSE`); by contributing you agree your contribution is licensed the
same way (Apache-2.0, section 5).

Dependencies must be MPL-2.0, Apache-2.0, MIT, BSD, ISC, OFL, Zlib or BSL-1.0, nothing else.
Never copy code from GPL projects (Open CAD Studio among them). A new library whose licence is
not MIT needs its licence text in `LICENSES/`; after any dependency change run
`scripts/third-party.sh` and commit `LICENSES/THIRD-PARTY.md`.

## Fonts

The window's Arabic font, Thmanyah Sans, is licensed and never committed: `scripts/fonts.sh`
copies it into the git-ignored `src/fonts/thmanyah/` for those who have it, and without it the
system's Arabic font is used. Instrument Sans, Noto Sans and Noto Sans Arabic are OFL and in the
repo with their licences.

## Releases

Maintainers release by pushing a tag `v<version>`; see "Updates and installers" in `CLAUDE.md`.

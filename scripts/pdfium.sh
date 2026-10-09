#!/usr/bin/env bash
# Downloads the pdfium shared library (BSD-3-Clause, prebuilt by bblanchon/pdfium-binaries) for
# this computer into crates/qiyas-desktop/resources/pdfium/ (git-ignored), its licence into
# LICENSES/pdfium-BSD-3.txt and the licences of the libraries built into it beside the library.
# Qiyas reads PDFs through it; without it the PDF commands say "PDF support is not installed".
# Run it once per machine, and before bundling a release.
#   scripts/pdfium.sh                 the latest release
#   PDFIUM_TAG=chromium/8086 scripts/pdfium.sh
set -euo pipefail
cd "$(dirname "$0")/.."

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) asset=pdfium-linux-x64.tgz lib=lib/libpdfium.so ;;
  Darwin-arm64) asset=pdfium-mac-arm64.tgz lib=lib/libpdfium.dylib ;;
  Darwin-x86_64) asset=pdfium-mac-x64.tgz lib=lib/libpdfium.dylib ;;
  MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64) asset=pdfium-win-x64.tgz lib=bin/pdfium.dll ;;
  *)
    echo "No pdfium build for $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

tag="${PDFIUM_TAG:-latest}"
if [[ "$tag" == latest ]]; then
  url="https://github.com/bblanchon/pdfium-binaries/releases/latest/download/$asset"
else
  url="https://github.com/bblanchon/pdfium-binaries/releases/download/$tag/$asset"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
echo "Downloading $url"
curl -fsSL "$url" -o "$work/$asset"
tar -xzf "$work/$asset" -C "$work"

dest=crates/qiyas-desktop/resources/pdfium
mkdir -p "$dest" LICENSES
cp "$work/$lib" "$dest/"
cp "$work/licenses/pdfium.txt" LICENSES/pdfium-BSD-3.txt
# The libraries built into pdfium (FreeType, ICU, libpng, zlib, ...) ship with it.
rm -rf "$dest/licenses"
cp -R "$work/licenses" "$dest/licenses"
if [[ -f "$work/VERSION" ]]; then cp "$work/VERSION" "$dest/VERSION"; fi
echo "$dest/$(basename "$lib")"

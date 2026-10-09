#!/usr/bin/env bash
# Downloads the pdfium shared library (BSD-3-Clause, prebuilt by bblanchon/pdfium-binaries) for
# this computer into crates/qiyas-desktop/resources/pdfium/ (git-ignored), its licence into
# LICENSES/pdfium-BSD-3.txt and the licences of the libraries built into it beside the library.
# Qiyas reads PDFs through it; without it the PDF commands say "PDF support is not installed".
# Run it once per machine, and before bundling a release.
#
# The release is pinned to the one Qiyas was tested with, and the download must match its
# SHA-256 in scripts/pdfium.sha256, or nothing is installed. To move to another release, add its
# four assets' sums to that file and change TESTED_TAG (then run scripts/check.sh).
#   scripts/pdfium.sh
#   PDFIUM_TAG=chromium/NNNN scripts/pdfium.sh   (needs that tag's sums in pdfium.sha256)
set -euo pipefail
cd "$(dirname "$0")/.."

TESTED_TAG=chromium/8086

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

tag="${PDFIUM_TAG:-$TESTED_TAG}"
expected="$(awk -v t="$tag" -v a="$asset" '$1 == t && $2 == a { print $3 }' scripts/pdfium.sha256)"
if [[ -z "$expected" ]]; then
  echo "No SHA-256 for $asset of $tag in scripts/pdfium.sha256: refusing to install it." >&2
  exit 1
fi
url="https://github.com/bblanchon/pdfium-binaries/releases/download/$tag/$asset"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
echo "Downloading $url"
curl -fsSL "$url" -o "$work/$asset"
if command -v sha256sum > /dev/null; then
  actual="$(sha256sum "$work/$asset" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$work/$asset" | cut -d' ' -f1)"
fi
if [[ "$actual" != "$expected" ]]; then
  echo "SHA-256 MISMATCH for $asset of $tag:" >&2
  echo "  expected $expected" >&2
  echo "  got      $actual" >&2
  echo "Nothing was installed." >&2
  exit 1
fi
tar -xzf "$work/$asset" -C "$work"

dest=crates/qiyas-desktop/resources/pdfium
mkdir -p "$dest" LICENSES
cp "$work/$lib" "$dest/"
cp "$work/licenses/pdfium.txt" LICENSES/pdfium-BSD-3.txt
# The libraries built into pdfium (FreeType, ICU, libpng, zlib, ...) ship with it.
rm -rf "$dest/licenses"
cp -R "$work/licenses" "$dest/licenses"
if [[ -f "$work/VERSION" ]]; then cp "$work/VERSION" "$dest/VERSION"; fi
echo "$dest/$(basename "$lib") ($tag, SHA-256 checked)"

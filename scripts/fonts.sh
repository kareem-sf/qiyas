#!/usr/bin/env bash
# Copies the licensed Thmanyah fonts into the git-ignored src/fonts/thmanyah/.
# Never commit these files.
set -euo pipefail
cd "$(dirname "$0")/.."

src="$HOME/Downloads/Thmanyah-Font-Family/thmanyah typeface"
dest="src/fonts/thmanyah"
mkdir -p "$dest"
find "$src" -type f \( -iname '*.ttf' -o -iname '*.otf' \) -exec cp {} "$dest"/ \;
echo "Copied $(find "$dest" -type f \( -iname '*.ttf' -o -iname '*.otf' \) | wc -l) font files to $dest"

#!/usr/bin/env bash
# Writes target/release/bundle/latest.json for the updater feed from the signed bundles
# (`pnpm bundle:signed` leaves a `.sig` beside each one). Usage:
#   scripts/release-feed.sh [notes]
# Releases live on GitHub: the bundles, their .sig files and latest.json are the assets of the
# release tagged v<version> (https://github.com/kareem-sf/qiyas/releases/download/v<version>/<file>);
# the app reads the feed at .../releases/latest/download/latest.json (tauri.conf.json,
# plugins.updater.endpoints), so the newest release must always carry a latest.json.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(python3 -c 'import json;print(json.load(open("crates/qiyas-desktop/tauri.conf.json"))["version"])')
notes=${1:-"Qiyas $version"}
bundle=target/release/bundle
base="https://github.com/kareem-sf/qiyas/releases/download/v$version"
python3 - "$version" "$notes" "$bundle" "$base" <<'PY'
import glob, json, os, sys, datetime
version, notes, bundle, base = sys.argv[1:]
keys = {
    "appimage/*.AppImage": "linux-x86_64-appimage",
    "deb/*.deb": "linux-x86_64-deb",
    "rpm/*.rpm": "linux-x86_64-rpm",
    "nsis/*-setup.exe": "windows-x86_64-nsis",
    "msi/*.msi": "windows-x86_64-msi",
    "macos/*aarch64.app.tar.gz": "darwin-aarch64-app",
    "macos/*x64.app.tar.gz": "darwin-x86_64-app",
}
platforms = {}
for pattern, key in keys.items():
    for path in sorted(glob.glob(os.path.join(bundle, pattern))):
        sig = path + ".sig"
        if not os.path.exists(sig):
            sys.exit(f"no signature beside {path}: build with `pnpm bundle:signed`")
        platforms[key] = {"url": f"{base}/{os.path.basename(path)}", "signature": open(sig).read().strip()}
if not platforms:
    sys.exit("no bundles found under " + bundle)
feed = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": platforms,
}
out = os.path.join(bundle, "latest.json")
json.dump(feed, open(out, "w"), indent=2, ensure_ascii=False)
print(out, "with", ", ".join(platforms))
PY

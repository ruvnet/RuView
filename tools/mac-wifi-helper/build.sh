#!/usr/bin/env bash
# Build MacWifi.app (ADR-025 Amendment 2). Needs the Xcode Command Line Tools.
# Usage: tools/mac-wifi-helper/build.sh [output-dir]   (default: ~/Applications)
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="${1:-$HOME/Applications}"
app="$out/MacWifi.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$here/Info.plist" "$app/Contents/Info.plist"
swiftc -O "$here/mac_wifi.swift" -o "$app/Contents/MacOS/mac_wifi"
# Ad-hoc signature: enough for a locally built app to hold a TCC grant.
codesign --force --sign - --identifier net.ruview.mac-wifi-helper "$app"
codesign --verify --verbose=1 "$app"
echo "Built $app"
echo "Next: open -W \"$app\" --args --authorize   (allow the Location Services prompt)"
echo "Note: the ad-hoc signature changes on every build, so macOS forgets the grant;"
echo "      re-run --authorize after each rebuild."

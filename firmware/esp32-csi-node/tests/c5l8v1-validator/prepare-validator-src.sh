#!/usr/bin/env bash
# Writes a read-only source archive of the RuView server validator into ./validator-src:
# v2/ at VALIDATOR_REF (default 198528075cb1, where room_learner::native and the C5L8/v1
# decoder live) plus the submodules the sensing-server needs, each at the commit that ref
# pins. Submodule commits come from SUBMODULE_SEARCH (colon-separated clone roots holding
# <path>) when given, else the checkout's own submodule, else a fresh clone of the URL in
# .gitmodules (network). The working tree is never modified.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
ref=${VALIDATOR_REF:-198528075cb1}
out="$here/validator-src"; cache="$out/.cache"
rm -rf "$out"; mkdir -p "$out"
git -C "$repo" cat-file -e "$ref^{commit}" || { echo "commit $ref not in $repo; git fetch origin first" >&2; exit 2; }
git -C "$repo" archive "$ref" v2 | tar -x -C "$out"
echo "ruview $(git -C "$repo" rev-parse "$ref") v2/" > "$out/MANIFEST.txt"
for sub in v2/crates/worldgraph vendor/rufield; do
  sha=$(git -C "$repo" ls-tree "$ref" "$sub" | awk '{print $3}')
  [ -n "$sha" ] || { echo "no submodule pin for $sub at $ref" >&2; exit 2; }
  src=""
  IFS=: read -r -a roots <<< "${SUBMODULE_SEARCH:-}"
  for c in "${roots[@]/%//$sub}" "$repo/$sub"; do
    [ -n "$c" ] && git -C "$c" cat-file -e "$sha^{commit}" 2>/dev/null && { src="$c"; break; }
  done
  if [ -z "$src" ]; then
    url=$(git -C "$repo" show "$ref:.gitmodules" | git config -f - --get "submodule.$sub.url")
    src="$cache/$(basename "$sub")"; git clone -q --filter=blob:none "$url" "$src"
  fi
  mkdir -p "$out/$sub"; git -C "$src" archive "$sha" | tar -x -C "$out/$sub"
  echo "$sub $sha" >> "$out/MANIFEST.txt"
done
rm -rf "$cache"
cat "$out/MANIFEST.txt"

#!/usr/bin/env bash
#
# Roll an OTA image across a fleet, ONE node at a time, verifying each before
# touching the next.
#
# Sequential and fail-fast by design. A parallel push puts every node at risk
# simultaneously, and a node that does not come back needs physical access --
# so this stops at the first node that fails to verify rather than continuing
# into a fleet-wide outage. The nodes it has not reached yet are left alone.
#
# Node addresses are discovered from the running server rather than hard-coded,
# so this works on any fleet without editing the script.
#
# Usage:
#   export RUVIEW_OTA_PSK_FILE=/path/to/ota_psk.txt
#   ./fleet_ota.sh                                   # discover from the server
#   ./fleet_ota.sh --nodes "0:10.0.0.5 1:10.0.0.6"   # or name them explicitly
#
# Options:
#   --server URL     server to discover nodes from (default http://127.0.0.1:3000)
#   --nodes "LIST"   explicit "id:ip id:ip ..." instead of discovery
#   --bin PATH       image to push (default build/esp32-csi-node.bin)
#   --version STR    version to verify against (default: contents of version.txt)
#   --first ID       roll this node first; use a board you can recover
#   --settle SECS    pause between nodes (default 20)
set -u

SERVER="http://127.0.0.1:3000"
NODES=""
BIN="build/esp32-csi-node.bin"
VER=""
FIRST=""
SETTLE=20

while [ $# -gt 0 ]; do
  case "$1" in
    --server)  SERVER="$2"; shift 2 ;;
    --nodes)   NODES="$2";  shift 2 ;;
    --bin)     BIN="$2";    shift 2 ;;
    --version) VER="$2";    shift 2 ;;
    --first)   FIRST="$2";  shift 2 ;;
    --settle)  SETTLE="$2"; shift 2 ;;
    -h|--help) sed -n '2,26p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")"

if [ -z "${RUVIEW_OTA_PSK_FILE:-}" ] || [ ! -f "$RUVIEW_OTA_PSK_FILE" ]; then
  echo "set RUVIEW_OTA_PSK_FILE to the file holding the OTA PSK" >&2
  echo "(a path, not the key itself -- an environment variable holding the key" >&2
  echo " is visible in a process listing, and that key can replace firmware)" >&2
  exit 2
fi

[ -z "$VER" ] && [ -f version.txt ] && VER="$(tr -d '\r\n' < version.txt)"
if [ -z "$VER" ]; then
  echo "no --version given and version.txt is missing or empty" >&2
  exit 2
fi
if [ ! -f "$BIN" ]; then
  echo "image not found: $BIN (build it first)" >&2
  exit 2
fi

# Discover from the server unless the caller named the nodes. /api/v1/nodes
# reports node_id and address for everything currently reporting, which is
# exactly the set worth updating -- a node the server has never heard from
# cannot be reached over the network anyway.
if [ -z "$NODES" ]; then
  NODES=$(python -c "
import json,sys,urllib.request
try:
    d=json.load(urllib.request.urlopen('$SERVER/api/v1/nodes', timeout=10))
except Exception as e:
    sys.exit('could not reach $SERVER: %s' % e)
ns=d.get('nodes', d) if isinstance(d, dict) else d
out=[f\"{n['node_id']}:{n['ip']}\" for n in ns if n.get('ip')]
if not out: sys.exit('server reported no nodes with an address')
print(' '.join(sorted(out, key=lambda s: int(s.split(':')[0]))))
") || exit 1
  echo "discovered $(echo "$NODES" | wc -w) nodes from $SERVER"
fi

# Roll a recoverable board first when asked. If the image is bad, it lands
# where it is cheapest to fix rather than on whichever node sorts lowest.
if [ -n "$FIRST" ]; then
  head=""; rest=""
  for e in $NODES; do
    case "$e" in "$FIRST":*) head="$e" ;; *) rest="$rest $e" ;; esac
  done
  [ -n "$head" ] && NODES="$head$rest"
fi

echo "rolling $VER from $BIN"
ok=0; fail=0
for entry in $NODES; do
  id="${entry%%:*}"; ip="${entry##*:}"
  echo "=============== node $id ($ip) ==============="
  if python ota_push.py --node "$ip" --bin "$BIN" --expect-version "$VER"; then
    ok=$((ok+1))
    # Settle before the next node so a struggling board is not masked by the
    # next upload competing for the same airtime.
    sleep "$SETTLE"
  else
    fail=$((fail+1))
    echo "ABORTING: node $id ($ip) did not verify. $ok done, $fail failed." >&2
    echo "Remaining nodes were NOT touched." >&2
    exit 1
  fi
done
echo "fleet roll complete: $ok verified, $fail failed"

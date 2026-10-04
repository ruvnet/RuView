#!/usr/bin/env sh
set -eu

# --------------------------------------------------------------------------
# RuView sensing-server entrypoint - hardened version
#
# Fixes ruvnet/RuView issue #864:
#   Original behavior started the server with --bind-addr 0.0.0.0 always,
#   regardless of whether RUVIEW_API_TOKEN was set, meaning the default
#   Docker path exposed live sensing/pose data with auth silently OFF.
#
# This version:
#   1. Refuses to start bound to a non-loopback address unless
#      RUVIEW_API_TOKEN is set AND RUVIEW_LAN_MODE=1 is explicitly passed
#      (fail-closed instead of fail-open).
#   2. Defaults RUVIEW_BIND_ADDR to 127.0.0.1 instead of 0.0.0.0.
#   3. Logs the actual auth/bind posture loudly on startup so it's obvious
#      what mode you're running in (matches the log lines referenced in
#      the original bug report, kept for compatibility with tooling that
#      parses them).
# --------------------------------------------------------------------------

BIND_ADDR="${RUVIEW_BIND_ADDR:-127.0.0.1}"
HTTP_PORT="${RUVIEW_HTTP_PORT:-3000}"
WS_PORT="${RUVIEW_WS_PORT:-3001}"
UDP_PORT="${RUVIEW_UDP_PORT:-5005}"
CSI_SOURCE="${CSI_SOURCE:-simulated}"
LAN_MODE="${RUVIEW_LAN_MODE:-0}"

# --- Fail-closed checks -----------------------------------------------------

if [ "$BIND_ADDR" != "127.0.0.1" ] && [ "$BIND_ADDR" != "localhost" ]; then
    if [ "$LAN_MODE" != "1" ]; then
        echo "FATAL: RUVIEW_BIND_ADDR=$BIND_ADDR requests a non-loopback bind," >&2
        echo "       but RUVIEW_LAN_MODE=1 was not set. Refusing to start." >&2
        echo "       Set RUVIEW_LAN_MODE=1 explicitly if you understand this" >&2
        echo "       exposes the sensing API/WebSocket beyond localhost." >&2
        exit 1
    fi
    if [ -z "${RUVIEW_API_TOKEN:-}" ]; then
        echo "FATAL: Non-loopback bind requested (RUVIEW_LAN_MODE=1) but" >&2
        echo "       RUVIEW_API_TOKEN is empty. Refusing to start with auth" >&2
        echo "       disabled on a non-local interface." >&2
        exit 1
    fi
fi

# Even on loopback, warn loudly (but don't block) if no token is set -
# useful for local dev, dangerous if someone port-forwards later.
if [ -z "${RUVIEW_API_TOKEN:-}" ]; then
    echo "WARNING: RUVIEW_API_TOKEN is not set. API auth: OFF." >&2
    echo "         /api/v1/* is unauthenticated. Set RUVIEW_API_TOKEN=<token>" >&2
    echo "         to enforce bearer auth." >&2
else
    echo "API auth: ON - RUVIEW_API_TOKEN is set."
fi

echo "HTTP server listening on ${BIND_ADDR}:${HTTP_PORT}"
echo "WebSocket server listening on ${BIND_ADDR}:${WS_PORT}"
echo "NOTE: /ws/sensing auth depends on the sensing-server binary version." >&2
echo "      This entrypoint cannot enforce WS auth on its own - see the" >&2
echo "      ws-auth-proxy sidecar in docker-compose.yml for a gateway-level" >&2
echo "      fix if your binary doesn't yet support token-gated WebSockets." >&2

exec /app/sensing-server \
    --source "$CSI_SOURCE" \
    --bind-addr "$BIND_ADDR" \
    --http-port "$HTTP_PORT" \
    --ws-port "$WS_PORT" \
    --udp-port "$UDP_PORT" \
    ${RUVIEW_API_TOKEN:+--api-token "$RUVIEW_API_TOKEN"} \
    "$@"

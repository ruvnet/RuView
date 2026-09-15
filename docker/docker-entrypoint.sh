#!/bin/sh
# Docker entrypoint for WiFi-DensePose sensing server.
#
# Supports two usage patterns:
#
# 1. No arguments — use defaults from environment:
#      docker run -e CSI_SOURCE=esp32 ruvnet/wifi-densepose:latest
#
# 2. Pass CLI flags directly:
#      docker run ruvnet/wifi-densepose:latest --source esp32 --tick-ms 500
#      docker run ruvnet/wifi-densepose:latest --model /app/models/my.rvf
#
# Environment variables:
#   CSI_SOURCE   — data source. Valid values:
#                    auto       — try ESP32 then Windows WiFi, **fail-loud if no
#                                 real hardware is detected** (issue #937 fix:
#                                 the server no longer silently falls back to
#                                 synthetic data — that's now opt-in only).
#                    esp32      — listen for UDP CSI on the configured port.
#                    wifi       — Windows-native WiFi capture.
#                    simulated  — explicit demo mode with synthetic CSI.
#                  Default is `auto`. Set CSI_SOURCE=simulated when you want
#                  fake data tagged as such; never set it implicitly.
#   MODELS_DIR   — directory to scan for .rvf model files (default: data/models)
set -e

# ── Issue #864: fail-closed on default posture ───────────────────────────────
# The pre-fix default was: empty RUVIEW_API_TOKEN (auth off) + --bind-addr
# 0.0.0.0 + docker-compose publishing :3000/:3001/:5005 → an unauthenticated
# attacker on any reachable network segment could read /api/v1/sensing/latest
# and the /ws/sensing live stream. That posture is unsafe on guest WiFi,
# untrusted LANs, accidentally-port-forwarded hosts, or any reverse-proxied
# deployment. Refuse to start with this combination.
#
# Escape hatches (operator must opt in explicitly):
#   * Set RUVIEW_API_TOKEN to a strong secret → auth enabled on /api/v1/*.
#   * Set RUVIEW_ALLOW_UNAUTHENTICATED=1 → preserves the pre-fix behaviour;
#     only safe on an isolated trust boundary.
#   * Set RUVIEW_BIND_ADDR to a loopback / private interface → unauth is fine
#     when the socket isn't reachable. The auto-bind nudges toward 127.0.0.1.
#
# This check runs only for the default sensing-server path (no args + flag-only
# args). The `cog-ha-matter` / `homecore` routes below are excluded because
# they own their own auth lifecycle.
case "${1:-}" in
    cog-ha-matter|ha-matter|homecore|homecore-server) ;;
    *)
        if [ -z "${RUVIEW_API_TOKEN:-}" ] && [ "${RUVIEW_ALLOW_UNAUTHENTICATED:-}" != "1" ]; then
            # If the operator hasn't overridden the bind, refuse outright on
            # the default 0.0.0.0. If they've nailed it to loopback (or a
            # specific private address they trust), let it run.
            __bind_default="${RUVIEW_BIND_ADDR:-0.0.0.0}"
            case "$__bind_default" in
                127.*|localhost|::1)
                    : ;;  # loopback bind is safe even without a token
                *)
                    echo "[entrypoint] ERROR: refusing to start sensing-server with default" >&2
                    echo "[entrypoint]        posture: RUVIEW_API_TOKEN is unset AND bind is" >&2
                    echo "[entrypoint]        ${__bind_default}. /ws/sensing streams live sensing" >&2
                    echo "[entrypoint]        frames; that data would be readable by anyone who" >&2
                    echo "[entrypoint]        can reach this host. Pick one:" >&2
                    echo "[entrypoint]          docker run -e RUVIEW_API_TOKEN=\$(openssl rand -hex 32) ..." >&2
                    echo "[entrypoint]          docker run -e RUVIEW_BIND_ADDR=127.0.0.1 ..." >&2
                    echo "[entrypoint]          docker run -e RUVIEW_ALLOW_UNAUTHENTICATED=1 ...   # only on trusted network" >&2
                    echo "[entrypoint]        See https://github.com/ruvnet/RuView/issues/864" >&2
                    exit 64
                    ;;
            esac
        fi
        ;;
esac

# Route to cog-ha-matter (ADR-116) when invoked as:
#   docker run <image> cog-ha-matter [--flags]
# or via the short alias `ha-matter`. Strips the keyword and execs the
# Home Assistant + Matter cog binary, defaulting --sensing-url to the
# co-located sensing-server endpoint so docker-compose deployments work
# out of the box.
case "${1:-}" in
    cog-ha-matter|ha-matter)
        shift
        exec /app/cog-ha-matter \
            --sensing-url "${SENSING_URL:-http://127.0.0.1:3000}" \
            "$@"
        ;;
    homecore|homecore-server)
        # Route to the HOMECORE native Rust port of Home Assistant
        # (ADRs 126-134, v0.10.0). Default bind matches HA at :8123.
        shift
        exec /app/homecore-server \
            --bind "${HOMECORE_BIND:-0.0.0.0:8123}" \
            "$@"
        ;;
esac

# If the first argument looks like a flag (starts with -), prepend the
# server binary so users can just pass flags:
#   docker run <image> --source esp32 --tick-ms 500
if [ "${1#-}" != "$1" ] || [ -z "$1" ]; then
    set -- /app/sensing-server \
        --source "${CSI_SOURCE:-auto}" \
        --tick-ms 100 \
        --ui-path /app/ui \
        --http-port 3000 \
        --ws-port 3001 \
        --bind-addr "${RUVIEW_BIND_ADDR:-0.0.0.0}" \
        "$@"

    # Append Neo4j flags if RUVIEW_NEO4J is set.
    if [ "${RUVIEW_NEO4J:-}" = "1" ] || [ "${RUVIEW_NEO4J:-}" = "true" ]; then
        set -- "$@" \
            --neo4j \
            --neo4j-url "${RUVIEW_NEO4J_URL:-bolt://x1-370:7687}" \
            --neo4j-user "${RUVIEW_NEO4J_USER:-neo4j}" \
            --neo4j-password-env "${RUVIEW_NEO4J_PASSWORD_ENV:-NEO4J_PASSWORD}" \
            --neo4j-room-name "${RUVIEW_NEO4J_ROOM:-main}" \
            --neo4j-ttl-hours "${RUVIEW_NEO4J_TTL_HOURS:-168}"
    fi
fi

exec "$@"

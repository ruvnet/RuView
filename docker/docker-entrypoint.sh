#!/bin/sh
# Docker entrypoint for WiFi-DensePose sensing server.
set -e

# Fail closed for the network-facing sensing server. Explicit alternate binary
# routes own their own transport/auth lifecycle.
case "${1:-}" in
    cog-ha-matter|ha-matter|homecore|homecore-server|sensing-mcp|mcp) ;;
    *)
        if [ -z "${RUVIEW_API_TOKEN:-}" ] && [ "${RUVIEW_ALLOW_UNAUTHENTICATED:-}" != "1" ]; then
            __bind_default="${RUVIEW_BIND_ADDR:-0.0.0.0}"
            case "$__bind_default" in
                127.*|localhost|::1) : ;;
                *)
                    echo "[entrypoint] ERROR: refusing unauthenticated network-facing sensing-server on ${__bind_default}" >&2
                    echo "[entrypoint] set RUVIEW_API_TOKEN, bind to loopback, or explicitly opt into trusted-LAN mode" >&2
                    exit 64
                    ;;
            esac
        fi
        ;;
esac

case "${1:-}" in
    cog-ha-matter|ha-matter)
        shift
        exec /app/cog-ha-matter --sensing-url "${SENSING_URL:-http://127.0.0.1:3000}" "$@"
        ;;
    homecore|homecore-server)
        shift
        exec /app/homecore-server --bind "${HOMECORE_BIND:-0.0.0.0:8123}" "$@"
        ;;
    sensing-mcp|mcp)
        # MCP is stdio-only and must use a dedicated read-only Neo4j principal.
        # Do not reuse the sensing sink/maintenance credential here.
        shift
        : "${RUVIEW_MCP_NEO4J_USER:?RUVIEW_MCP_NEO4J_USER must name a dedicated read-only Neo4j principal}"
        : "${RUVIEW_MCP_NEO4J_PASSWORD_ENV:=RUVIEW_MCP_NEO4J_PASSWORD}"
        : "${RUVIEW_MCP_ROOM:?RUVIEW_MCP_ROOM must scope this MCP process to one authorized room}"
        export RUVIEW_NEO4J_USER="$RUVIEW_MCP_NEO4J_USER"
        export RUVIEW_NEO4J_ROOM="$RUVIEW_MCP_ROOM"
        exec /app/sensing-mcp \
            --neo4j-user "$RUVIEW_MCP_NEO4J_USER" \
            --neo4j-password-env "$RUVIEW_MCP_NEO4J_PASSWORD_ENV" \
            --room "$RUVIEW_MCP_ROOM" \
            "$@"
        ;;
esac

if [ "${1#-}" != "$1" ] || [ -z "$1" ]; then
    set -- /app/sensing-server \
        --source "${CSI_SOURCE:-auto}" \
        --tick-ms 100 \
        --ui-path /app/ui \
        --http-port 3000 \
        --ws-port 3001 \
        --bind-addr "${RUVIEW_BIND_ADDR:-0.0.0.0}" \
        "$@"

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

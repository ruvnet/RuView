#!/bin/sh
set -eu

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/app" "$TMP/bin"

cat > "$TMP/app/sensing-mcp" <<'EOF'
#!/bin/sh
printf '%s\n' "$*"
EOF
cat > "$TMP/app/cog-ha-matter" <<'EOF'
#!/bin/sh
printf 'matter:%s\n' "$*"
EOF
cat > "$TMP/app/homecore-server" <<'EOF'
#!/bin/sh
printf 'homecore:%s\n' "$*"
EOF
cat > "$TMP/app/sensing-server" <<'EOF'
#!/bin/sh
printf 'server:%s\n' "$*"
EOF
chmod +x "$TMP/app/"*

sed "s#/app/#$TMP/app/#g" docker/docker-entrypoint.sh > "$TMP/entrypoint.sh"
chmod +x "$TMP/entrypoint.sh"

# Legacy routes remain executable.
"$TMP/entrypoint.sh" cog-ha-matter --help | grep -q '^matter:'
"$TMP/entrypoint.sh" homecore --help | grep -q '^homecore:'

# Default network-facing unauthenticated server remains fail-closed.
if "$TMP/entrypoint.sh" >/dev/null 2>&1; then
  echo 'expected unauthenticated network-facing launch to fail' >&2
  exit 1
fi

# MCP must fail closed unless room, dedicated principal and privacy posture exist.
if RUVIEW_MCP_ROOM=lab RUVIEW_MCP_PRIVACY_MODE=true "$TMP/entrypoint.sh" mcp >/dev/null 2>&1; then
  echo 'expected MCP launch without dedicated principal to fail' >&2
  exit 1
fi

# Invalid privacy posture is rejected.
if RUVIEW_MCP_NEO4J_USER=reader RUVIEW_MCP_ROOM=lab RUVIEW_MCP_PRIVACY_MODE=maybe "$TMP/entrypoint.sh" mcp >/dev/null 2>&1; then
  echo 'expected invalid privacy posture to fail' >&2
  exit 1
fi

# A fully scoped MCP launch reaches the MCP binary with the authorized room only.
out="$(RUVIEW_MCP_NEO4J_USER=reader RUVIEW_MCP_ROOM=lab RUVIEW_MCP_PRIVACY_MODE=true "$TMP/entrypoint.sh" mcp)"
printf '%s\n' "$out" | grep -q -- '--room lab'
printf '%s\n' "$out" | grep -q -- '--neo4j-user reader'

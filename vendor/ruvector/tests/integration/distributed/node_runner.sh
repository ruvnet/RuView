#!/bin/bash
# Ruvector Distributed Node Runner Script

set -e

echo "=== Ruvector Distributed Node ==="
echo "Node ID: ${NODE_ID}"
echo "Role: ${NODE_ROLE}"
echo "Raft Port: ${RAFT_PORT}"
echo "Cluster Port: ${CLUSTER_PORT}"
echo "Replication Port: ${REPLICATION_PORT}"
echo "Cluster Members: ${CLUSTER_MEMBERS}"
echo "Shard Count: ${SHARD_COUNT}"
echo "Replication Factor: ${REPLICATION_FACTOR}"
echo "================================="

# Health check endpoint (simple HTTP server)
start_health_server() {
    while true; do
        echo -e "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK" | nc -l -p ${CLUSTER_PORT} -q 1 2>/dev/null || true
    done
}

# Start health server in background
start_health_server &
HEALTH_PID=$!

# Trap to cleanup on exit
cleanup() {
    echo "Shutting down node ${NODE_ID}..."
    kill $HEALTH_PID 2>/dev/null || true
    exit 0
}
trap cleanup SIGTERM SIGINT

echo "Node ${NODE_ID} is running..."

# Keep container running
while true; do
    sleep 5
    echo "[${NODE_ID}] Heartbeat - Role: ${NODE_ROLE}"
done

#!/bin/bash
# RuView Web App - Interactive Explorer & Learning Platform
# Serves the webapp on port 3001 (avoids conflict with existing UI on 3000)

PORT=${1:-3001}

echo "================================================"
echo "  RuView - WiFi DensePose Interactive Explorer"
echo "================================================"
echo ""
echo "  Web App:  http://localhost:${PORT}"
echo "  Press Ctrl+C to stop"
echo ""

cd "$(dirname "$0")"
python3 -m http.server "$PORT"

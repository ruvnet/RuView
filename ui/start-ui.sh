#!/bin/bash

# WiFi DensePose UI Startup Script
# This script starts the UI on port 3000 to avoid conflicts with the FastAPI backend on port 8000

echo "🚀 Starting WiFi DensePose UI..."
echo ""
echo "📋 Configuration:"
echo "   - UI Server: http://localhost:3000"
echo "   - Backend API: http://localhost:8000 (make sure it's running)"
echo "   - Test Runner: http://localhost:3000/tests/test-runner.html"
echo "   - Integration Tests: http://localhost:3000/tests/integration-test.html"
echo ""

# Check if port 3000 is already in use
if lsof -Pi :3000 -sTCP:LISTEN -t >/dev/null ; then
    echo "⚠️  Port 3000 is already in use. Please stop the existing server or use a different port."
    echo "   You can manually start with: python -m http.server 3001"
    exit 1
fi

# Check if FastAPI backend is running on port 8000
if lsof -Pi :8000 -sTCP:LISTEN -t >/dev/null ; then
    echo "✅ FastAPI backend detected on port 8000"
else
    echo "⚠️  FastAPI backend not detected on port 8000"
    echo "   Please start it with: wifi-densepose start"
    echo "   Or: python -m wifi_densepose.main"
    echo ""
    echo "   The UI will still work with the mock server for testing."
fi

echo ""
echo "🌐 Starting HTTP server on port 3000..."
echo "   Press Ctrl+C to stop"
echo ""

# Start the HTTP server
python -m http.server 3000
#!/bin/bash
#
# RuVector Benchmark Setup Script
# Sets up the benchmarking environment
#

set -e

echo "=========================================="
echo "RuVector Benchmark Suite Setup"
echo "=========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if k6 is installed
echo -n "Checking for k6... "
if command -v k6 &> /dev/null; then
    echo -e "${GREEN}✓ Found k6 $(k6 version --quiet)${NC}"
else
    echo -e "${RED}✗ k6 not found${NC}"
    echo ""
    echo "Please install k6:"
    echo "  macOS:   brew install k6"
    echo "  Linux:   See https://k6.io/docs/getting-started/installation/"
    echo "  Windows: choco install k6"
    exit 1
fi

# Check if Node.js is installed
echo -n "Checking for Node.js... "
if command -v node &> /dev/null; then
    echo -e "${GREEN}✓ Found Node.js $(node --version)${NC}"
else
    echo -e "${RED}✗ Node.js not found${NC}"
    echo "Please install Node.js v18 or higher"
    exit 1
fi

# Check if TypeScript is installed
echo -n "Checking for TypeScript... "
if command -v ts-node &> /dev/null; then
    echo -e "${GREEN}✓ Found ts-node${NC}"
else
    echo -e "${YELLOW}! ts-node not found, installing...${NC}"
    npm install -g typescript ts-node
fi

# Check for Claude Flow (optional)
echo -n "Checking for Claude Flow... "
if command -v claude-flow &> /dev/null; then
    echo -e "${GREEN}✓ Found claude-flow${NC}"
    HOOKS_ENABLED=true
else
    echo -e "${YELLOW}! claude-flow not found (optional)${NC}"
    HOOKS_ENABLED=false
fi

# Create results directory
echo -n "Creating results directory... "
mkdir -p results
echo -e "${GREEN}✓${NC}"

# Set up environment
echo ""
echo "Setting up environment..."
echo ""

# Prompt for BASE_URL
read -p "Enter RuVector cluster URL (default: http://localhost:8080): " BASE_URL
BASE_URL=${BASE_URL:-http://localhost:8080}

# Create .env file
cat > .env << EOF
# RuVector Benchmark Configuration
BASE_URL=${BASE_URL}
PARALLEL=1
ENABLE_HOOKS=${HOOKS_ENABLED}
LOG_LEVEL=info

# Optional: Slack notifications
# SLACK_WEBHOOK_URL=https://hooks.slack.com/services/...

# Optional: Email notifications
# EMAIL_NOTIFICATION=team@example.com
EOF

echo -e "${GREEN}✓ Created .env file${NC}"

# Make scripts executable
chmod +x setup.sh
chmod +x benchmark-runner.ts 2>/dev/null || true

echo ""
echo "=========================================="
echo -e "${GREEN}Setup Complete!${NC}"
echo "=========================================="
echo ""
echo "Quick Start:"
echo ""
echo "  # List available scenarios"
echo "  ts-node benchmark-runner.ts list"
echo ""
echo "  # Run quick validation (45 minutes)"
echo "  ts-node benchmark-runner.ts run baseline_100m"
echo ""
echo "  # Run standard test suite"
echo "  ts-node benchmark-runner.ts group standard_suite"
echo ""
echo "  # View results"
echo "  open visualization-dashboard.html"
echo ""
echo "For detailed documentation, see README.md"
echo ""

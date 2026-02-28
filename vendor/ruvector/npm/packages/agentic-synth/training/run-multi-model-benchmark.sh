#!/usr/bin/env bash
#
# DSPy Multi-Model Benchmark Runner
#
# Usage:
#   ./run-multi-model-benchmark.sh [sample_size]
#
# Examples:
#   ./run-multi-model-benchmark.sh           # Default: 100 samples
#   ./run-multi-model-benchmark.sh 1000      # 1000 samples
#   SAMPLE_SIZE=50 ./run-multi-model-benchmark.sh  # 50 samples
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default sample size
SAMPLE_SIZE=${1:-${SAMPLE_SIZE:-100}}

echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       DSPy Multi-Model Benchmark Suite Runner                  ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check for API keys
echo -e "${YELLOW}🔍 Checking API keys...${NC}"

if [ -z "$OPENAI_API_KEY" ] && [ -z "$ANTHROPIC_API_KEY" ]; then
    echo -e "${RED}❌ Error: No API keys found!${NC}"
    echo ""
    echo "Please set at least one of the following:"
    echo "  export OPENAI_API_KEY='your-key'"
    echo "  export ANTHROPIC_API_KEY='your-key'"
    echo ""
    echo "Or create a .env file with:"
    echo "  OPENAI_API_KEY=your-key"
    echo "  ANTHROPIC_API_KEY=your-key"
    exit 1
fi

if [ -n "$OPENAI_API_KEY" ]; then
    echo -e "${GREEN}✓ OpenAI API key found${NC}"
fi

if [ -n "$ANTHROPIC_API_KEY" ]; then
    echo -e "${GREEN}✓ Anthropic API key found${NC}"
fi

echo ""

# Check dependencies
echo -e "${YELLOW}🔍 Checking dependencies...${NC}"

if ! command -v npx &> /dev/null; then
    echo -e "${RED}❌ Error: npx not found. Please install Node.js.${NC}"
    exit 1
fi

if ! [ -f "node_modules/dspy.ts/package.json" ]; then
    echo -e "${YELLOW}⚠️  dspy.ts not found. Installing...${NC}"
    npm install
fi

echo -e "${GREEN}✓ All dependencies ready${NC}"
echo ""

# Display configuration
echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Configuration                               ║${NC}"
echo -e "${BLUE}╠════════════════════════════════════════════════════════════════╣${NC}"
echo -e "${BLUE}║${NC} Sample Size: ${YELLOW}${SAMPLE_SIZE}${NC}"
echo -e "${BLUE}║${NC} Output Dir:  ${YELLOW}./training/results/multi-model${NC}"
echo -e "${BLUE}║${NC} Models:      ${YELLOW}All available (based on API keys)${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Run benchmark
echo -e "${GREEN}🚀 Starting benchmark...${NC}"
echo ""

export SAMPLE_SIZE=$SAMPLE_SIZE

if npx tsx training/dspy-multi-model-benchmark.ts; then
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                  ✅ Benchmark Completed!                        ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${YELLOW}📊 Results saved to:${NC}"
    echo -e "   ${BLUE}./training/results/multi-model/${NC}"
    echo ""
    echo -e "${YELLOW}📄 View reports:${NC}"
    ls -lh training/results/multi-model/*.md 2>/dev/null | tail -1 | awk '{print "   " $9 " (" $5 ")"}'
    ls -lh training/results/multi-model/*.json 2>/dev/null | tail -1 | awk '{print "   " $9 " (" $5 ")"}'
    echo ""
else
    echo ""
    echo -e "${RED}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║                  ❌ Benchmark Failed!                           ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${YELLOW}💡 Troubleshooting tips:${NC}"
    echo "   1. Check your API keys are valid"
    echo "   2. Ensure you have network connectivity"
    echo "   3. Try with a smaller sample size: ./run-multi-model-benchmark.sh 10"
    echo "   4. Check the error message above for details"
    echo ""
    exit 1
fi

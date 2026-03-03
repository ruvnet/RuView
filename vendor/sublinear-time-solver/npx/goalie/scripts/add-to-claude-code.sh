#!/bin/bash

# Add Goalie to Claude Code MCP Configuration
# This script adds the Goalie MCP server to Claude Code

echo "🥅 Adding Goalie to Claude Code MCP Configuration"
echo "================================================="
echo ""

# Check if API key is set
if [ -z "$PERPLEXITY_API_KEY" ]; then
    echo "⚠️  Warning: PERPLEXITY_API_KEY environment variable not set"
    echo "📝 You'll need to add it to the MCP configuration"
    echo ""
    read -p "Enter your Perplexity API key (or press Enter to skip): " api_key
    if [ ! -z "$api_key" ]; then
        export PERPLEXITY_API_KEY="$api_key"
    fi
fi

# Method 1: Add using npx command (recommended)
echo "Method 1: Using npx (recommended)"
echo "---------------------------------"
echo "Run this command:"
echo ""
echo "claude mcp add goalie npx goalie"
echo ""

# Method 2: Add with environment variable
if [ ! -z "$PERPLEXITY_API_KEY" ]; then
    echo "Method 2: With API key configured"
    echo "----------------------------------"
    echo "Run this command:"
    echo ""
    echo "claude mcp add goalie npx goalie --env PERPLEXITY_API_KEY=$PERPLEXITY_API_KEY"
    echo ""
fi

# Method 3: Add using JSON configuration
echo "Method 3: Using JSON configuration"
echo "-----------------------------------"
echo "Run this command:"
echo ""

# Create JSON config
json_config='{
  "command": "npx",
  "args": ["goalie"],
  "env": {
    "PERPLEXITY_API_KEY": "'${PERPLEXITY_API_KEY:-YOUR_API_KEY_HERE}'"
  }
}'

# Escape the JSON for command line
escaped_json=$(echo "$json_config" | jq -c . 2>/dev/null || echo "$json_config" | tr -d '\n')

echo "claude mcp add-json goalie '$escaped_json'"
echo ""

# Method 4: Manual configuration
echo "Method 4: Manual configuration file"
echo "------------------------------------"
echo "Add to your Claude Code MCP config:"
echo ""
cat << EOF
{
  "mcpServers": {
    "goalie": {
      "command": "npx",
      "args": ["goalie"],
      "env": {
        "PERPLEXITY_API_KEY": "${PERPLEXITY_API_KEY:-YOUR_API_KEY_HERE}"
      }
    }
  }
}
EOF

echo ""
echo "================================================="
echo "📋 Quick Commands to Copy:"
echo ""
echo "1. Simple add:"
echo "   claude mcp add goalie npx goalie"
echo ""
echo "2. List servers:"
echo "   claude mcp list"
echo ""
echo "3. Test Goalie:"
echo "   claude mcp get goalie"
echo ""
echo "4. Remove (if needed):"
echo "   claude mcp remove goalie"
echo ""
echo "================================================="
echo "✅ Ready to add Goalie to Claude Code!"
echo ""
echo "After adding, you can use Goalie's tools in Claude Code:"
echo "  • goap.search - Multi-step planning search"
echo "  • search.raw - Direct Perplexity search"
echo ""
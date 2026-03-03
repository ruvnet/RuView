#!/bin/bash

# Load environment variables from .env file
export $(cat /workspaces/sublinear-time-solver/npx/goap/.env | grep -v '^#' | xargs)

# Start the MCP server
exec node /workspaces/sublinear-time-solver/npx/goap/dist/cli.js start
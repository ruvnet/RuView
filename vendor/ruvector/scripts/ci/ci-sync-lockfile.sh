#!/bin/bash
# CI/CD script to auto-fix package-lock.json and create a commit
# Use this in GitHub Actions to automatically fix lock file issues

set -e

echo "🔍 Checking package-lock.json sync for CI/CD..."

cd npm

# Try npm ci first to check if lock file is in sync
if npm ci --dry-run 2>&1 | grep -q "can only install packages when your package.json and package-lock.json"; then
    echo "❌ Lock file out of sync - fixing automatically..."

    # Update lock file
    npm install

    # Check if we're in a git repository and have changes
    if git diff --quiet npm/package-lock.json; then
        echo "✅ Lock file is now in sync (no changes needed)"
    else
        echo "✅ Lock file updated"

        # If running in GitHub Actions, commit and push
        if [ -n "$GITHUB_ACTIONS" ]; then
            git config user.name "github-actions[bot]"
            git config user.email "github-actions[bot]@users.noreply.github.com"
            git add npm/package-lock.json
            git commit -m "chore: Auto-sync package-lock.json [skip ci]"
            git push
            echo "✅ Lock file committed and pushed"
        else
            echo "⚠️  Lock file updated but not committed (not in GitHub Actions)"
        fi
    fi
else
    echo "✅ Lock file is already in sync"
fi

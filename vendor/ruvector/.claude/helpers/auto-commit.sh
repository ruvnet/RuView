#!/bin/bash
# Auto-commit helper for Claude Code hooks
# Handles git add, commit, and push in a robust way

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Configuration
MIN_CHANGES=${MIN_CHANGES:-1}
COMMIT_PREFIX=${COMMIT_PREFIX:-"checkpoint"}
AUTO_PUSH=${AUTO_PUSH:-true}

log() {
    echo -e "${GREEN}[auto-commit]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[auto-commit]${NC} $1"
}

error() {
    echo -e "${RED}[auto-commit]${NC} $1"
}

# Check if there are changes to commit
has_changes() {
    ! git diff --quiet HEAD 2>/dev/null || ! git diff --cached --quiet 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]
}

# Count changes
count_changes() {
    local staged=$(git diff --cached --numstat | wc -l)
    local unstaged=$(git diff --numstat | wc -l)
    local untracked=$(git ls-files --others --exclude-standard | wc -l)
    echo $((staged + unstaged + untracked))
}

# Main auto-commit function
auto_commit() {
    local message="$1"
    local file="$2"  # Optional specific file

    # Check if in a git repo
    if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        error "Not in a git repository"
        return 1
    fi

    # Check for changes
    if ! has_changes; then
        log "No changes to commit"
        return 0
    fi

    local change_count=$(count_changes)
    if [ "$change_count" -lt "$MIN_CHANGES" ]; then
        log "Only $change_count change(s), skipping (min: $MIN_CHANGES)"
        return 0
    fi

    # Stage changes
    if [ -n "$file" ] && [ -f "$file" ]; then
        git add "$file"
        log "Staged: $file"
    else
        git add -A
        log "Staged all changes ($change_count files)"
    fi

    # Create commit message
    local branch=$(git branch --show-current)
    local timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    if [ -z "$message" ]; then
        message="$COMMIT_PREFIX: Auto-commit from Claude Code"
    fi

    # Commit
    if git commit -m "$message

Automatic checkpoint created by Claude Code
- Branch: $branch
- Timestamp: $timestamp
- Changes: $change_count file(s)

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>" --quiet 2>/dev/null; then
        log "Created commit: $message"

        # Push if enabled
        if [ "$AUTO_PUSH" = "true" ]; then
            if git push origin "$branch" --quiet 2>/dev/null; then
                log "Pushed to origin/$branch"
            else
                warn "Push failed (will retry later)"
            fi
        fi

        return 0
    else
        warn "Commit failed (possibly nothing to commit)"
        return 1
    fi
}

# Batch commit (commits all changes together)
batch_commit() {
    local message="${1:-Batch checkpoint}"
    auto_commit "$message"
}

# Single file commit
file_commit() {
    local file="$1"
    local message="${2:-Checkpoint: $file}"

    if [ -z "$file" ]; then
        error "No file specified"
        return 1
    fi

    if [ ! -f "$file" ]; then
        error "File not found: $file"
        return 1
    fi

    auto_commit "$message" "$file"
}

# Push only (no commit)
push_only() {
    local branch=$(git branch --show-current)

    if git push origin "$branch" 2>/dev/null; then
        log "Pushed to origin/$branch"
    else
        warn "Push failed"
        return 1
    fi
}

# Entry point
case "${1:-batch}" in
    batch)
        batch_commit "$2"
        ;;
    file)
        file_commit "$2" "$3"
        ;;
    push)
        push_only
        ;;
    check)
        if has_changes; then
            echo "Changes detected: $(count_changes) files"
            exit 0
        else
            echo "No changes"
            exit 1
        fi
        ;;
    *)
        echo "Usage: $0 {batch|file|push|check} [args]"
        echo ""
        echo "Commands:"
        echo "  batch [message]     Commit all changes with optional message"
        echo "  file <path> [msg]   Commit specific file"
        echo "  push                Push without committing"
        echo "  check               Check if there are uncommitted changes"
        exit 1
        ;;
esac

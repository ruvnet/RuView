#!/bin/bash
# Claude Checkpoint Manager
# Provides easy rollback and management of Claude Code checkpoints

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CHECKPOINT_DIR=".claude/checkpoints"
BACKUP_DIR=".claude/backups"

# Help function
show_help() {
    cat << EOF
Claude Checkpoint Manager
========================

Usage: $0 <command> [options]

Commands:
  list              List all checkpoints
  show <id>         Show details of a specific checkpoint
  rollback <id>     Rollback to a specific checkpoint
  diff <id>         Show diff since checkpoint
  clean             Clean old checkpoints (older than 7 days)
  summary           Show session summary
  
Options:
  --hard            For rollback: use git reset --hard (destructive)
  --soft            For rollback: use git reset --soft (default)
  --branch          For rollback: create new branch from checkpoint

Examples:
  $0 list
  $0 show checkpoint-20240130-143022
  $0 rollback checkpoint-20240130-143022 --branch
  $0 diff session-end-session-20240130-150000
EOF
}

# List all checkpoints
function list_checkpoints() {
    echo -e "${BLUE}📋 Available Checkpoints:${NC}"
    echo ""
    
    # List checkpoint tags
    echo -e "${YELLOW}Git Tags:${NC}"
    local tags=$(git tag -l 'checkpoint-*' -l 'session-end-*' -l 'task-*' --sort=-creatordate | head -20)
    if [ -n "$tags" ]; then
        echo "$tags"
    else
        echo "No checkpoint tags found"
    fi
    
    echo ""
    
    # List checkpoint branches
    echo -e "${YELLOW}Checkpoint Branches:${NC}"
    local branches=$(git branch -a | grep "checkpoint/" | sed 's/^[ *]*//')
    if [ -n "$branches" ]; then
        echo "$branches"
    else
        echo "No checkpoint branches found"
    fi
    
    echo ""
    
    # List checkpoint files
    if [ -d "$CHECKPOINT_DIR" ]; then
        echo -e "${YELLOW}Recent Checkpoint Files:${NC}"
        find "$CHECKPOINT_DIR" -name "*.json" -type f -printf "%T@ %p\n" | \
            sort -rn | head -10 | cut -d' ' -f2- | xargs -I {} basename {}
    fi
}

# Show checkpoint details
function show_checkpoint() {
    local checkpoint_id="$1"
    
    echo -e "${BLUE}📍 Checkpoint Details: $checkpoint_id${NC}"
    echo ""
    
    # Check if it's a tag
    if git tag -l "$checkpoint_id" | grep -q "$checkpoint_id"; then
        echo -e "${YELLOW}Type:${NC} Git Tag"
        echo -e "${YELLOW}Commit:${NC} $(git rev-list -n 1 "$checkpoint_id")"
        echo -e "${YELLOW}Date:${NC} $(git log -1 --format=%ai "$checkpoint_id")"
        echo -e "${YELLOW}Message:${NC}"
        git log -1 --format=%B "$checkpoint_id" | sed 's/^/  /'
        echo ""
        echo -e "${YELLOW}Files changed:${NC}"
        git diff-tree --no-commit-id --name-status -r "$checkpoint_id" | sed 's/^/  /'
    # Check if it's a branch
    elif git branch -a | grep -q "$checkpoint_id"; then
        echo -e "${YELLOW}Type:${NC} Git Branch"
        echo -e "${YELLOW}Latest commit:${NC}"
        git log -1 --oneline "$checkpoint_id"
    else
        echo -e "${RED}❌ Checkpoint not found: $checkpoint_id${NC}"
        exit 1
    fi
}

# Rollback to checkpoint
function rollback_checkpoint() {
    local checkpoint_id="$1"
    local mode="$2"
    
    echo -e "${YELLOW}🔄 Rolling back to checkpoint: $checkpoint_id${NC}"
    echo ""
    
    # Verify checkpoint exists
    if ! git tag -l "$checkpoint_id" | grep -q "$checkpoint_id" && \
       ! git branch -a | grep -q "$checkpoint_id"; then
        echo -e "${RED}❌ Checkpoint not found: $checkpoint_id${NC}"
        exit 1
    fi
    
    # Create backup before rollback
    local backup_name="backup-$(date +%Y%m%d-%H%M%S)"
    echo "Creating backup: $backup_name"
    git tag "$backup_name" -m "Backup before rollback to $checkpoint_id"
    
    case "$mode" in
        "--hard")
            echo -e "${RED}⚠️  Performing hard reset (destructive)${NC}"
            git reset --hard "$checkpoint_id"
            echo -e "${GREEN}✅ Rolled back to $checkpoint_id (hard reset)${NC}"
            ;;
        "--branch")
            local branch_name="rollback-$checkpoint_id-$(date +%Y%m%d-%H%M%S)"
            echo "Creating new branch: $branch_name"
            git checkout -b "$branch_name" "$checkpoint_id"
            echo -e "${GREEN}✅ Created branch $branch_name from $checkpoint_id${NC}"
            ;;
        "--stash"|*)
            echo "Stashing current changes..."
            git stash push -m "Stash before rollback to $checkpoint_id"
            git reset --soft "$checkpoint_id"
            echo -e "${GREEN}✅ Rolled back to $checkpoint_id (soft reset)${NC}"
            echo "Your changes are stashed. Use 'git stash pop' to restore them."
            ;;
    esac
}

# Show diff since checkpoint
function diff_checkpoint() {
    local checkpoint_id="$1"
    
    echo -e "${BLUE}📊 Changes since checkpoint: $checkpoint_id${NC}"
    echo ""
    
    if git tag -l "$checkpoint_id" | grep -q "$checkpoint_id"; then
        git diff "$checkpoint_id"
    elif git branch -a | grep -q "$checkpoint_id"; then
        git diff "$checkpoint_id"
    else
        echo -e "${RED}❌ Checkpoint not found: $checkpoint_id${NC}"
        exit 1
    fi
}

# Clean old checkpoints
function clean_checkpoints() {
    local days=${1:-7}
    
    echo -e "${YELLOW}🧹 Cleaning checkpoints older than $days days...${NC}"
    echo ""
    
    # Clean old checkpoint files
    if [ -d "$CHECKPOINT_DIR" ]; then
        find "$CHECKPOINT_DIR" -name "*.json" -type f -mtime +$days -delete
        echo "✅ Cleaned old checkpoint files"
    fi
    
    # List old tags (but don't delete automatically)
    echo ""
    echo "Old checkpoint tags (manual deletion required):"
    git tag -l 'checkpoint-*' --sort=-creatordate | tail -n +50 || echo "No old tags found"
}

# Show session summary
function show_summary() {
    echo -e "${BLUE}📊 Session Summary${NC}"
    echo ""
    
    # Find most recent session summary
    if [ -d "$CHECKPOINT_DIR" ]; then
        local latest_summary=$(find "$CHECKPOINT_DIR" -name "summary-*.md" -type f -printf "%T@ %p\n" | \
            sort -rn | head -1 | cut -d' ' -f2-)
        
        if [ -n "$latest_summary" ]; then
            echo -e "${YELLOW}Latest session summary:${NC}"
            cat "$latest_summary"
        else
            echo "No session summaries found"
        fi
    fi
}

# Main command handling
case "$1" in
    list)
        list_checkpoints
        ;;
    show)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify a checkpoint ID${NC}"
            show_help
            exit 1
        fi
        show_checkpoint "$2"
        ;;
    rollback)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify a checkpoint ID${NC}"
            show_help
            exit 1
        fi
        rollback_checkpoint "$2" "$3"
        ;;
    diff)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Please specify a checkpoint ID${NC}"
            show_help
            exit 1
        fi
        diff_checkpoint "$2"
        ;;
    clean)
        clean_checkpoints "$2"
        ;;
    summary)
        show_summary
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo -e "${RED}Error: Unknown command: $1${NC}"
        echo ""
        show_help
        exit 1
        ;;
esac

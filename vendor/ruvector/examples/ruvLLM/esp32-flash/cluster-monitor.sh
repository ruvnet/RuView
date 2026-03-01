#!/bin/bash
# RuvLLM ESP32 - Cluster Monitor
# Opens serial monitors for all chips in cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

CONFIG_FILE="${1:-cluster.toml}"

echo "╔══════════════════════════════════════════════════════════╗"
echo "║          RuvLLM ESP32 - Cluster Monitor                  ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: $CONFIG_FILE not found"
    exit 1
fi

# Extract ports
PORTS=$(grep 'port = ' "$CONFIG_FILE" | cut -d'"' -f2)
NUM_PORTS=$(echo "$PORTS" | wc -l)

echo "Found $NUM_PORTS chips in cluster"
echo ""

# Check for tmux
if command -v tmux &> /dev/null; then
    echo "Using tmux for multi-pane view..."

    # Create new tmux session
    SESSION="ruvllm-cluster"
    tmux kill-session -t $SESSION 2>/dev/null || true
    tmux new-session -d -s $SESSION

    PANE=0
    for PORT in $PORTS; do
        if [ $PANE -gt 0 ]; then
            tmux split-window -t $SESSION
            tmux select-layout -t $SESSION tiled
        fi

        # Start monitor in pane
        tmux send-keys -t $SESSION.$PANE "echo 'Chip $((PANE+1)): $PORT' && espflash monitor --port $PORT" Enter
        PANE=$((PANE + 1))
    done

    tmux select-layout -t $SESSION tiled
    tmux attach-session -t $SESSION

elif command -v screen &> /dev/null; then
    echo "Using screen (press Ctrl+A then n to switch between chips)..."

    CHIP=1
    for PORT in $PORTS; do
        screen -dmS "chip$CHIP" espflash monitor --port "$PORT"
        echo "Started screen session 'chip$CHIP' for $PORT"
        CHIP=$((CHIP + 1))
    done

    echo ""
    echo "Attach with: screen -r chip1"
    echo "Switch with: Ctrl+A, n"
    echo "Detach with: Ctrl+A, d"

else
    echo "Note: Install tmux or screen for multi-pane monitoring"
    echo ""
    echo "Opening monitors in separate terminals..."

    CHIP=1
    for PORT in $PORTS; do
        if command -v gnome-terminal &> /dev/null; then
            gnome-terminal --title="Chip $CHIP: $PORT" -- espflash monitor --port "$PORT" &
        elif command -v xterm &> /dev/null; then
            xterm -title "Chip $CHIP: $PORT" -e "espflash monitor --port $PORT" &
        elif [[ "$OSTYPE" == "darwin"* ]]; then
            osascript -e "tell app \"Terminal\" to do script \"espflash monitor --port $PORT\""
        else
            echo "Monitor chip $CHIP manually: espflash monitor --port $PORT"
        fi
        CHIP=$((CHIP + 1))
    done
fi

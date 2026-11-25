#!/usr/bin/env bash
# RuView 家用 WiFi MVP 一鍵安裝啟動（macOS 版）。
#
# 流程：檢查 Git → 檢查 Rust（缺則經 rustup 安裝）→ clone 或更新 fork 分支
# feat/home-wifi-rssi-mvp → cargo build --release → 以 --source wifi（或 auto）
# 啟動 sensing-server → 印出本機與 LAN 查看地址並開瀏覽器。
#
# 用法：
#   bash scripts/run-home-wifi-mac.sh [wifi|auto] [http_port]
#   SOURCE=auto HTTP_PORT=8080 bash scripts/run-home-wifi-mac.sh
set -euo pipefail

SOURCE="${1:-${SOURCE:-wifi}}"
HTTP_PORT="${2:-${HTTP_PORT:-8080}}"
REPO_URL="${REPO_URL:-https://github.com/mtlam1028/RuView.git}"
BRANCH="${BRANCH:-feat/home-wifi-rssi-mvp}"
WORK_DIR="${WORK_DIR:-$HOME/RuView}"
BIND_ADDR="${BIND_ADDR:-0.0.0.0}"

if [[ "$SOURCE" != "wifi" && "$SOURCE" != "auto" ]]; then
  echo "錯誤：來源須為 wifi 或 auto（收到：$SOURCE）。" >&2
  exit 1
fi

# ── 1. 檢查 Git ──────────────────────────────────────────────
if ! command -v git >/dev/null 2>&1; then
  echo "[1/5] 未偵測到 Git，嘗試經 Homebrew 安裝…"
  if command -v brew >/dev/null 2>&1; then
    brew install git
  else
    echo "找不到 git。請先安裝 Xcode Command Line Tools：" >&2
    echo "  xcode-select --install" >&2
    exit 1
  fi
fi
git --version
echo "[1/5] Git 就緒。"

# ── 2. 檢查 Rust（缺則安裝）──────────────────────────────────
if ! command -v cargo >/dev/null 2>&1; then
  echo "[2/5] 未偵測到 Rust，正在經 rustup 安裝…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi
cargo --version
echo "[2/5] Rust 就緒。"

# ── 3. clone 或更新分支 ──────────────────────────────────────
if [[ -d "$WORK_DIR/.git" ]]; then
  echo "[3/5] 更新既有工作區 $WORK_DIR …"
  git -C "$WORK_DIR" fetch origin
  git -C "$WORK_DIR" checkout "$BRANCH"
  git -C "$WORK_DIR" pull --ff-only origin "$BRANCH"
else
  echo "[3/5] 複製 $REPO_URL（$BRANCH）至 $WORK_DIR …"
  git clone -b "$BRANCH" "$REPO_URL" "$WORK_DIR"
fi
echo "[3/5] 原始碼就緒。"

# ── 4. cargo build --release ─────────────────────────────────
echo "[4/5] 編譯 sensing-server（release，需時數分鐘）…"
(
  cd "$WORK_DIR/v2"
  cargo build --release -p wifi-densepose-sensing-server
)
SERVER_BIN="$WORK_DIR/v2/target/release/sensing-server"
[[ -x "$SERVER_BIN" ]] || { echo "錯誤：編譯完成但找不到 $SERVER_BIN。" >&2; exit 1; }
echo "[4/5] 編譯完成。"

# ── 5. 啟動 server＋印出地址 ────────────────────────────────
echo "[5/5] 啟動 server（--source $SOURCE，port $HTTP_PORT）…"
"$SERVER_BIN" --source "$SOURCE" --http-port "$HTTP_PORT" --bind-addr "$BIND_ADDR" &
SERVER_PID=$!
sleep 3
if ! kill -0 "$SERVER_PID" 2>/dev/null; then
  echo "錯誤：server 啟動後即退出，請檢查上方輸出。" >&2
  exit 1
fi

LOCAL_URL="http://localhost:${HTTP_PORT}/ui/observatory.html"
echo ""
echo "=== RuView 家用 WiFi MVP 已啟動（PID $SERVER_PID）==="
echo "本機查看：$LOCAL_URL"
# LAN 地址（macOS：en0/en1 等介面之 IPv4）
LAN_IPS="$(ipconfig getifaddr en0 2>/dev/null || true) $(ipconfig getifaddr en1 2>/dev/null || true)"
for ip in $LAN_IPS; do
  echo "區域網路查看：http://${ip}:${HTTP_PORT}/ui/observatory.html"
done
echo "來源：--source $SOURCE（wifi = 本機 WiFi，RSSI-only）"
echo "停止：kill $SERVER_PID"
echo ""
open "$LOCAL_URL" || true
wait "$SERVER_PID"

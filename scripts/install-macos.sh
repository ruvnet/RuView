#!/usr/bin/env bash
# RuView Desktop macOS installer
# Usage: curl -fsSL https://raw.githubusercontent.com/ruvnet/RuView/main/scripts/install-macos.sh | bash

set -euo pipefail

VERSION="${RUVIEW_VERSION:-0.4.5}"
REPO="ruvnet/RuView"

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  arm64|aarch64) ARCH_LABEL="arm64" ;;
  x86_64)        ARCH_LABEL="x64" ;;
  *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

DMG_NAME="RuView-Desktop-${VERSION}-macos-${ARCH_LABEL}.dmg"
ZIP_NAME="RuView-Desktop-${VERSION}-macos-${ARCH_LABEL}.zip"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/desktop-v${VERSION}"

echo "RuView Desktop Installer"
echo "========================"
echo "Version:  ${VERSION}"
echo "Arch:     ${ARCH_LABEL}"
echo ""

# Try DMG first, fall back to ZIP
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

echo "Downloading..."
if curl -fsSL -o "${TMPDIR}/${DMG_NAME}" "${DOWNLOAD_URL}/${DMG_NAME}" 2>/dev/null; then
  echo "Installing from DMG..."
  MOUNT_POINT=$(hdiutil attach "${TMPDIR}/${DMG_NAME}" -nobrowse -noautoopen | tail -1 | awk '{print $NF}')

  # Remove old version
  if [ -d "/Applications/RuView Desktop.app" ]; then
    echo "Removing previous installation..."
    rm -rf "/Applications/RuView Desktop.app"
  fi

  cp -r "${MOUNT_POINT}/RuView Desktop.app" /Applications/
  hdiutil detach "$MOUNT_POINT" -quiet

elif curl -fsSL -o "${TMPDIR}/${ZIP_NAME}" "${DOWNLOAD_URL}/${ZIP_NAME}" 2>/dev/null; then
  echo "Installing from ZIP..."
  cd "$TMPDIR"
  unzip -q "$ZIP_NAME"

  if [ -d "/Applications/RuView Desktop.app" ]; then
    echo "Removing previous installation..."
    rm -rf "/Applications/RuView Desktop.app"
  fi

  cp -r "RuView Desktop.app" /Applications/

else
  echo "Failed to download RuView Desktop v${VERSION}"
  echo "Check https://github.com/${REPO}/releases for available versions"
  exit 1
fi

# Clear Gatekeeper quarantine attribute
xattr -cr "/Applications/RuView Desktop.app" 2>/dev/null || true

echo ""
echo "RuView Desktop v${VERSION} installed to /Applications"
echo ""
echo "To launch: open '/Applications/RuView Desktop.app'"
echo ""
echo "ESP32 serial drivers:"
echo "  CP210x:  https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers"
echo "  CH340:   https://github.com/WCHSoftGroup/ch34xser_macos"
echo "  FTDI:    https://ftdichip.com/drivers/vcp-drivers/"

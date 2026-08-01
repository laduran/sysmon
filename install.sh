#!/usr/bin/env bash
# Installs system-monitor for the current user (no root required):
# builds the release binary and places it, the .desktop entry, and the
# app icon under the standard per-user XDG directories so it shows up
# with its own icon in application launchers and file managers.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

APP_ID="io.github.laduran.sysmon"
BIN_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"

echo "Building release binary..."
cargo build --release

install -Dm755 target/release/system-monitor "$BIN_DIR/system-monitor"
install -Dm644 "data/${APP_ID}.desktop" "$APPS_DIR/${APP_ID}.desktop"
install -Dm644 "data/icons/hicolor/scalable/apps/${APP_ID}.svg" "$ICON_DIR/${APP_ID}.svg"

if ! command -v system-monitor >/dev/null && [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "Note: $BIN_DIR is not on your PATH. Add it in your shell profile," \
         "e.g. export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

command -v update-desktop-database >/dev/null && update-desktop-database "$APPS_DIR" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" || true

echo "Installed. Launch from your application menu, or run: system-monitor"

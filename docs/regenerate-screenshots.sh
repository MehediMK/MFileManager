#!/bin/bash
# Regenerates the documentation screenshots using headless Chrome.
# The page under docs/screenshot-src uses a mocked Tauri IPC so the real
# frontend (styles.css + script.js) renders with representative sample data.
set -e
SRC="$(cd "$(dirname "$0")/screenshot-src" && pwd)"
DST="$(cd "$(dirname "$0")/screenshots" && pwd)"
CHROME="${CHROME:-google-chrome}"

shot() {
    local out="$1" query="$2"
    "$CHROME" --headless=new --disable-gpu --no-sandbox --hide-scrollbars \
        --force-device-scale-factor=2 --window-size=600,400 \
        --virtual-time-budget=4000 \
        --screenshot="$out" "file://$SRC/index.screenshot.html?$query" >/dev/null 2>&1
}

shot "$DST/main-view.png" "shot=main"
shot "$DST/properties-dialog.png" "shot=properties"
shot "$DST/disk-usage.png" "shot=disk"
echo "Screenshots written to $DST"
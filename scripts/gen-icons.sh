#!/bin/sh
# Regenerate the app icon set (PNGs, .icns, .ico) from assets/icon.svg.
# `tauri icon` also emits mobile/Store assets the bundle config never uses —
# prune them so src-tauri/icons stays minimal.

set -eu

cd "$(dirname "$0")/.."

bun tauri icon assets/icon.svg

cd src-tauri/icons
rm -rf android ios
for f in *; do
  case "$f" in
    .gitkeep|32x32.png|128x128.png|128x128@2x.png|icon.icns|icon.ico|icon.png) ;;
    *) rm -rf "$f" ;;
  esac
done

echo "icons regenerated in src-tauri/icons"

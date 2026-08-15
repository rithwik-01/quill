#!/bin/sh
# Sign the `tauri dev` binary with a stable identity.
#
# macOS TCC keys the Accessibility grant to the code signature. An unsigned or
# ad-hoc-signed binary gets a new identity on every rebuild, so the grant stops
# matching and System Settings is left showing a dead toggle. Signing with a
# real certificate keeps the identity stable, so you grant once and it sticks.
#
# `tauri build` reads bundle.macOS.signingIdentity from tauri.conf.json (add
# yours locally — don't commit it). `tauri dev` runs the bare target/debug/quill
# binary, which the bundler never touches — hence this script.
#
# ponytail: Tauri has no post-build hook for `dev`, so this is manual. Re-run it
# after any Rust change before testing permissions. Frontend-only changes hot
# reload without rebuilding the binary, so the signature survives most of the
# dev loop. For anything permission-critical, test `bun tauri build --debug`
# output instead — that's the real .app and its own TCC entry.

set -eu

# Set your own dev certificate via the environment, e.g. in your shell profile:
#   export APPLE_SIGNING_IDENTITY="Apple Development: you@example.com (TEAMID)"
IDENTITY="${APPLE_SIGNING_IDENTITY:?set APPLE_SIGNING_IDENTITY to your Apple Development identity}"
BINARY="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/target/debug/quill"

if [ ! -f "$BINARY" ]; then
  echo "no dev binary at $BINARY — run 'bun tauri dev' once first" >&2
  exit 1
fi

codesign --force --sign "$IDENTITY" "$BINARY"
codesign -dv --verbose=4 "$BINARY" 2>&1 | grep -E '^Authority|^Identifier'

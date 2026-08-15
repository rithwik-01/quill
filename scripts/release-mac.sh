#!/bin/sh
# Build the distributable macOS .dmg (universal: Apple Silicon + Intel).
#
# Signing is driven entirely by the environment. Set all four to get a real,
# notarized build; leave them unset for an ad-hoc build you can only run
# locally.
#
#   export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
#   export APPLE_ID="you@example.com"
#   export APPLE_PASSWORD="abcd-efgh-ijkl-mnop"   # app-specific password
#   export APPLE_TEAM_ID="TEAMID"
#
# APPLE_SIGNING_IDENTITY overrides bundle.macOS.signingIdentity ("-") in
# tauri.conf.json. When the notarization trio is present the Tauri bundler
# submits the .app to Apple's notary service and staples the ticket before
# packing the .dmg.
#
# A "Developer ID Application" cert requires a paid Apple Developer Program
# membership. An "Apple Development" cert is NOT a substitute — it cannot be
# notarized, so Gatekeeper rejects it on every Mac but this one.

set -eu

cd "$(dirname "$0")/.."

# Local credentials live in the gitignored .env (never commit it).
if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "warning: APPLE_SIGNING_IDENTITY unset — building ad-hoc signed." >&2
  echo "         Not distributable: Gatekeeper blocks it on other Macs." >&2
elif [ -z "${APPLE_ID:-}" ] || [ -z "${APPLE_PASSWORD:-}" ] || [ -z "${APPLE_TEAM_ID:-}" ]; then
  echo "warning: signing but not notarizing (APPLE_ID/APPLE_PASSWORD/APPLE_TEAM_ID incomplete)." >&2
  echo "         Gatekeeper blocks unnotarized apps even when signed." >&2
fi

rustup target add aarch64-apple-darwin x86_64-apple-darwin
bun tauri build --target universal-apple-darwin

APP="src-tauri/target/universal-apple-darwin/release/bundle/macos/Quill.app"
DMG="$(ls src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg)"

# The check that matters: what Gatekeeper will decide on someone else's Mac.
# `spctl -a` fails on an ad-hoc or dev-signed build — that failure is the point.
echo "--- signature ---"
codesign -dv --verbose=2 "$APP" 2>&1 | grep -E '^Authority|^TeamIdentifier|^Identifier'
echo "--- gatekeeper ---"
spctl -a -vvv -t install "$APP" || echo "REJECTED — not distributable (see warnings above)"
echo "--- notarization ticket ---"
xcrun stapler validate "$APP" || echo "no stapled ticket"

echo
echo "dmg: $DMG"

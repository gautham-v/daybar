#!/usr/bin/env bash
# Build a release binary and assemble target/Daybar.app around it.
#
# The .app is not cosmetic: EventKit will not grant calendar access to a bare
# binary — it needs a bundle identifier and the usage-description keys below.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Honour CARGO_TARGET_DIR / .cargo/config.toml rather than assuming ./target.
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT/Cargo.toml" \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
TARGET_DIR="${TARGET_DIR:-$ROOT/target}"

APP="$TARGET_DIR/Daybar.app"
# DAYBAR_BIN points at a prebuilt binary (the release workflow passes the
# universal one); otherwise build for this machine.
BIN="${DAYBAR_BIN:-$TARGET_DIR/release/daybar}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"

if [ -z "${DAYBAR_BIN:-}" ]; then
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/daybar"
cp "$ROOT/assets/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>Daybar</string>
	<key>CFBundleDisplayName</key>
	<string>Daybar</string>
	<key>CFBundleExecutable</key>
	<string>daybar</string>
	<key>CFBundleIdentifier</key>
	<string>com.gauthamv.daybar</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>${VERSION}</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>LSMinimumSystemVersion</key>
	<string>13.0</string>
	<key>LSUIElement</key>
	<true/>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSCalendarsFullAccessUsageDescription</key>
	<string>Daybar shows your calendar events in the menu bar.</string>
	<key>NSCalendarsUsageDescription</key>
	<string>Daybar shows your calendar events in the menu bar.</string>
</dict>
</plist>
PLIST

# Sign with a stable identity when the machine has one. Ad-hoc signatures get a
# fresh code identity on every rebuild, which makes macOS treat each build as a
# different app: TCC re-prompts for calendar access every single launch.
# Override with CODESIGN_IDENTITY.
IDENTITY="${CODESIGN_IDENTITY:-}"
if [ -z "$IDENTITY" ]; then
  # Prefer Developer ID, which Gatekeeper trusts, over an Xcode development
  # certificate, which it does not.
  IDENTITIES="$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(.*\)"/\1/p')"
  IDENTITY="$(printf '%s\n' "$IDENTITIES" | grep -m1 '^Developer ID Application' || printf '%s\n' "$IDENTITIES" | head -n 1)"
fi
if [ -n "$IDENTITY" ]; then
  # Hardened runtime blocks EventKit unless the calendars entitlement is present.
  codesign --force --options runtime \
    --entitlements "$(dirname "$0")/entitlements.plist" \
    --sign "$IDENTITY" "$APP" \
    || echo "warning: codesign with '$IDENTITY' failed; calendar permission may not stick"
else
  echo "note: no codesigning identity found; signing ad-hoc, so macOS will re-prompt on every rebuild"
  codesign --force --sign - "$APP" 2>/dev/null \
    || echo "warning: ad-hoc codesign failed; calendar permission may not stick"
fi

echo "built $APP"

# When CARGO_TARGET_DIR points elsewhere, keep the documented ./target/Daybar.app
# path working via a symlink.
if [ "$TARGET_DIR" != "$ROOT/target" ]; then
  # A copy, not a symlink: Launch Services refuses to `open` a symlinked .app.
  mkdir -p "$ROOT/target"
  rm -rf "$ROOT/target/Daybar.app"
  cp -R "$APP" "$ROOT/target/Daybar.app"
  echo "copied to $ROOT/target/Daybar.app"
fi

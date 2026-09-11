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
BIN="$TARGET_DIR/release/daybar"

cargo build --release --manifest-path "$ROOT/Cargo.toml"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/daybar"

cat > "$APP/Contents/Info.plist" <<'PLIST'
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
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>0.1.0</string>
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
# different app: TCC re-prompts for calendar access (and mailbar re-prompts for
# its Keychain item) every single launch. Override with CODESIGN_IDENTITY.
IDENTITY="${CODESIGN_IDENTITY:-}"
if [ -z "$IDENTITY" ]; then
  IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null \
    | sed -n 's/.*"\(.*\)"/\1/p' | head -n 1)"
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

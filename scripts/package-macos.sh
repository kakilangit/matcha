#!/bin/sh
# Builds dist/Matcha.app and dist/Matcha-<version>-<arch>.dmg. Run on macOS from the
# repository root (usually via `make dmg`).
#
#   UNIVERSAL=1            build one binary for Apple silicon and Intel (needs both
#                          rustup targets: aarch64-apple-darwin, x86_64-apple-darwin)
#   MACOS_SIGN_IDENTITY    "Developer ID Application: ..." to sign for distribution;
#                          without it the app is signed ad hoc (runs locally only)
#   SKIP_DMG=1             stop after building the .app
set -eu

if [ "$(uname -s)" != Darwin ]; then
  echo "package-macos.sh must run on macOS" >&2
  exit 1
fi

app_id=dev.kakilangit.matcha
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
target_dir=${CARGO_TARGET_DIR:-target}
dist=dist
app="$dist/Matcha.app"

if [ "${UNIVERSAL:-0}" = 1 ]; then
  arch=universal
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release --target "$target"
  done
  binary="$target_dir/matcha-universal"
  lipo -create -output "$binary" \
    "$target_dir/aarch64-apple-darwin/release/matcha" \
    "$target_dir/x86_64-apple-darwin/release/matcha"
else
  arch=$(uname -m)
  cargo build --release
  binary="$target_dir/release/matcha"
fi

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/matcha"
cp assets/icons/matcha.icns "$app/Contents/Resources/matcha.icns"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Matcha</string>
  <key>CFBundleDisplayName</key><string>Matcha</string>
  <key>CFBundleIdentifier</key><string>$app_id</string>
  <key>CFBundleExecutable</key><string>matcha</string>
  <key>CFBundleIconFile</key><string>matcha</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>10.15.7</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
EOF
plutil -lint "$app/Contents/Info.plist"

if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$app"
else
  codesign --force --sign - "$app"
fi
codesign --verify --strict "$app"
echo "Built $app"

if [ "${SKIP_DMG:-0}" = 1 ]; then
  exit 0
fi

dmg="$dist/Matcha-$version-$arch.dmg"
staging="$target_dir/dmg"
rm -rf "$staging" "$dmg"
mkdir -p "$staging"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
# `diskutil image` replaces the deprecated `hdiutil create` on newer macOS;
# older releases only have hdiutil.
if diskutil image create from --help >/dev/null 2>&1; then
  diskutil image create from --format UDZO --volumeName Matcha "$staging" "$dmg"
else
  hdiutil create -volname Matcha -srcfolder "$staging" -format UDZO "$dmg"
fi
rm -rf "$staging"
echo "Built $dmg"

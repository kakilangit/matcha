#!/bin/sh
# Builds dist/Matcha-<version>-<arch>.AppImage. Run on Linux from the repository root
# (usually via `make appimage`).
#
#   APPIMAGETOOL          path to an existing appimagetool; otherwise the pinned
#                         release below is downloaded into the target directory
#   APPIMAGETOOL_VERSION  appimagetool release to download (default 1.9.1)
#
# The binary links against the build machine's glibc, so build on the oldest
# distribution you want to support.
set -eu

if [ "$(uname -s)" != Linux ]; then
  echo "package-appimage.sh must run on Linux" >&2
  exit 1
fi

app_id=dev.kakilangit.matcha
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
arch=$(uname -m)
target_dir=${CARGO_TARGET_DIR:-target}
appdir="$target_dir/appimage/Matcha.AppDir"
dist=dist
output="$dist/Matcha-$version-$arch.AppImage"

# appimagetool needs `file` to inspect the binary.
if ! command -v file >/dev/null; then
  echo "package-appimage.sh needs the 'file' command (e.g. apt install file)" >&2
  exit 1
fi

cargo build --release

tool=${APPIMAGETOOL:-}
if [ -z "$tool" ]; then
  tool_version=${APPIMAGETOOL_VERSION:-1.9.1}
  tool="$target_dir/appimage/appimagetool-$tool_version-$arch.AppImage"
  if [ ! -x "$tool" ]; then
    mkdir -p "$(dirname "$tool")"
    curl --fail --location --silent --show-error --output "$tool.part" \
      "https://github.com/AppImage/appimagetool/releases/download/$tool_version/appimagetool-$arch.AppImage"
    chmod +x "$tool.part"
    mv "$tool.part" "$tool"
  fi
fi

rm -rf "$appdir"
install -Dm755 "$target_dir/release/matcha" "$appdir/usr/bin/matcha"
install -Dm644 "assets/$app_id.desktop" "$appdir/usr/share/applications/$app_id.desktop"
for size in 16 32 48 64 128 256 512; do
  install -Dm644 "assets/icons/matcha-$size.png" \
    "$appdir/usr/share/icons/hicolor/${size}x${size}/apps/$app_id.png"
done
# appimagetool expects the desktop entry and icon at the AppDir root too.
cp "assets/$app_id.desktop" "$appdir/$app_id.desktop"
cp assets/icons/matcha-256.png "$appdir/$app_id.png"
ln -s "$app_id.png" "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/matcha" "$@"
EOF
chmod +x "$appdir/AppRun"

mkdir -p "$dist"
# Extract-and-run avoids needing FUSE on the build machine.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$arch" "$tool" --no-appstream "$appdir" "$output"
echo "Built $output"

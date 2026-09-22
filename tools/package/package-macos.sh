#!/usr/bin/env bash
# Build the macOS package: T.O.R.E-Fighters.app inside a compressed DMG.
#
#   cargo build --release --locked -p tore-app -p tore-extract
#   tools/package/package-macos.sh [--version 0.2.0]
#
# The DMG lands in dist/ and the staged .app stays in dist/stage/macos/ so the
# asset guard can scan real files. The build is unsigned and not notarized: the
# first launch is a right-click, Open, then Open again in the dialog.
# Requires macOS: hdiutil, sips and iconutil are Apple tools.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
here="$root/tools/package"
# shellcheck source=tools/package/version.sh
. "$here/version.sh"

explicit=""
while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            explicit="${2:-}"
            shift 2
            ;;
        --version=*)
            explicit="${1#--version=}"
            shift
            ;;
        -h|--help)
            sed -n '2,12p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

if [ "$(uname -s)" != "Darwin" ]; then
    echo "package-macos.sh needs macOS: hdiutil and iconutil are Apple tools." >&2
    exit 1
fi

version=$(tore_resolve_version "$explicit")
arch=$(uname -m)
target="$root/target/release"
dist="$root/dist"
stage="$dist/stage/macos"
app="$stage/T.O.R.E-Fighters.app"
name="T.O.R.E-Fighters-$version-macos-$arch"

for binary in tore-app tore-extract; do
    if [ ! -x "$target/$binary" ]; then
        echo "Missing $target/$binary. Run: cargo build --release --locked -p tore-app -p tore-extract" >&2
        exit 1
    fi
done

echo "Staging $app"
rm -rf "$stage"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$dist"

install -m 755 "$target/tore-app" "$app/Contents/MacOS/tore-app"
# The extractor is a developer tool. It rides along so a player who wants a
# terminal has one, but nothing in the bundle launches it.
install -m 755 "$target/tore-extract" "$app/Contents/MacOS/tore-extract"
install -m 644 "$root/LICENSE" "$app/Contents/Resources/LICENSE"
install -m 644 "$root/THIRD_PARTY_NOTICES.md" "$app/Contents/Resources/THIRD_PARTY_NOTICES.md"
install -m 644 "$root/README.md" "$app/Contents/Resources/README.md"
printf 'APPL????' > "$app/Contents/PkgInfo"
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"

# The icon geometry is scale free, so each iconset size is drawn rather than
# resampled. iconutil then packs them into tore.icns.
iconset="$stage/tore.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    python3 "$here/make_icon.py" --size "$size" "$iconset/icon_${size}x${size}.png"
    python3 "$here/make_icon.py" --size "$((size * 2))" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil --convert icns "$iconset" --output "$app/Contents/Resources/tore.icns"
rm -rf "$iconset"

echo "Checking the staged bundle for retail data"
python3 "$root/tools/check_assets.py" "$app"

dmgroot="$stage/dmg"
rm -rf "$dmgroot"
mkdir -p "$dmgroot"
cp -R "$app" "$dmgroot/T.O.R.E-Fighters.app"
ln -s /Applications "$dmgroot/Applications"
cp "$root/README.md" "$dmgroot/README.md"

dmg="$dist/$name.dmg"
rm -f "$dmg"
hdiutil create -volname "T.O.R.E-Fighters" -srcfolder "$dmgroot" -ov -format UDZO "$dmg"
echo "Wrote $dmg"
python3 "$root/tools/check_assets.py" "$dmg"

echo "macOS packaging complete for version $version"

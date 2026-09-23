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
icons="$root/crates/tore-app/assets/icon"
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

# iconutil packs a .iconset folder into tore.icns. The sizes come straight
# from the committed PNG set, so nothing is resampled here and the macOS icon
# is the same artwork Windows and Linux get. See tools/package/build_icons.py.
#
# An @2x entry is just the PNG of twice the nominal size. icon_512x512@2x
# would need a 1024 px PNG, which is not committed: the source render is
# photographic and a lossless 1024 px PNG costs about 2 MB. macOS scales the
# 512 px entry for that one case, which is what an upscaled file would have
# given it anyway.
iconset="$stage/tore.iconset"
mkdir -p "$iconset"

# Copy one committed PNG into the .iconset under the name iconutil expects.
# $1 is the nominal point size, $2 is the pixel size, $3 is "" or "@2x".
stage_iconset_entry() {
    local source_png="$icons/tore-$2.png"
    if [ ! -f "$source_png" ]; then
        echo "Missing $source_png. Run: python3 tools/package/build_icons.py" >&2
        exit 1
    fi
    install -m 644 "$source_png" "$iconset/icon_$1x$1$3.png"
}

stage_iconset_entry 16 16 ""
stage_iconset_entry 16 32 "@2x"
stage_iconset_entry 32 32 ""
stage_iconset_entry 32 64 "@2x"
stage_iconset_entry 128 128 ""
stage_iconset_entry 128 256 "@2x"
stage_iconset_entry 256 256 ""
stage_iconset_entry 256 512 "@2x"
stage_iconset_entry 512 512 ""
iconutil --convert icns "$iconset" --output "$app/Contents/Resources/tore.icns"
rm -rf "$iconset"

echo "Checking the staged bundle for retail data"
python3 "$root/tools/check_assets.py" "$app"
python3 "$root/tools/check_runtime_dependencies.py" "$app/Contents/MacOS/tore-app" "$app/Contents/MacOS/tore-extract"
python3 "$root/tools/check_startup_diagnostics.py" "$app/Contents/MacOS/tore-app"

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
mountpoint="$stage/dmg check with spaces"
mkdir -p "$mountpoint"
hdiutil attach -readonly -nobrowse -mountpoint "$mountpoint" "$dmg"
trap 'hdiutil detach "$mountpoint"' EXIT
python3 "$root/tools/check_startup_diagnostics.py" "$mountpoint/T.O.R.E-Fighters.app/Contents/MacOS/tore-app"
hdiutil detach "$mountpoint"
trap - EXIT

echo "macOS packaging complete for version $version"

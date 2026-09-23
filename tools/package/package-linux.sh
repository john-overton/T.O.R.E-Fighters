#!/usr/bin/env bash
# Build the Linux packages: a .tar.gz always, and an AppImage when
# appimagetool is available or can be downloaded.
#
#   cargo build --release --locked -p tore-app -p tore-extract
#   tools/package/package-linux.sh [--version 0.2.0]
#
# Packages land in dist/. The staged directory the tar.gz is built from stays
# in dist/stage/ so the asset guard can scan real files rather than a
# compressed blob. Nothing here is signed and nothing here contains retail
# media: the app imports the player's own copy at runtime.
#
# Locally, a missing appimagetool is a warning and the script still succeeds
# with the tar.gz. In CI (CI=true) a missing AppImage is a failure, because the
# release is expected to carry one.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
here="$root/tools/package"
icons="$root/crates/tore-app/assets/icon"
# shellcheck source=tools/package/version.sh
. "$here/version.sh"

# appimagetool is published only under the moving "continuous" tag, so the
# download is pinned by content hash. Recorded 2026-09-22 by downloading the
# asset and running sha256sum. When upstream rebuilds it this script fails
# loudly rather than running an unreviewed binary; re-pin deliberately.
APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
APPIMAGETOOL_SHA256="a6d71e2b6cd66f8e8d16c37ad164658985e0cf5fcaa950c90a482890cb9d13e0"

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
            sed -n '2,20p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

version=$(tore_resolve_version "$explicit")
target="$root/target/release"
dist="$root/dist"
name="T.O.R.E-Fighters-$version-linux-x86_64"
stage="$dist/stage/linux/$name"
appdir="$dist/stage/linux/AppDir"

for binary in tore-app tore-extract; do
    if [ ! -x "$target/$binary" ]; then
        echo "Missing $target/$binary. Run: cargo build --release --locked -p tore-app -p tore-extract" >&2
        exit 1
    fi
done

echo "Staging $name"
rm -rf "$stage" "$appdir"
mkdir -p "$stage" "$dist"
install -m 755 "$target/tore-app" "$stage/tore-app"
install -m 755 "$target/tore-extract" "$stage/tore-extract"
install -m 644 "$root/LICENSE" "$stage/LICENSE"
install -m 644 "$root/THIRD_PARTY_NOTICES.md" "$stage/THIRD_PARTY_NOTICES.md"
install -m 644 "$root/README.md" "$stage/README.md"
install -m 644 "$here/tore-fighters.desktop" "$stage/tore-fighters.desktop"
# The desktop entry says Icon=tore-fighters, so the file is named to match.
install -m 644 "$icons/tore-256.png" "$stage/tore-fighters.png"

echo "Checking the staged directory for retail data"
python3 "$root/tools/check_assets.py" "$stage"
python3 "$root/tools/check_runtime_dependencies.py" "$stage/tore-app" "$stage/tore-extract"
python3 "$root/tools/check_startup_diagnostics.py" "$stage/tore-app"

tarball="$dist/$name.tar.gz"
rm -f "$tarball"
tar -czf "$tarball" -C "$(dirname "$stage")" "$name"
echo "Wrote $tarball"
python3 "$root/tools/check_assets.py" "$tarball"
unpacked="$dist/stage/linux/package check with spaces"
rm -rf "$unpacked"
mkdir -p "$unpacked"
tar -xzf "$tarball" -C "$unpacked"
python3 "$root/tools/check_startup_diagnostics.py" "$unpacked/$name/tore-app"

# The AppImage carries the game only. tore-extract is a developer tool and
# stays in the tar.gz.
echo "Staging the AppDir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/256x256/apps"
install -m 755 "$target/tore-app" "$appdir/usr/bin/tore-app"
install -m 644 "$root/LICENSE" "$appdir/usr/share/LICENSE"
install -m 644 "$root/THIRD_PARTY_NOTICES.md" "$appdir/usr/share/THIRD_PARTY_NOTICES.md"
install -m 644 "$here/tore-fighters.desktop" "$appdir/tore-fighters.desktop"
install -m 644 "$here/tore-fighters.desktop" "$appdir/usr/share/applications/tore-fighters.desktop"
# appimagetool wants the icon at the AppDir root under the desktop entry's
# Icon= name, and desktops that unpack the AppImage read the hicolor copy.
install -m 644 "$icons/tore-256.png" "$appdir/tore-fighters.png"
install -m 644 "$icons/tore-256.png" "$appdir/usr/share/icons/hicolor/256x256/apps/tore-fighters.png"
cat > "$appdir/AppRun" <<'APPRUN'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/tore-app" "$@"
APPRUN
chmod 755 "$appdir/AppRun"

python3 "$root/tools/check_assets.py" "$appdir"

appimagetool=""
if command -v appimagetool >/dev/null 2>&1; then
    appimagetool=$(command -v appimagetool)
elif command -v curl >/dev/null 2>&1; then
    cache="$dist/stage/linux/appimagetool-x86_64.AppImage"
    if [ ! -f "$cache" ]; then
        echo "Downloading appimagetool"
        if ! curl -sSL --fail -o "$cache.part" "$APPIMAGETOOL_URL"; then
            rm -f "$cache.part"
            echo "Could not download appimagetool." >&2
        else
            mv "$cache.part" "$cache"
        fi
    fi
    if [ -f "$cache" ]; then
        actual=$(sha256sum "$cache" | cut -d' ' -f1)
        if [ "$actual" != "$APPIMAGETOOL_SHA256" ]; then
            echo "appimagetool hash mismatch: expected $APPIMAGETOOL_SHA256, got $actual." >&2
            echo "Upstream rebuilt the continuous release. Review it and update the pin." >&2
            rm -f "$cache"
        else
            chmod 755 "$cache"
            appimagetool="$cache"
        fi
    fi
fi

appimage="$dist/$name.AppImage"
if [ -n "$appimagetool" ]; then
    rm -f "$appimage"
    # appimagetool is itself an AppImage and needs FUSE. Where FUSE is absent,
    # extract it once and run the extracted entry point instead.
    if ! ARCH=x86_64 "$appimagetool" "$appdir" "$appimage" 2>"$dist/stage/linux/appimagetool.log"; then
        extracted="$dist/stage/linux/squashfs-root"
        if [ ! -d "$extracted" ]; then
            (cd "$dist/stage/linux" && "$appimagetool" --appimage-extract >/dev/null 2>&1) || true
        fi
        if [ -x "$extracted/AppRun" ]; then
            ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$extracted/AppRun" "$appdir" "$appimage" \
                2>>"$dist/stage/linux/appimagetool.log" || true
        fi
    fi
fi

if [ -f "$appimage" ]; then
    chmod 755 "$appimage"
    echo "Wrote $appimage"
    python3 "$root/tools/check_assets.py" "$appimage"
    recovered="$dist/stage/linux/appimage check with spaces"
    rm -rf "$recovered"
    mkdir -p "$recovered"
    (cd "$recovered" && "$appimage" --appimage-extract >/dev/null)
    python3 "$root/tools/check_startup_diagnostics.py" "$recovered/squashfs-root/AppRun"
else
    echo "No AppImage was produced. See $dist/stage/linux/appimagetool.log if present." >&2
    if [ "${CI:-}" = "true" ]; then
        echo "CI requires the AppImage." >&2
        exit 1
    fi
    echo "Continuing with the tar.gz only; install appimagetool to build one locally." >&2
fi

echo "Linux packaging complete for version $version"

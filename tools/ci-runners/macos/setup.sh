#!/usr/bin/env bash
# Installs a GitHub Actions runner on an Apple Silicon Mac for one of the
# macOS jobs (docs/CI-RUNNERS.md):
#
#   tools/ci-runners/macos/setup.sh arm64  <registration token>   # macos-14 jobs
#   tools/ci-runners/macos/setup.sh x86_64 <registration token>   # macos-15-intel jobs
#
# arm64 runs natively. x86_64 is the Intel runner under Rosetta: its runner
# is the x64 build, so every step it starts runs as x86_64 (`uname -m` says
# x86_64, and package-macos.sh names the DMG for it), with its own x86_64
# rustup, so nothing changes in the workflows. Each runner keeps its rustup,
# cargo home and work folder under ~/tore-ci/<arch>, apart from the user's
# own ~/.cargo, and starts at login as a LaunchAgent. Safe to run again.
set -euo pipefail

arch="${1:?arch: arm64 or x86_64}"
token="${2:?registration token}"
url="${TORE_RUNNER_URL:-https://github.com/john-overton/T.O.R.E-Fighters}"
version="${TORE_RUNNER_VERSION:-2.338.1}"
toolchain="${TORE_TOOLCHAIN:-1.91.1}"
python_version="${TORE_PYTHON:-3.14.9}"
python_tag="${TORE_PYTHON_BUILD:-20261021}"

case "$arch" in
    arm64) runner_arch=arm64; rust_host=aarch64-apple-darwin; label=tore-macos-14; run=(env) ;;
    x86_64) runner_arch=x64; rust_host=x86_64-apple-darwin; label=tore-macos-15-intel; run=(arch -x86_64) ;;
    *) echo "arch must be arm64 or x86_64" >&2; exit 2 ;;
esac
if [ "$arch" = x86_64 ] && ! /usr/bin/pgrep -q oahd; then
    softwareupdate --install-rosetta --agree-to-license
fi

base="$HOME/tore-ci/$arch"
mkdir -p "$base/runner" "$base/work"
export CARGO_HOME="$base/cargo" RUSTUP_HOME="$base/rustup"
if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
    curl -fsSL -o "$base/rustup-init" "https://static.rust-lang.org/rustup/dist/$rust_host/rustup-init"
    chmod +x "$base/rustup-init"
    "${run[@]}" "$base/rustup-init" -y --profile minimal --default-toolchain none --no-modify-path
fi
"${run[@]}" "$CARGO_HOME/bin/rustup" toolchain install "$toolchain" --profile minimal --component rustfmt --component clippy

# Python for the workflows' tools. setup-python's macOS builds install a
# system package with sudo on every run, so the workflows skip that step on
# a self-hosted Mac and use this one: a standalone CPython for the runner's
# own architecture (python-build-standalone, as uv uses), so
# platform.machine() names the server archive correctly. `python` and
# `python3` in $base/bin lead the runner's PATH.
if [ ! -x "$base/python/bin/python3" ]; then
    curl -fsSL "https://github.com/astral-sh/python-build-standalone/releases/download/$python_tag/cpython-$python_version%2B$python_tag-$rust_host-install_only.tar.gz" \
        | tar -xz -C "$base"
fi
mkdir -p "$base/bin"
ln -sf "$base/python/bin/python3" "$base/bin/python"
ln -sf "$base/python/bin/python3" "$base/bin/python3"

cd "$base/runner"
if [ ! -x ./run.sh ]; then
    curl -fsSL "https://github.com/actions/runner/releases/download/v$version/actions-runner-osx-$runner_arch-$version.tar.gz" | tar -xz
fi
if [ ! -f .runner ]; then
    "${run[@]}" ./config.sh --unattended --replace --url "$url" --token "$token" \
        --name "tore-mac-$arch" --labels "$label" --work "$base/work"
fi
# The runner reads .env and .path at start: this runner's own Rust first.
cat > .env <<EOF
CARGO_HOME=$CARGO_HOME
RUSTUP_HOME=$RUSTUP_HOME
EOF
echo "$base/bin:$CARGO_HOME/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin" > .path
if [ ! -f .service ]; then
    ./svc.sh install
fi
./svc.sh start
./svc.sh status

#!/usr/bin/env bash
# Register once (RUNNER_TOKEN is only needed the first time), then run. The
# runner folder, the work folder (checkouts and cargo target folders) and the
# Rust toolchain live in volumes, so registration, the runner's own updates
# and incremental builds survive restarts and image rebuilds.
set -euo pipefail
home=/home/runner/actions-runner
if [ ! -x "$home/run.sh" ]; then
    cp -a /opt/actions-runner/. "$home/"
fi
for dir in .cargo .rustup; do
    if [ -z "$(ls -A "/home/runner/$dir" 2>/dev/null)" ]; then
        cp -a "/opt/rustup-seed/$dir/." "/home/runner/$dir/"
    fi
done
cd "$home"
if [ ! -f .runner ]; then
    : "${RUNNER_TOKEN:?first start needs RUNNER_TOKEN, see docs/CI-RUNNERS.md}"
    ./config.sh --unattended --replace \
        --url "${RUNNER_URL:-https://github.com/john-overton/T.O.R.E-Fighters}" \
        --token "$RUNNER_TOKEN" \
        --name "$RUNNER_NAME" \
        --labels "${RUNNER_LABELS:-tore-ubuntu-22.04}" \
        --work /home/runner/work
fi
exec nice -n 10 ./run.sh

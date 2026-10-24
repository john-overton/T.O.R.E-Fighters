# A GitHub Actions runner for the ubuntu-22.04 jobs. Ubuntu 22.04 is the
# release floor (glibc 2.35), so what this builds starts on every supported
# distribution, as the hosted ubuntu-22.04 runner's builds do. The windowing
# libraries the workflows apt-get are already here, so that step is quick.
FROM ubuntu:22.04

ARG RUNNER_VERSION=2.338.1
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl git git-lfs jq sudo tzdata xz-utils zip unzip file \
        build-essential pkg-config binutils python3 python3-venv \
        libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxrandr-dev libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

# The runner user matches the host user so the bind-mounted folders stay
# readable from the desktop.
RUN useradd --create-home --uid 1000 --shell /bin/bash runner \
    && echo 'runner ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/runner

RUN mkdir -p /opt/actions-runner \
    && curl -fsSL "https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz" \
        | tar -xz -C /opt/actions-runner \
    && /opt/actions-runner/bin/installdependencies.sh \
    && rm -rf /var/lib/apt/lists/* \
    && chown -R runner:runner /opt/actions-runner

USER runner
WORKDIR /home/runner
# rustup into /opt/rustup-seed; the entrypoint copies it into the runner's
# volumes on first start. The pinned toolchain in rust-toolchain.toml installs
# itself on the first build and stays in those volumes.
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none --no-modify-path \
    && sudo mkdir -p /opt/rustup-seed \
    && sudo mv /home/runner/.cargo /home/runner/.rustup /opt/rustup-seed/ \
    && sudo chown -R runner:runner /opt/rustup-seed
ENV PATH=/home/runner/.cargo/bin:$PATH

COPY --chown=runner:runner entrypoint.sh /usr/local/bin/tore-runner
ENTRYPOINT ["/usr/local/bin/tore-runner"]

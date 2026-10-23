# The checked-in toolchain selects the compiler, including when this tag changes.
FROM rust:1.91.1-bookworm AS build
WORKDIR /src
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY crates crates
ARG TORE_BUILD_COMMIT
ENV CARGO_BUILD_JOBS=12
RUN test -n "$TORE_BUILD_COMMIT" && \
    TORE_BUILD_COMMIT="$TORE_BUILD_COMMIT" nice -n 5 cargo build --release --locked -p tore-server && \
    strip target/release/tore-server

# Debian 12 supplies glibc 2.36. Retail data only enters through a runtime mount.
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    mkdir /data /config && chown 1000:1000 /data /config
COPY --from=build /src/target/release/tore-server /usr/local/bin/tore-server
COPY tools/docker-entrypoint.sh /usr/local/bin/tore-entrypoint
USER 1000:1000
ENV TORE_DATA_DIR=/data
WORKDIR /config
EXPOSE 26900/udp
STOPSIGNAL SIGTERM
ENTRYPOINT ["/bin/sh", "/usr/local/bin/tore-entrypoint"]
CMD ["--config", "/config/server.conf"]

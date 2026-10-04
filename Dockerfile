# Linux amd64 is the exercised target. Other architectures are not accepted.
ARG RUST_IMAGE=docker.io/library/rust:1.99.0-slim-bookworm
ARG RUNTIME_IMAGE=docker.io/library/debian:bookworm-slim
FROM ${RUST_IMAGE} AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config clang cmake libssl-dev ca-certificates python3 binutils file \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
ARG SOURCE_REVISION=unknown
ENV CARGO_BUILD_JOBS=2
# No host Cargo tree or helpers are mounted. Keep the compiler's normal defaults.
RUN mkdir /out \
    && target="$(rustc -vV | sed -n 's/^host: //p')" \
    && cargo fetch --locked --target "$target" \
    && RUSTFLAGS='--remap-path-prefix=/src=/webtool --remap-path-prefix=/usr/local/cargo=/cargo' \
       cargo build --locked --release -p webtool-cli -p webtool-server \
         --message-format=json-render-diagnostics > /out/cargo.jsonl \
    && cargo metadata --locked --offline --format-version 1 --filter-platform "$target" > /out/metadata.json \
    && python3 scripts/package.py --record-build /out --source-revision "$SOURCE_REVISION"

FROM ${RUNTIME_IMAGE} AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 webtool \
    && useradd --uid 10001 --gid 10001 --create-home webtool \
    && mkdir /data /etc/webtool && chown webtool:webtool /data
COPY --from=build /out/webtool /out/webtoold /usr/local/bin/
COPY --from=build /out/materials/ /usr/share/doc/webtool/
COPY --from=build /out/webtool-*-source.tar.gz /usr/share/doc/webtool/
COPY config.example.toml /etc/webtool/server.toml
ARG SOURCE_REVISION=unknown
LABEL org.opencontainers.image.source="https://github.com/JCFrags/webtool" \
      org.opencontainers.image.revision="$SOURCE_REVISION" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"
USER 10001:10001
WORKDIR /data
EXPOSE 8420
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/webtoold"]
# 0.0.0.0 is container-local. Publish only to host 127.0.0.1, as compose.yaml does.
CMD ["--config", "/etc/webtool/server.toml", "--bind", "0.0.0.0:8420", "--data-dir", "/data"]

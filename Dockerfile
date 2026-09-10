# Source-only delivery: this image has not been built in the delivery environment.
FROM rust:slim-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config clang cmake ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
ARG SERVER_FEATURES=""
RUN if [ -n "$SERVER_FEATURES" ]; then \
      cargo build --release -p webtool-server --features "$SERVER_FEATURES"; \
    else cargo build --release -p webtool-server; fi

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home webtool && mkdir /data && chown webtool:webtool /data
COPY --from=build /src/target/release/webtoold /usr/local/bin/webtoold
USER webtool
WORKDIR /data
EXPOSE 8420
ENTRYPOINT ["/usr/local/bin/webtoold"]
CMD ["--bind", "0.0.0.0:8420", "--data-dir", "/data"]

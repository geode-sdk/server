# syntax=docker/dockerfile:1

ARG LIBC=musl

# 1. shared toolchain (Rust + Zig + cargo-chef + cargo-zigbuild)
# rust:1.97.0-slim-trixie
FROM --platform=$BUILDPLATFORM rust@sha256:7284e7501ed1b80ae3d2f826024e8384749bb860c46d7989d3b70033b56bf31e AS builder-tools

ARG ZIG_VERSION=0.15.2
ARG ZIG_SHA256=02aa270f183da276e5b5920b1dac44a63f1a49e55050ebde3aecc9eb82f93239
ARG CARGO_ZIGBUILD_VERSION=0.23.0
ARG CARGO_CHEF_VERSION=0.1.77

ENV PATH=/zig:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config ca-certificates curl xz-utils build-essential \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add \
        x86_64-unknown-linux-musl \
        aarch64-unknown-linux-musl \
        x86_64-unknown-linux-gnu \
        aarch64-unknown-linux-gnu

# Install Zig (used by cargo-zigbuild as the cross-linker)
RUN curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" -o /tmp/zig.tar.xz \
    && echo "${ZIG_SHA256}  /tmp/zig.tar.xz" | sha256sum -c - \
    && tar -xJf /tmp/zig.tar.xz \
    && mv "zig-x86_64-linux-${ZIG_VERSION}" /zig \
    && rm /tmp/zig.tar.xz

# Install cargo-chef and cargo-zigbuild
RUN cargo install --locked cargo-chef --version ${CARGO_CHEF_VERSION} \
    && cargo install --locked cargo-zigbuild --version ${CARGO_ZIGBUILD_VERSION}

WORKDIR /app

# 2. compute the cargo-chef recipe (used for dependency caching)
FROM builder-tools AS planner

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo chef prepare --recipe-path recipe.json

# 3. build the binary for the requested target arch + libc
FROM builder-tools AS builder

ARG LIBC=musl
ARG TARGETARCH

RUN case "${TARGETARCH}-${LIBC}" in \
        amd64-musl) echo "x86_64-unknown-linux-musl"  > /rust_target.txt ;; \
        arm64-musl) echo "aarch64-unknown-linux-musl" > /rust_target.txt ;; \
        amd64-gnu)  echo "x86_64-unknown-linux-gnu"   > /rust_target.txt ;; \
        arm64-gnu)  echo "aarch64-unknown-linux-gnu"  > /rust_target.txt ;; \
        *) echo "Unsupported TARGETARCH/LIBC combination: ${TARGETARCH}/${LIBC}" >&2; exit 1 ;; \
    esac

# Pre-build all dependencies (cached as long as Cargo.toml/lock don't change)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook \
        --release \
        --zigbuild \
        --target "$(cat /rust_target.txt)" \
        --recipe-path recipe.json

# Build the actual application
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx

# SQLX offline mode – the .sqlx directory provides the query metadata
ENV SQLX_OFFLINE=true

RUN cargo zigbuild \
        --release \
        --target "$(cat /rust_target.txt)" \
    && cp "target/$(cat /rust_target.txt)/release/geode-index" /app/geode-index

# 4a. minimal Alpine runtime (musl / statically linked)
FROM alpine:3.21 AS runtime-musl

RUN apk add --no-cache ca-certificates tzdata

WORKDIR /app
COPY --from=builder /app/geode-index ./geode-index
COPY migrations ./migrations
COPY static ./static
COPY docker/sync-static.sh /usr/local/bin/sync-static.sh

RUN addgroup -S -g 1000 geode && adduser -S -u 1000 geode -G geode \
    && chmod +x /usr/local/bin/sync-static.sh \
    && chown -R geode:geode /app
USER geode

EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q --spider "http://127.0.0.1:${PORT:-3000}/" || exit 1
ENTRYPOINT ["./geode-index"]

# 4b. Debian slim runtime (glibc / dynamically linked)
FROM debian:trixie-slim AS runtime-gnu

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tzdata wget \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/geode-index ./geode-index
COPY migrations ./migrations
COPY static ./static
COPY docker/sync-static.sh /usr/local/bin/sync-static.sh

RUN groupadd --system --gid 1000 geode && useradd --system --uid 1000 --gid geode geode \
    && chmod +x /usr/local/bin/sync-static.sh \
    && chown -R geode:geode /app
USER geode

EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q --spider "http://127.0.0.1:${PORT:-3000}/" || exit 1
ENTRYPOINT ["./geode-index"]

FROM runtime-${LIBC:-musl} AS runtime
ARG LIBC

# Overriden in CI
ARG VERSION=dev
ARG REVISION=unknown
ARG BUILD_DATE=unknown

LABEL org.opencontainers.image.title="geode-index" \
      org.opencontainers.image.description="Geode SDK mod index server" \
      org.opencontainers.image.source="https://github.com/geode-sdk/server" \
      org.opencontainers.image.documentation="https://github.com/geode-sdk/server#environment-variables" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.created="${BUILD_DATE}"

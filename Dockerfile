# syntax=docker/dockerfile:1

# 1. shared toolchain (Rust + Zig + cargo-chef + cargo-zigbuild)
FROM --platform=$BUILDPLATFORM debian:trixie-slim AS builder-tools

ARG ZIG_VERSION=0.15.2
ARG CARGO_ZIGBUILD_VERSION=0.22.1
ARG CARGO_CHEF_VERSION=0.1.77

ENV CARGO_HOME=/cargo \
    RUSTUP_HOME=/rustup \
    PATH=/cargo/bin:/zig:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config ca-certificates curl xz-utils build-essential \
    && rm -rf /var/lib/apt/lists/*

# Install Rust (minimal profile, stable toolchain)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable \
    && rustup target add \
        x86_64-unknown-linux-musl \
        aarch64-unknown-linux-musl \
        x86_64-unknown-linux-gnu \
        aarch64-unknown-linux-gnu

# Install Zig (used by cargo-zigbuild as the cross-linker)
RUN curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" \
        | tar -xJ \
    && mv "zig-x86_64-linux-${ZIG_VERSION}" /zig

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
COPY config ./config

RUN addgroup -S geode && adduser -S geode -G geode \
    && chown -R geode:geode /app
USER geode

EXPOSE 3000
ENTRYPOINT ["./geode-index"]

# 4b. Debian slim runtime (glibc / dynamically linked)
FROM debian:trixie-slim AS runtime-gnu

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/geode-index ./geode-index
COPY migrations ./migrations
COPY config ./config

RUN groupadd --system geode && useradd --system --gid geode geode \
    && chown -R geode:geode /app
USER geode

EXPOSE 3000
ENTRYPOINT ["./geode-index"]

FROM runtime-${LIBC:-musl} AS runtime

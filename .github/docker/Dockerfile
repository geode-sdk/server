FROM ubuntu:24.04
LABEL authors="geode-sdk"

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        g++-aarch64-linux-gnu \
        && \
    rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
     CARGO_HOME=/usr/local/cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH="/usr/local/cargo/bin:${PATH}"

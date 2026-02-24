FROM ubuntu:22.04
LABEL authors="geode-sdk"

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        g++-aarch64-linux-gnu \
        build-essential \
        curl \
        ca-certificates \
        && \
    rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

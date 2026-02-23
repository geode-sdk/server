FROM ubuntu:24.04
LABEL authors="geode-sdk"

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        g++-aarch64-linux-gnu \
        && \
    rm -rf /var/lib/apt/lists/*

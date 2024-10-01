FROM rust:1-bullseye
FROM ubuntu:22.04

RUN apt-get -qq update 

RUN apt-get install -y -q \
    git curl ca-certificates build-essential \
    libssl-dev pkg-config

# install rustup and add to path
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install sqlx-cli 
RUN cargo install cargo-watch 
RUN rustup component add clippy

WORKDIR /app

FROM ubuntu:22.04

RUN apt-get -qq update 

RUN apt-get install -y -q \
    git curl ca-certificates build-essential \
    libssl-dev pkg-config software-properties-common

# install rustup and add to path
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install sqlx-cli 
RUN cargo install cargo-watch 
RUN rustup component add clippy
RUN rustup component add rust-analyzer

# install neovim and other dev stuff
RUN apt-add-repository ppa:neovim-ppa/unstable
RUN apt-get update
RUN apt-get install -y -q neovim fzf \
    postgresql-client ripgrep

ENV TERM xterm-256color

RUN git config --global --add safe.directory /app

WORKDIR /app

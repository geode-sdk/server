FROM rust:1-alpine3.19 as chef
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN apk update
RUN apk add --no-cache pkgconfig openssl openssl-dev musl-dev
RUN cargo install cargo-chef
WORKDIR /app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release
RUN strip target/release/geode-index

FROM alpine:3.19
WORKDIR /app
COPY --from=builder /app/target/release/geode-index /app/target/release/geode-index
COPY . .
RUN apk add --no-cache libgcc
RUN chmod +x /app/target/release/geode-index
EXPOSE 3000
ENTRYPOINT [ "/app/target/release/geode-index" ]
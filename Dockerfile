#0 ==
FROM rust:1.90 AS tools

RUN cargo install cargo-chef --version 0.1.71 \
 && cargo install sccache --version 0.10.0

#1 ==
FROM rust:1.90 AS base

COPY --from=tools /usr/local/cargo/bin/* /usr/local/cargo/bin/

ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/sccache

WORKDIR /app

#2 ==
FROM base AS planner

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo chef prepare --recipe-path recipe.json

#3 ==
FROM base AS builder

COPY --from=planner /app/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo build --release

#4 ==
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl3 \
  && ln -snf /usr/share/zoneinfo/Asia/Jakarta /etc/localtime \
  && echo "Asia/Jakarta" > /etc/timezone \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/tethys ./tethys
CMD ["./tethys"]
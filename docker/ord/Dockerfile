FROM rust:1.83.0-bookworm as builder

WORKDIR /usr/src

RUN apt-get update && apt-get install -y git libssl-dev build-essential libclang-dev clang pkg-config bash

RUN git clone https://github.com/ordinals/ord -b 0.22.1
WORKDIR /usr/src/ord

RUN cargo build --bin ord --release

FROM debian:bookworm-slim

COPY --from=builder /usr/src/ord/target/release/ord /usr/local/bin
COPY docker-entrypoint.sh /docker-entrypoint.sh

RUN apt-get update && apt-get install -y openssl curl jq
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

CMD ["/docker-entrypoint.sh"]

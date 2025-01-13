FROM rust:1.83.0-slim-bookworm AS build

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update -y && apt-get install build-essential pkg-config \
    libssl-dev python3-dev zlib1g-dev libsasl2-dev libzstd-dev libsasl2-dev \
    libcurl4-openssl-dev -y

# workaround to cache dependencies compilation
# https://github.com/rust-lang/cargo/issues/2644#issuecomment-335272535
# https://stackoverflow.com/questions/42130132/can-cargo-download-and-build-dependencies-without-also-building-the-application
WORKDIR /usr/src
RUN USER=root cargo new --vcs none ingest
WORKDIR /usr/src/ingest
COPY vendor ./vendor/
COPY Cargo.toml Cargo.lock ./
RUN cargo build --target x86_64-unknown-linux-gnu --release
RUN rm -rf src
# end workaround

COPY . .
RUN cargo build --target x86_64-unknown-linux-gnu --release --bins

FROM debian:bookworm-slim

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update -y && apt-get install ca-certificates \
    libssl-dev python3-dev zlib1g-dev libsasl2-dev libzstd-dev libsasl2-dev \
    libcurl4-openssl-dev -y

RUN mkdir -p /opt/ingest/python
ENV PYTHONPATH=/opt/ingest/python

COPY --from=build /usr/src/ingest/target/x86_64-unknown-linux-gnu/release/ingest /usr/local/bin/ingestd
COPY --from=build /usr/src/ingest/src/python/ /usr/local/src/ingest/python/
ENTRYPOINT ["ingestd"]

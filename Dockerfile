FROM rust:1.73.0-bookworm AS build

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update -y && apt-get install build-essential pkg-config libssl-dev python3-dev -y

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

FROM debian:bookworm-20230919

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update -y && apt-get install ca-certificates python3-dev -y

RUN mkdir -p /opt/ingest/python
ENV PYTHONPATH=/opt/ingest/python

COPY --from=build /usr/src/ingest/target/x86_64-unknown-linux-gnu/release/ingest /usr/local/bin/ingestd
COPY --from=build /usr/src/ingest/src/python/ /usr/local/src/ingest/python/
ENTRYPOINT ["ingestd"]

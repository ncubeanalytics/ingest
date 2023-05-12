FROM rust:1.69.0-slim-bullseye AS build
RUN apt-get update -y && apt-get install build-essential pkg-config libssl-dev -y
WORKDIR /usr/src/ingest
COPY . .
RUN cargo build --target x86_64-unknown-linux-gnu --release --bins

FROM debian:bullseye-20230502
RUN apt-get update -y && apt-get install ca-certificates -y
COPY --from=build /usr/src/ingest/target/x86_64-unknown-linux-gnu/release/ingest /usr/local/bin/ingestd
COPY --from=build /usr/src/ingest/src/python/ /usr/local/src/ingest/python/
ENTRYPOINT ["ingestd"]

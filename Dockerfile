FROM rust:1.67.1-slim-buster AS build
RUN apt-get update -y && apt-get install build-essential pkg-config libssl-dev -y
WORKDIR /usr/src/ingest
COPY . .
RUN cargo build --target x86_64-unknown-linux-gnu --release --bins

FROM debian:buster-20230109-slim
RUN apt-get update -y && apt-get install ca-certificates -y
COPY --from=build /usr/src/ingest/target/x86_64-unknown-linux-gnu/release/ingest /usr/local/bin/ingestd
ENTRYPOINT ["ingestd"]

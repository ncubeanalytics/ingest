FROM rust:1.56.1-slim-bullseye AS build
RUN apt-get update -y && apt-get install build-essential pkg-config libssl-dev -y
WORKDIR /usr/src/query
COPY . .
RUN cargo build --target x86_64-unknown-linux-gnu --release --bins

FROM debian:bullseye-slim
RUN apt-get update -y && apt-get install ca-certificates -y
COPY --from=build /usr/src/query/target/x86_64-unknown-linux-gnu/release/ingest /usr/local/bin/ingestd
CMD ["ingestd"]

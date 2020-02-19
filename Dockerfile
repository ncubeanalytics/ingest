FROM rust:1-buster AS build
RUN apt-get update && apt-get install -y musl-tools
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /usr/src/ingest
COPY . .
RUN cargo install --target x86_64-unknown-linux-musl --path .

FROM scratch
COPY --from=build /usr/local/cargo/bin/ingest /usr/local/bin/ingest
CMD ["ingest"]

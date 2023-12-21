# ncube ingest

HTTP service to ingest data into Kafka. Check [DESIGN.md](DESIGN.md) for more details.

## Clone
This repo uses git submodules, so to get all the code use:
```sh
git clone --recurse-submodules git@github.com:ncubeanalytics/ingest.git

# or
git clone git@github.com:ncubeanalytics/ingest.git
cd ingest
git submodule update --init --recursive
```

## Vendored dependencies
This project has some private git dependencies which are vendored
in the `vendor` dir using git submodules.

To fetch them:
```sh
git submodule update --init --recursive
```

To update them:
```sh
git submodule update --remote --init --recursive
```

## Configuration

Check [config.sample.toml](./config.sample.toml)

## Testing

The tests are mainly integration tests. To run them you will need a kafka broker.

You can start a broker with docker compose:
```sh
BROKER_HOST=localhost BROKER_PORT=19092 docker-compose up -d
```

Run integration tests only:
```sh
BROKER_ADDRESS=localhost:19092 cargo test --test server
```

Run all tests:
```sh
cargo test
```

## Build
For development:
```sh
cargo build
```

## Run
For development:
```sh
cargo run
```

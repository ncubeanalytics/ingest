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

The tests are mainly integration tests. To run them you will need a kafka broker. The one
from the common repo docker compose is ready to use.

Run integration tests only:
```sh
cargo test --test server
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

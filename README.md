# N-Cube ingest service
N-Cube service for ingesting data from clients.
This service does no validation or parsing of data and simply forwards everything
as is in client requests to tenant specific kafka topics.

Works through HTTP and WebSockets.

## Clone
This repo uses git submodules, so to get all the code use:
```sh
git clone --recurse-submodules git@gitlab.com:n-cube/ingest.git

# or
git clone git@gitlab.com:n-cube/ingest.git
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
#### Integration
To run **integration** tests, you will need a kafka broker.

You can start a broker with docker compose:
```sh
BROKER_HOST=localhost BROKER_PORT=19092 docker-compose up -d
```

Run the integration tests:
```sh
BROKER_ADDRESS=localhost:19092 cargo test --test server
```

Run all tests:
```sh
cargo test
```

## Docker
To create a docker image with a static binary of the ingest service:
```sh
./scripts/build.sh $VERSION
```

Push it to docker registry

```sh
./scripts/push.sh $VERSION
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

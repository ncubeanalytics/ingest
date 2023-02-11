# N-Cube ingest service
N-Cube service for ingesting data from clients.
This service does no validation or parsing of data and simply forwards everything
as is in client requests to tenant specific kafka topics.

Works through HTTP and WebSockets.

## Clone
This repo uses git submodules, so to get all the code use:
```
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
```
git submodule update --init --recursive
```

To update them:
```
git submodule update --remote --init --recursive
```

## Configuration

Check [config.sample.toml](./config.sample.toml)

## Testing
#### Integration
To run **integration** tests, you will need a kafka broker.

You can start a broker with docker compose:
```
BROKER_HOST=localhost BROKER_PORT=19092 docker-compose up -d
```

Run the integration tests:
```
BROKER_ADDRESS=localhost:19092 cargo test --test server
```

Run all tests:
```
cargo test
```

## Docker
To create a docker image with a static binary of the ingest service:
```
# make sure that submodules are up to date
git submodule update --remote --init --recursive

docker build .
```

To set up and run docker containers for kafka and zookeeper (useful when
developing/running integration tests):
```
docker-compose up -d
```

The containerized kafka broker will be accessible at `localhost:19092`, which
is also the default kafka broker address that the ingest service uses.

## Build
For development:
```
cargo build
```

## Run
For development:
```
cargo run
```

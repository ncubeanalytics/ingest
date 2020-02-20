# phaedra ingest service
phaedra service for ingesting events from clients.

Works through HTTP and WebSockets.

**TODO**: Document endpoints.

## Configuration
Config and default values (TOML format):
```toml
# local address to bind the service to
addr = '127.0.0.1:8088'
# output logs in JSON format
log_json = false

[kafka]
# list of kafka brokers to connect to, comma separated
servers = '127.0.0.1:19092'
# topic to send events to
topic = 'events'
# value for "delivery.timeout.ms" kafka producer config property
timeout_ms = '5000'
# value for "acks" kafka producer config property
acks = 'all'
```

The ingest service supports loading configuration from a TOML file.
If you want to use a custom configuration file set the `PHAEDRA_INGEST_CONFIG`
env var to the file path:
```
PHAEDRA_INGEST_CONFIG=/path/to/config.toml cargo run
```

If no config file is provided or values are missing, defaults will be used.

## Testing
#### Unit
```
cargo test --lib
```

#### Integration
To run **integration** tests, you will need a kafka broker.
By default, ingest tries to connect to one at`127.0.0.1:19092`.
You can start a broker at that address with docker compose:
```
docker-compose up -d
```

If you want to use a different broker address use a configuration file as
explained in the [Configuration](#configuration) section.

Run the integration tests:
```
cargo test --test server
```

Run all tests:
```
cargo test
```

## Docker
To create a docker container with a static binary of the ingest service:
```
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
For production use the [docker container](#docker).

## Run
For development:
```
cargo run
```

For production use the [docker container](#docker).

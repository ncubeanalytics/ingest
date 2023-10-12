# Ingest Service Design

## Purpose

The service's purpose is to be an HTTP entry point for ingesting real time and bulk data from
external sources.

The service is stateless and forwards data to Kafka as fast as possible, with configurable
latency/throughput.

The service responds to an HTTP request successfully only after the data has successfully
reached the broker.

The service is meant to cover a wide range of ingest use cases, from high throughput streaming
data to individual HTTP requests for each.

There is special support for HTTP webhooks, with the possibility to configure additional
logic with a python plugin to cover things such as webhook authentication and custom
responses.

An HTTP endpoint is preferable to sending directly to kafka because:
* external data is widely available through HTTP webhooks
* it is much easier for applications to interface with HTTP than the kafka protocol
* it hides the kafka topic configuration behind the service, decoupling apps from kafka. the
  coupling with the ingest service is much lighter and easier to deal with with kafka

## Operation

Data is sent to the service through a POST /<schema-id> request. Different data streams
are identified by the schema-id and different ingest behavior can be configured based on the
schema-id.

The service will forward the data exactly as received, avoiding parsing the contents or any
transformation. The only parsing and transform that takes place is breaking line-delimited json
into individual lines and forwarding them as individual data items. 

The service can also forward metadata to kafka such as request url, headers, method, client ip
address. Metadata is passed as kafka headers.

## Configuration

Configuration can be specified as a default for the whole service and overriden for specific
schemas as follows:

```toml
[service.default_schema_config]
# ... defaults

[[service.schema_config]]
schema_id = "1"
# ... schema 1 overrides
```

### Schema configuration options

#### `destination_topic`

To which kafka topic to forward the data

```toml
destination_topic = 'events'
```

#### `content_type_from_header`, `content_type`

The data format that data is delivered to the HTTP endpoint. Is read from the `Content-Type`
header by default, but can be overriden by config. Supported values are:
* "application/json" (default)
* "application/jsonlines"

```toml
content_type = "application/json"
```

#### `forward_request_url`, `forward_request_method`, `forward_request_http_headers`

Whether to forward the HTTP request url, method, and headers to kafka, useful when they carry
data of interest. Passed as a kafka headers with configurable keys.

```toml
forward_request_url = false
forward_request_method = false
forward_request_http_headers = false
```

#### `response_status`

Which HTTP status code to return on successful forwarding of data.

```toml
response_status = 200
```

#### `allowed_methods`

Which HTTP methods are allowed.

```toml
allowed_methods = ["POST"]
```

#### `python_request_processor`

A subclass of `ncube_ingest_plugin.RequestProcessor` that can be used to add custom behavior

### Header names

Data passed to kafka as headers can have the header keys configured

```toml
[headers]
schema_id = "ncube-ingest-schema-id"
ip = "ncube-ingest-ip"
http_url = "ncube-ingest-http-url"
http_method = "ncube-ingest-http-method"
http_header_prefix = "ncube-ingest-http-header-"
```

### Kafka librdkafka producer

All librdkafka settings can be configured

```toml
[librdkafka_config]
# accepts any librdkafka configuration value
"bootstrap.servers" = "localhost:9093"
```

## Custom behavior with python plugin

It is possible to implement custom handling of HTTP requests beyond the configuration
options by implementing the `ncube_ingest_plugin.PythonProcessor` interface, which
receives the full HTTP request and can implement arbitrary request handling to 
* form the full HTTP response which will be returned to the ingest caller
* decide whether to forward or ignore data from being forwarded to kafka

A common use case for this is to implement custom behavior required by webhook initialization,
authentication, and general operation.

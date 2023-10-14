# Ingest Service Design

## Contents

<!-- TOC -->
* [Ingest Service Design](#ingest-service-design)
  * [Contents](#contents)
  * [Purpose](#purpose)
  * [Operation](#operation)
  * [Configuration](#configuration)
    * [Schema configuration options](#schema-configuration-options)
    * [Header names](#header-names)
    * [Kafka librdKafka producer](#kafka-librdkafka-producer)
    * [Other service configuration](#other-service-configuration)
  * [Custom behavior with python plugin](#custom-behavior-with-python-plugin)
<!-- TOC -->

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

An HTTP endpoint is useful when compared to sending directly to Kafka because:
* external data is widely available through HTTP webhooks
* it is much easier for applications to interface with HTTP than the Kafka protocol
* it hides the Kafka topic configuration behind the service, decoupling apps from Kafka. The
  coupling with the ingest service is lighter and easier to deal with than Kafka

## Operation

Data is sent to the service through a POST /<schema-id> request. Different data streams
are identified by the schema-id and different ingest behavior can be configured based on the
schema id. The schema id is an arbitrary string.

The service will forward the data exactly as received, avoiding parsing the contents or any
transformation, with the following exceptions:
* for json and line-delimited json, whitespace is trimmed
* for line-delimited json, lines are broken into individual lines and forwarded as separate
Kafka messages

The service can also forward metadata to Kafka such as request url, headers, method, client ip
address. Such metadata is forwarded as Kafka headers.

## Configuration

Configuration can be specified as a default for all schemas and overriden for specific
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

To which Kafka topic to forward the data

```toml
destination_topic = 'events'
```

#### `content_type_from_header`, `content_type`

The data format of the data delivered to the HTTP endpoint. It is read from the `Content-Type`
header by default, but can be overriden by config. Supported values are:
* "application/json" (default)
* "application/jsonlines"
* "application/octet-stream" (any other unsupported content type also lands here)

```toml
content_type_from_header = false
content_type = "application/json"
```

#### `forward_request_url`, `forward_request_method`, `forward_request_http_headers`

Whether to forward the HTTP request url, method, and headers to Kafka, useful when they carry
data of interest. Passed as Kafka headers with [configurable header names](#header-names).

```toml
forward_request_url = false
forward_request_method = false
forward_request_http_headers = false
```

#### `response_status`

Which HTTP status code to return on successful forwarding of data. Default: 200

```toml
response_status = 200
```

#### `allowed_methods`

Which HTTP methods are allowed.

```toml
allowed_methods = ["POST"]
```

#### `python_request_processor`

A nested configuration that specifies a [Python plugin](#custom-behavior-with-python-plugin).
Can be set once or multiple times, per schema or the global default.

Example of setting a python plugin for schema with id 1 for GET and POST requests:

```toml
[[service.schema_config]]
schema_id = "1"

[[service.schema_config.python_request_processor]]
methods = ["GET", "POST"]
processor = "webhook.mailgun:MailgunProcessor"
implements_process_head = true
process_is_blocking = true
process_head_is_blocking = false
```

##### `python_request_processor.processor`

The import location of a `ncube_ingest_plugin.RequestProcessor` subclass, in the form of
`python.module.path:module_attribute`

##### `python_request_processor.methods`

The list of HTTP methds the processor should be used with. If omitted, will be used with all methods.

##### `python_request_processor.implements_process_head`

Whether the `process_head()` function is implemented. If set, it will be called before the
`process()` function, and if it returns a response the `process()` function will never be called.
Useful if the processor does not need the request body for its outcome.

##### `python_request_processor.process_is_blocking`, `python_request_processor.process_head_is_blocking`

Whether the respective process function is expected to do blocking IO. If set, it will be invoked
in a `tokio::spawn_blocking` task to avoid stalling the rest of the asynchronous tasks.


### Header names

Data passed to Kafka as headers can have the header keys configured

```toml
[headers]
schema_id = "ncube-ingest-schema-id"
ip = "ncube-ingest-ip"
http_url = "ncube-ingest-http-url"
http_method = "ncube-ingest-http-method"
http_header_prefix = "ncube-ingest-http-header-"
```

### Kafka librdKafka producer

All librdKafka settings can be configured

```toml
[librdKafka_config]
# accepts any librdKafka configuration value
"bootstrap.servers" = "localhost:9093"
```

### Other service configuration

#### `max_event_size_bytes`

The max supported event size in bytes. This should equal the configured max message side on Kafka.
Default: 1Mb (this is also the default Kafka `message.max.bytes`, and the message size limit on
Azure Event Hubs)

#### `keepalive_seconds`

The HTTP keep-alive timeout. Default: 5 minutes.

## Custom behavior with python plugin

It is possible to implement custom handling of HTTP requests beyond the configuration
options by implementing the `ncube_ingest_plugin.PythonProcessor` interface, which
receives the full HTTP request and can implement arbitrary request handling to 
* form the full HTTP response which will be returned to the ingest caller
* decide whether to forward or ignore data from being forwarded to Kafka

A common use case for this is to implement custom behavior required by webhook initialization,
authentication, and general operation.

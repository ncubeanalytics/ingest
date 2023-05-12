use std::collections::HashMap;
use std::net::SocketAddr;

use common::config::CommonConfig;
use common::logging::LoggingConfig;
use serde::Deserialize;
use vec1::{vec1, Vec1};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct HeaderNames {
    pub schema_id: String,
    pub ip: String,
    pub http_url: String,
    pub http_method: String,
    pub http_header_prefix: String,
}

impl Default for HeaderNames {
    fn default() -> HeaderNames {
        HeaderNames {
            schema_id: "ncube-ingest-schema-id".to_owned(),
            ip: "ncube-ingest-ip".to_owned(),
            http_url: "ncube-ingest-http-url".to_owned(),
            http_method: "ncube-ingest-http-method".to_owned(),
            http_header_prefix: "ncube-ingest-http-header-".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub enum ContentType {
    #[serde(rename = "application/json")]
    Json,
    #[serde(rename = "application/jsonlines")]
    Jsonlines,
}

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default)]
pub struct KafkaSecrets {
    pub sasl_password_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SchemaConfig {
    #[serde(default = "default_content_type_from_header")]
    pub content_type_from_header: bool,
    #[serde(default = "default_content_type")]
    pub content_type: Option<ContentType>,
    #[serde(default = "default_forward_request_url")]
    pub forward_request_url: bool,
    #[serde(default = "default_forward_request_method")]
    pub forward_request_method: bool,
    #[serde(default = "default_forward_request_http_headers")]
    pub forward_request_http_headers: bool,
    #[serde(default = "default_response_status")]
    pub response_status: u16,
    #[serde(default = "default_allowed_methods")]
    pub allowed_methods: Vec1<String>,
    pub destination_topic: String,
    #[serde(default = "default_python_request_processor")]
    pub python_request_processor: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PartialSchemaConfig {
    pub content_type_from_header: Option<bool>,
    pub content_type: Option<ContentType>,
    pub forward_request_url: Option<bool>,
    pub forward_request_method: Option<bool>,
    pub forward_request_http_headers: Option<bool>,
    pub response_status: Option<u16>,
    pub allowed_methods: Option<Vec1<String>>,
    pub destination_topic: Option<String>,
    pub python_request_processor: Option<String>,
}

impl Default for PartialSchemaConfig {
    fn default() -> PartialSchemaConfig {
        PartialSchemaConfig {
            content_type_from_header: None,
            content_type: None,
            forward_request_url: None,
            forward_request_method: None,
            forward_request_http_headers: None,
            allowed_methods: None,
            response_status: None,
            destination_topic: None,
            python_request_processor: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PartialSchemaConfigWithSchemaId {
    pub schema_id: String,
    #[serde(flatten)]
    pub schema_config: PartialSchemaConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServiceConfig {
    pub addr: SocketAddr,
    #[serde(default = "default_keepalive_seconds")]
    pub keepalive_seconds: u64,
    #[serde(default = "default_python_plugin_src_dir")]
    pub python_plugin_src_dir: String,
    pub default_schema_config: SchemaConfig,
    #[serde(default)]
    pub schema_config: Vec<PartialSchemaConfigWithSchemaId>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    #[serde(default)]
    pub headers: HeaderNames,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub librdkafka_config: HashMap<String, String>,
    #[serde(default)]
    pub librdkafka_secrets: KafkaSecrets,
}

impl CommonConfig for Config {
    const CMD_NAME: &'static str = "ingestd";
    const DEFAULT_CONFIG_PATH: &'static str = "/etc/ncube-ingest/ingest.toml";
}

const fn default_keepalive_seconds() -> u64 {
    60
}
fn default_python_plugin_src_dir() -> String {
    "/usr/local/src/ingest/python/".to_owned()
}
const fn default_content_type_from_header() -> bool {
    true
}
const fn default_content_type() -> Option<ContentType> {
    None
}
const fn default_forward_request_url() -> bool {
    false
}
const fn default_forward_request_http_headers() -> bool {
    false
}
const fn default_forward_request_method() -> bool {
    false
}
const fn default_response_status() -> u16 {
    200
}
const fn default_python_request_processor() -> Option<String> {
    None
}
fn default_allowed_methods() -> Vec1<String> {
    vec1!["POST".to_owned()]
}
// const fn none_content_type() -> Option<ContentType> {
//     None
// }
//
// const fn none_response_status() -> Option<u16> {
//     None
// }
//
// const fn none_bool() -> Option<bool> {
//     None
// }
